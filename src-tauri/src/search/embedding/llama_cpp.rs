//! [`LlamaCppEmbeddingEngine`] — Granite R2 (GGUF Q8_0, 384d, CLS pooling)
//! running on llama.cpp via the `llama-cpp-2` crate.
//!
//! # Lifecycle
//!
//! The model is **lazy-loaded**: loading does not happen at `new()` but on the
//! first call to [`EmbeddingEngine::embed_query`] or
//! [`EmbeddingEngine::embed_documents`].
//! After the first load the model lives for the lifetime of this struct.
//!
//! ```text
//! PrepLoop starts          → model unloaded (0 extra RSS)
//! First semantic search    → model loaded once (~50 MB RSS)
//! Every subsequent search  → reuses loaded model (3–4 ms p50 on CPU)
//! ```
//!
//! # Thread safety
//!
//! `LlamaCppEmbeddingEngine` is `Send + Sync`. Concurrent callers share a
//! single `Mutex<Option<LoadedModel>>`. Because embedding a single query takes only a
//! few milliseconds on CPU, lock contention is negligible at interactive use.
//!
//! # CPU baseline
//!
//! This implementation uses zero GPU layers (`n_gpu_layers = 0`).
//! Platform-specific acceleration can be added here in the future without
//! touching `SearchService`, FTS, the vector index, or the ranking code.

use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::LlamaModel;

use super::engine::{l2_normalize, Embedding, EmbeddingEngine, EmbeddingError};

/// The expected embedding dimension for Granite R2 Q8_0.
const GRANITE_R2_DIMS: usize = 384;

/// Maximum context window used for embedding (must be ≥ longest document).
/// Granite R2's training context is 512 tokens; 1024 gives headroom.
const CTX_SIZE: u32 = 1024;

/// Number of CPU threads used for embedding inference.
/// Defaults to the number of physical cores, capped at 8.
fn default_n_threads() -> i32 {
    let phys = num_cpus::get_physical() as i32;
    phys.clamp(1, 8)
}

// ---------------------------------------------------------------------------
// Global llama backend (must be initialised once per process)
// ---------------------------------------------------------------------------

static LLAMA_BACKEND: Mutex<Option<Arc<LlamaBackend>>> = Mutex::new(None);

fn get_or_init_backend() -> Result<Arc<LlamaBackend>, EmbeddingError> {
    let mut guard = LLAMA_BACKEND
        .lock()
        .map_err(|_| EmbeddingError::ModelLoad("backend mutex poisoned".to_string()))?;
    if let Some(backend) = guard.as_ref() {
        return Ok(Arc::clone(backend));
    }
    let mut backend = LlamaBackend::init()
        .map_err(|e| EmbeddingError::ModelLoad(format!("llama backend init failed: {e}")))?;
    if !cfg!(debug_assertions) {
        backend.void_logs();
    }
    let arc_backend = Arc::new(backend);
    *guard = Some(Arc::clone(&arc_backend));
    Ok(arc_backend)
}

// ---------------------------------------------------------------------------
// Internal loaded state
// ---------------------------------------------------------------------------

struct LoadedModel {
    backend: Arc<LlamaBackend>,
    model: LlamaModel,
    n_threads: i32,
}

// ---------------------------------------------------------------------------
// Public engine
// ---------------------------------------------------------------------------

/// Persistent, lazy-loading llama.cpp embedding engine.
///
/// Construct with [`LlamaCppEmbeddingEngine::new`], then store behind an
/// `Arc<dyn EmbeddingEngine>` in `SearchService`.
pub struct LlamaCppEmbeddingEngine {
    gguf_path: PathBuf,
    n_threads: i32,
    n_gpu_layers: u32,
    inner: Mutex<Option<LoadedModel>>,
}

impl LlamaCppEmbeddingEngine {
    /// Create an engine that will load the GGUF at `gguf_path` on first use.
    ///
    /// # Errors
    ///
    /// Returns an error only if the path does not exist. Actual model loading
    /// is deferred to first inference.
    pub fn new(gguf_path: impl AsRef<Path>) -> Result<Self, EmbeddingError> {
        let path = gguf_path.as_ref().to_path_buf();
        if !path.exists() {
            return Err(EmbeddingError::ModelLoad(format!(
                "GGUF not found: {}",
                path.display()
            )));
        }
        Ok(Self {
            gguf_path: path,
            n_threads: default_n_threads(),
            n_gpu_layers: 0,
            inner: Mutex::new(None),
        })
    }

    /// Override the number of CPU threads (useful in tests).
    pub fn with_n_threads(mut self, n: i32) -> Self {
        self.n_threads = n.max(1);
        self
    }

    /// Opt into GPU offload for offline tooling. The shipped application does
    /// not call this and always keeps the CPU-only default of zero layers.
    pub fn with_n_gpu_layers(mut self, layers: u32) -> Self {
        self.n_gpu_layers = layers;
        self
    }

    // ------------------------------------------------------------------
    // Internal: ensure model is loaded, then call f
    // ------------------------------------------------------------------

    fn with_model<F, R>(&self, f: F) -> Result<R, EmbeddingError>
    where
        F: FnOnce(&LoadedModel) -> Result<R, EmbeddingError>,
    {
        // SAFETY INVARIANT: every operation that can touch `LoadedModel`,
        // including model loading and inference, must stay inside this lock.
        // Do not expose `LoadedModel` or add an unlocked fast path.
        let mut guard = self.inner.lock().map_err(|_| {
            EmbeddingError::Inference("embedding engine mutex poisoned".to_string())
        })?;

        if guard.is_none() {
            #[cfg(debug_assertions)]
            eprintln!(
                "Loading embedding model {:?}; metadata={:?}",
                self.gguf_path,
                std::fs::metadata(&self.gguf_path)
            );
            // First call — load backend and model now.
            let backend = get_or_init_backend()?;

            let model_params = LlamaModelParams::default().with_n_gpu_layers(self.n_gpu_layers);

            let model = LlamaModel::load_from_file(&backend, &self.gguf_path, &model_params)
                .map_err(|e| EmbeddingError::ModelLoad(format!("{e}")))?;
            #[cfg(debug_assertions)]
            eprintln!("Embedding model loaded successfully");

            *guard = Some(LoadedModel {
                backend,
                model,
                n_threads: self.n_threads,
            });
        }

        f(guard.as_ref().unwrap())
    }

    // ------------------------------------------------------------------
    // Core: tokenise + encode a batch of texts → embeddings
    // ------------------------------------------------------------------

    fn encode_texts(
        loaded: &LoadedModel,
        texts: &[&str],
    ) -> Result<Vec<Embedding>, EmbeddingError> {
        let model = &loaded.model;
        let backend = &loaded.backend;
        let n_threads = loaded.n_threads;

        // Build context configured for embedding (CLS pooling, embeddings=true).
        let ctx_params = LlamaContextParams::default()
            .with_n_ctx(Some(NonZeroU32::new(CTX_SIZE).unwrap()))
            .with_n_batch(CTX_SIZE)
            // Encoder models cannot split one sequence across micro-batches.
            .with_n_ubatch(CTX_SIZE)
            .with_embeddings(true)
            .with_n_threads(n_threads)
            .with_n_threads_batch(n_threads);

        let mut ctx = model
            .new_context(backend, ctx_params)
            .map_err(|e| EmbeddingError::Inference(format!("context init: {e}")))?;

        let mut embeddings: Vec<Embedding> = Vec::with_capacity(texts.len());

        // Process texts one at a time via seq_id to keep memory bounded.
        // For batch embedding (Phase 6 worker) we can extend this to
        // submit multiple sequences per llama_decode call.
        for text in texts {
            // Tokenise (add BOS, no EOS for embedding models).
            let tokens = model
                .str_to_token(text, llama_cpp_2::model::AddBos::Always)
                .map_err(|e| EmbeddingError::Inference(format!("tokenise: {e}")))?;

            if tokens.is_empty() {
                return Err(EmbeddingError::EmptyInput);
            }

            // Truncate silently if over context window.
            let tokens: Vec<_> = tokens.into_iter().take(CTX_SIZE as usize - 1).collect();

            let mut batch = LlamaBatch::new(CTX_SIZE as usize, 1);
            let last_idx = tokens.len() - 1;
            for (i, token) in tokens.into_iter().enumerate() {
                batch
                    .add(token, i as i32, &[0], i == last_idx)
                    .map_err(|e| EmbeddingError::Inference(format!("batch add: {e}")))?;
            }

            ctx.clear_kv_cache();
            ctx.decode(&mut batch)
                .map_err(|e| EmbeddingError::Inference(format!("decode: {e}")))?;

            // CLS pooling — embeddings_seq_ith returns the pooled vector.
            let raw = ctx
                .embeddings_seq_ith(0)
                .map_err(|e| EmbeddingError::Inference(format!("embeddings_seq_ith: {e}")))?;

            let expected = GRANITE_R2_DIMS;
            if raw.len() != expected {
                return Err(EmbeddingError::DimensionMismatch {
                    expected,
                    actual: raw.len(),
                });
            }

            let mut v: Vec<f32> = raw.to_vec();
            l2_normalize(&mut v);
            embeddings.push(v);
        }

        Ok(embeddings)
    }
}

impl EmbeddingEngine for LlamaCppEmbeddingEngine {
    fn dimensions(&self) -> usize {
        GRANITE_R2_DIMS
    }

    fn embed_query(&self, text: &str) -> Result<Embedding, EmbeddingError> {
        if text.trim().is_empty() {
            return Err(EmbeddingError::EmptyInput);
        }
        self.with_model(|loaded| {
            let mut vecs = Self::encode_texts(loaded, &[text])?;
            Ok(vecs.remove(0))
        })
    }

    fn embed_documents(&self, texts: &[String]) -> Result<Vec<Embedding>, EmbeddingError> {
        if texts.is_empty() {
            return Ok(vec![]);
        }
        let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
        self.with_model(|loaded| Self::encode_texts(loaded, &refs))
    }
}

// SAFETY: llama.cpp's model/backend handles are not `Sync`. They are stored in
// `inner` and can only be created or accessed by `with_model`, which holds the
// mutex for the complete inference call. No reference to `LoadedModel` escapes
// that closure. If this access pattern changes, these impls must be revisited.
unsafe impl Send for LlamaCppEmbeddingEngine {}
unsafe impl Sync for LlamaCppEmbeddingEngine {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_remains_send_and_sync_for_shared_search_services() {
        fn assert_send_and_sync<T: Send + Sync>() {}
        assert_send_and_sync::<LlamaCppEmbeddingEngine>();
    }

    /// Verify that constructing with a non-existent path fails eagerly.
    #[test]
    fn missing_gguf_returns_error() {
        let result = LlamaCppEmbeddingEngine::new("/tmp/no_such_model.gguf");
        assert!(
            matches!(result, Err(EmbeddingError::ModelLoad(_))),
            "expected ModelLoad error"
        );
    }
}
