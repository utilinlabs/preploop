<script lang="ts">
  import { onDestroy, onMount, tick } from "svelte";
  import MathText from "$lib/components/math-text.svelte";
  import ScrollIndicator from "$lib/components/scroll-indicator.svelte";
  import { Button } from "$lib/components/ui/button";
  import {
    Dialog,
    DialogContent,
    DialogTitle,
  } from "$lib/components/ui/dialog";
  import { searchQuestions } from "$lib/services/question-search";
  import type {
    QuestionSearchResponse,
    QuestionSearchResult,
  } from "$lib/types";
  import { LoaderCircle, Search } from "@lucide/svelte";

  interface Props {
    open?: boolean;
    enabled?: boolean;
    sections?: string[];
    scopeLabel?: string;
  }

  type PendingSearch = {
    value: string;
    generation: number;
    sections: string[];
    scopeKey: string;
  };

  let {
    open = $bindable(false),
    enabled = true,
    sections = [],
    scopeLabel = "All Papers",
  }: Props = $props();

  let query = $state("");
  let response = $state<QuestionSearchResponse | null>(null);
  let isSearching = $state(false);
  let error = $state<string | null>(null);
  let inputElement = $state<HTMLInputElement | null>(null);
  let resultsScrollElement = $state<HTMLElement | null>(null);
  let searchTimer: ReturnType<typeof setTimeout> | null = null;
  let requestGeneration = 0;
  let requestInFlight = false;
  let queuedSearch: PendingSearch | null = null;
  let activeScopeKey = "";
  let wasOpen = false;

  const trimmedQuery = $derived(query.trim());
  const sectionKey = $derived(sections.join("\u001f"));
  const showResultsPanel = $derived(
    trimmedQuery.length >= 2 && (response !== null || error !== null),
  );

  function handleGlobalKeydown(event: KeyboardEvent) {
    if (
      enabled &&
      (event.metaKey || event.ctrlKey) &&
      event.key.toLowerCase() === "k"
    ) {
      event.preventDefault();
      open = true;
      return;
    }

    if (
      !open ||
      event.target === inputElement ||
      (event.key !== "Backspace" && event.key !== "Delete") ||
      event.metaKey ||
      event.ctrlKey ||
      event.altKey
    ) {
      return;
    }

    // Clicking or scrolling results can move focus away from the field. Keep
    // deletion behaving like an active search instead of invoking page-back.
    event.preventDefault();
    const input = inputElement;
    const selectionStart = input?.selectionStart ?? query.length;
    const selectionEnd = input?.selectionEnd ?? selectionStart;
    let deleteStart = selectionStart;
    let deleteEnd = selectionEnd;

    if (selectionStart === selectionEnd && event.key === "Backspace") {
      const previousCharacter = Array.from(query.slice(0, selectionStart)).at(
        -1,
      );
      deleteStart = Math.max(
        0,
        selectionStart - (previousCharacter?.length ?? 0),
      );
    } else if (selectionStart === selectionEnd && event.key === "Delete") {
      const nextCharacter = Array.from(query.slice(selectionEnd))[0];
      deleteEnd = Math.min(
        query.length,
        selectionEnd + (nextCharacter?.length ?? 0),
      );
    }

    query = query.slice(0, deleteStart) + query.slice(deleteEnd);
    void tick().then(() => {
      input?.focus();
      input?.setSelectionRange(deleteStart, deleteStart);
    });
  }

  function resetPendingSearch() {
    requestGeneration += 1;
    queuedSearch = null;
    if (searchTimer) {
      clearTimeout(searchTimer);
      searchTimer = null;
    }
  }

  function queueSearch(request: PendingSearch) {
    // Keep at most one queued request while SQLite finishes the current one.
    // Newer input replaces older input instead of building an IPC backlog.
    queuedSearch = request;
    if (!requestInFlight) {
      void drainSearchQueue();
    }
  }

  async function drainSearchQueue() {
    requestInFlight = true;
    let lastGeneration = -1;

    try {
      while (queuedSearch) {
        const current = queuedSearch;
        queuedSearch = null;
        lastGeneration = current.generation;

        try {
          const next = await searchQuestions(current.value, current.sections);
          if (
            current.generation !== requestGeneration ||
            query.trim() !== current.value ||
            sectionKey !== current.scopeKey
          ) {
            continue;
          }
          response = next;
          error = null;
          await tick();
          if (
            current.generation === requestGeneration &&
            query.trim() === current.value &&
            sectionKey === current.scopeKey &&
            resultsScrollElement
          ) {
            resultsScrollElement.scrollTop = 0;
          }
        } catch (caught) {
          if (current.generation !== requestGeneration) continue;
          response = null;
          error =
            caught instanceof Error ? caught.message : "Search is unavailable";
        }
      }
    } finally {
      requestInFlight = false;
      if (lastGeneration === requestGeneration) {
        isSearching = false;
      }
    }
  }

  function optionsFitSingleRow(result: QuestionSearchResult): boolean {
    if (result.options.length > 4) return false;
    const optionLengths = result.options.map((option) => option.text.length);
    return (
      optionLengths.every((length) => length <= 28) &&
      optionLengths.reduce((total, length) => total + length, 0) <= 88
    );
  }

  function resultContext(result: QuestionSearchResult): string {
    return [result.stage, result.paper].filter(Boolean).join(" · ");
  }

  $effect(() => {
    const dialogOpen = open;

    if (dialogOpen && !wasOpen) {
      void tick().then(() => inputElement?.focus());
    } else if (!dialogOpen && wasOpen) {
      query = "";
      response = null;
      error = null;
      isSearching = false;
      resetPendingSearch();
    }

    wasOpen = dialogOpen;
  });

  $effect(() => {
    const value = trimmedQuery;
    const dialogOpen = open;
    const scopeKey = sectionKey;
    const scopedSections = [...sections];
    const scopeChanged = activeScopeKey !== scopeKey;
    activeScopeKey = scopeKey;
    resetPendingSearch();

    if (scopeChanged) {
      response = null;
    }

    if (!dialogOpen || value.length < 2) {
      isSearching = false;
      error = null;
      if (value.length < 2) {
        response = null;
      }
      return;
    }

    const generation = requestGeneration;
    isSearching = true;
    error = null;
    searchTimer = setTimeout(() => {
      searchTimer = null;
      queueSearch({
        value,
        generation,
        sections: scopedSections,
        scopeKey,
      });
    }, 140);

    return () => {
      if (searchTimer) {
        clearTimeout(searchTimer);
        searchTimer = null;
      }
    };
  });

  onMount(() => {
    window.addEventListener("keydown", handleGlobalKeydown, { capture: true });
  });

  onDestroy(() => {
    resetPendingSearch();
    window.removeEventListener("keydown", handleGlobalKeydown, {
      capture: true,
    });
  });
</script>

<Button
  variant="ghost"
  size="icon"
  class="app-chrome-enter h-9 w-9 rounded-full border border-border/55 text-muted-foreground transition-colors hover:border-foreground/35 hover:text-foreground"
  onclick={() => (open = true)}
  title={`Search ${scopeLabel}`}
  aria-label={`Search ${scopeLabel}`}
  disabled={!enabled}
>
  <Search class="h-4 w-4" />
</Button>

<Dialog bind:open>
  <DialogContent
    closeOnInteractOutside={true}
    preventScroll={false}
    showCloseButton={false}
    class="top-[clamp(5.5rem,13vh,8.5rem)] flex w-[calc(100%-2rem)] max-w-5xl translate-y-0 flex-col gap-2 border-0 bg-transparent p-0 shadow-none"
  >
    <DialogTitle class="sr-only">Search {scopeLabel}</DialogTitle>

    <div
      class="flex h-[3.25rem] w-full items-center gap-3 border border-border bg-popover px-4 shadow-[0_16px_48px_rgba(0,0,0,0.14)] transition-[border-color,box-shadow] duration-150 focus-within:border-foreground/28 focus-within:shadow-[0_20px_60px_rgba(0,0,0,0.2)] dark:shadow-[0_20px_60px_rgba(0,0,0,0.42)] dark:focus-within:shadow-[0_24px_72px_rgba(0,0,0,0.52)]"
      aria-busy={isSearching}
    >
      {#if isSearching}
        <LoaderCircle
          class="h-4 w-4 shrink-0 animate-spin text-muted-foreground"
        />
      {:else}
        <Search class="h-4 w-4 shrink-0 text-muted-foreground" />
      {/if}
      <label class="sr-only" for="question-search-input">Search questions</label
      >
      <input
        id="question-search-input"
        bind:this={inputElement}
        bind:value={query}
        class="h-full min-w-0 flex-1 bg-transparent text-[0.98rem] font-medium tracking-[-0.012em] text-foreground outline-none placeholder:font-normal placeholder:text-muted-foreground/48"
        placeholder="Search"
        autocomplete="off"
        spellcheck="false"
      />
    </div>

    {#if showResultsPanel}
      <section
        id="question-search-results"
        class="relative flex max-h-[65dvh] min-h-0 w-full animate-in flex-col overflow-hidden border border-border bg-popover shadow-[0_24px_70px_rgba(0,0,0,0.16)] fade-in-0 slide-in-from-top-1 duration-150 dark:shadow-[0_28px_80px_rgba(0,0,0,0.45)]"
        aria-live="polite"
      >
        {#if error}
          <p class="px-5 py-4 text-sm text-destructive">{error}</p>
        {:else if response?.results.length === 0}
          <p class="px-5 py-4 text-sm text-muted-foreground">No results</p>
        {:else if response}
          <div
            bind:this={resultsScrollElement}
            class="min-h-0 overflow-y-auto no-scrollbar"
          >
            <div class="space-y-1.5 p-3 sm:p-4">
              {#each response.results as result, index (result.questionId)}
                {#if index === 0 || response.results[index - 1]?.matchStrength !== result.matchStrength}
                  <div
                    class="px-1 pb-1 pt-1 text-[0.61rem] font-bold uppercase tracking-[0.14em] text-muted-foreground/48"
                  >
                    {result.matchStrength === "strong"
                      ? "Strong matches"
                      : "Related results"}
                  </div>
                {:else}
                  <div
                    class="mx-0 h-px bg-border/45 sm:mx-2"
                    aria-hidden="true"
                  ></div>
                {/if}
                <article
                  class="grid gap-3 px-4 py-4 [content-visibility:auto] [contain-intrinsic-size:auto_10rem] sm:grid-cols-[7.25rem_minmax(0,1fr)] sm:gap-6 sm:px-5"
                >
                  <div class="flex items-baseline gap-2 sm:block">
                    {#if result.year}
                      <p
                        class="text-[0.84rem] font-semibold tabular-nums tracking-[-0.01em] text-foreground/78"
                      >
                        {result.year}
                      </p>
                    {/if}
                    <p
                      class="text-[0.62rem] font-bold uppercase tracking-[0.13em] text-muted-foreground/50 sm:mt-1.5"
                    >
                      {resultContext(result)}
                    </p>
                    {#if result.questionNumber != null}
                      <p
                        class="text-[0.62rem] font-bold uppercase tracking-[0.13em] text-muted-foreground/38 sm:mt-1"
                      >
                        Q {result.questionNumber}
                      </p>
                    {/if}
                  </div>

                  <div class="min-w-0">
                    <MathText
                      text={result.question}
                      class="text-[0.94rem] font-medium leading-[1.52] tracking-[-0.008em] text-foreground/88"
                    />

                    {#if result.options.length > 0}
                      <ol
                        class={`mt-3 grid grid-cols-1 gap-x-6 gap-y-1 text-[0.82rem] leading-relaxed text-foreground/66 sm:grid-cols-2 ${optionsFitSingleRow(result) ? "lg:grid-cols-4" : ""}`}
                      >
                        {#each result.options as option}
                          <li
                            class={`grid min-w-0 grid-cols-[auto_minmax(0,1fr)] items-baseline gap-1.5 ${optionsFitSingleRow(result) ? "lg:whitespace-nowrap" : ""}`}
                          >
                            <span
                              class="font-semibold uppercase text-muted-foreground/48"
                            >
                              ({option.id})
                            </span>
                            <MathText text={option.text} />
                          </li>
                        {/each}
                      </ol>
                    {/if}
                  </div>
                </article>
              {/each}
            </div>
          </div>
          <ScrollIndicator
            scroller={resultsScrollElement}
            right={2}
            updateTrigger={response}
          />
        {/if}
      </section>
    {/if}
  </DialogContent>
</Dialog>
