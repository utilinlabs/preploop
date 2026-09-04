<script lang="ts">
  import CatalogTile from "$lib/components/catalog-tile.svelte";
  import LoadingProgress from "$lib/components/loading-progress.svelte";
  import {
    PRELIMS_PAPER_TYPES,
    type MainsPaperType,
  } from "$lib/constants/upsc-catalog";
  import type {
    CatalogScreen,
    PaperListItem,
  } from "$lib/services/catalog-model";
  import type { StoredQuestionBank, TestAttemptHistoryEntry } from "$lib/types";
  import { formatDuration, formatMarks } from "$lib/utils";

  interface Props {
    screen: CatalogScreen;
    banks: StoredQuestionBank[];
    totalQuestions: number;
    prelimsCount: number;
    mainsCount: number;
    mainsPaperTypes: MainsPaperType[];
    prelimsPapers: PaperListItem[];
    mainsPapers: PaperListItem[];
    dualPaper1: PaperListItem[];
    dualPaper2: PaperListItem[];
    isDualPaper: boolean;
    historyEntries: TestAttemptHistoryEntry[];
    historyLoading: boolean;
    historyLoadingComplete: boolean;
    historyError: string | null;
    onHistoryLoadingComplete: () => void;
    onScreenChange: (screen: CatalogScreen) => void;
    onOpenHistory: () => void;
    onOpenResult: (id: string) => void;
    onOpenPrelim: (bank: StoredQuestionBank) => void;
    onOpenTheory: (item: PaperListItem) => void;
  }

  let {
    screen,
    banks,
    totalQuestions,
    prelimsCount,
    mainsCount,
    mainsPaperTypes,
    prelimsPapers,
    mainsPapers,
    dualPaper1,
    dualPaper2,
    isDualPaper,
    historyEntries,
    historyLoading,
    historyLoadingComplete,
    historyError,
    onHistoryLoadingComplete,
    onScreenChange,
    onOpenHistory,
    onOpenResult,
    onOpenPrelim,
    onOpenTheory,
  }: Props = $props();

  const formatDate = (timestamp: number) =>
    new Intl.DateTimeFormat(undefined, { dateStyle: "medium" }).format(
      new Date(timestamp),
    );
  const formatScore = (score: number, max: number) =>
    max > 0
      ? `${formatMarks(score)} / ${formatMarks(max)}`
      : formatMarks(score);
  let dualPaperGroups = $derived<{ label: string; items: PaperListItem[] }[]>([
    { label: "Paper I", items: dualPaper1 },
    { label: "Paper II", items: dualPaper2 },
  ]);
  const mainsDescription = $derived(
    mainsPaperTypes.some((paper) => paper.optional)
      ? "Essay · General Studies · Optional"
      : "Essay · General Studies",
  );
</script>

{#if screen.kind === "home"}
  <section
    class="catalog-grid-start catalog-grid-frame mx-auto w-full max-w-3xl"
  >
    <div class="catalog-tile-grid grid grid-cols-1">
      <CatalogTile
        title="Prelims"
        eyebrow="Objective Stage"
        description="General Studies · CSAT"
        asideValue={String(prelimsCount)}
        asideLabel="Papers"
        size="lg"
        index={0}
        onclick={() => onScreenChange({ kind: "prelims" })}
      />
      <CatalogTile
        title="Mains"
        eyebrow="Written Stage"
        description={mainsDescription}
        asideValue={String(mainsCount)}
        asideLabel="Papers"
        size="lg"
        index={1}
        onclick={() => onScreenChange({ kind: "mains" })}
      />
    </div>
    <div
      class="app-surface-enter mt-6 flex items-center justify-center gap-4 text-muted-foreground/55"
      style="--enter-delay: 110ms;"
    >
      <span class="text-[0.68rem] font-bold uppercase tracking-[0.15em]"
        ><strong
          class="mr-1.5 text-[0.82rem] font-semibold tabular-nums text-foreground/75"
          >{banks.length}</strong
        >Papers</span
      >
      <span
        class="h-1 w-1 rounded-full bg-muted-foreground/30"
        aria-hidden="true"
      ></span>
      <span class="text-[0.68rem] font-bold uppercase tracking-[0.15em]"
        ><strong
          class="mr-1.5 text-[0.82rem] font-semibold tabular-nums text-foreground/75"
          >{totalQuestions.toLocaleString("en-IN")}</strong
        >Questions</span
      >
    </div>
  </section>
{:else if screen.kind === "mains"}
  <div
    class="catalog-grid-start mx-auto grid w-full max-w-4xl content-center grid-cols-1 gap-1.5 sm:grid-cols-2 sm:gap-[0.4375rem] lg:grid-cols-3"
  >
    {#each mainsPaperTypes as paper, index (paper.id)}
      <CatalogTile
        title={paper.label}
        description={paper.description}
        size="md"
        class="h-[14.5rem] min-h-[14.5rem]"
        {index}
        onclick={() => onScreenChange({ kind: "mains-paper", paper })}
      />
    {/each}
  </div>
{:else if screen.kind === "prelims"}
  <div
    class="catalog-grid-start catalog-grid-frame relative -top-6 mx-auto w-full max-w-3xl"
  >
    <div class="catalog-tile-grid grid grid-cols-1 sm:grid-cols-2">
      {#each PRELIMS_PAPER_TYPES as paper, index (paper.id)}
        <CatalogTile
          title={paper.label}
          description={paper.description}
          asideValue={paper.id === "gs1" ? "I" : "II"}
          asideLabel="Paper"
          size="md"
          class="h-[17rem] min-h-[17rem]"
          {index}
          onclick={() => onScreenChange({ kind: "prelims-paper", paper })}
        />
      {/each}
    </div>
  </div>
{:else if screen.kind === "prelims-history"}
  <section
    class="catalog-grid-start catalog-grid-frame mx-auto w-full max-w-3xl"
  >
    {#if historyLoading}
      <LoadingProgress
        class="min-h-64 w-full"
        complete={historyLoadingComplete}
        onComplete={onHistoryLoadingComplete}
      />
    {:else if historyError}
      <div
        class="flex min-h-64 flex-col items-center justify-center gap-4 text-center"
      >
        <p class="text-sm text-destructive">{historyError}</p>
        <button
          type="button"
          class="rounded-full border border-border/75 px-4 py-2 text-xs font-semibold uppercase tracking-[0.14em] text-muted-foreground transition-colors hover:border-foreground/45 hover:text-foreground"
          onclick={onOpenHistory}>Retry</button
        >
      </div>
    {:else if historyEntries.length === 0}
      <div class="flex min-h-64 items-center justify-center text-center">
        <p class="text-sm text-muted-foreground">
          No completed test attempts yet.
        </p>
      </div>
    {:else}
      <div class="history-list">
        {#each historyEntries as entry, index (entry.id)}
          {#if index > 0}
            <div
              class="mx-0 h-px bg-border/45 sm:mx-2"
              aria-hidden="true"
            ></div>
          {/if}
          <button
            type="button"
            class="history-entry app-surface-enter grid grid-cols-[minmax(0,1fr)_auto] items-center gap-4 text-left sm:grid-cols-[minmax(0,0.9fr)_minmax(0,2fr)_auto]"
            style={`--enter-delay: ${Math.min(index * 35, 280)}ms;`}
            onclick={() => onOpenResult(entry.id)}
          >
            <span class="history-entry-date"
              >{formatDate(entry.completedAt)}</span
            >
            <span class="history-entry-paper col-span-2 sm:col-span-1"
              >{entry.paper}</span
            >
            <span class="history-entry-score"
              >{formatScore(entry.score, entry.maxScore)}</span
            >
          </button>
        {/each}
      </div>
    {/if}
  </section>
{:else if screen.kind === "prelims-paper"}
  {#if prelimsPapers.length === 0}
    <div
      class="catalog-grid-start catalog-grid-frame mx-auto w-full max-w-5xl text-center"
    >
      <p class="text-sm text-muted-foreground">No papers in this section.</p>
    </div>
  {:else}
    <div class="catalog-grid-start catalog-grid-frame mx-auto w-full max-w-5xl">
      <div
        class="catalog-tile-grid grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-5"
      >
        {#each prelimsPapers as item, index (item.bank.id)}
          <CatalogTile
            title={item.label}
            description={`${item.bank.totalQuestions} questions`}
            meta={formatDuration(item.bank.defaultDuration)}
            size="sm"
            {index}
            onclick={() => onOpenPrelim(item.bank)}
          />
        {/each}
      </div>
    </div>
  {/if}
{:else if screen.kind === "mains-paper" && isDualPaper}
  {#if dualPaper1.length === 0 && dualPaper2.length === 0}
    <div
      class="catalog-grid-start catalog-grid-frame mx-auto w-full max-w-5xl text-center"
    >
      <p class="text-sm text-muted-foreground">No papers in this section.</p>
    </div>
  {:else}
    <div
      class="catalog-grid-start catalog-grid-start--dual catalog-grid-frame mx-auto w-full max-w-5xl"
    >
      {#each dualPaperGroups as { label, items } (label)}
        {#if items.length > 0}
          <section class="catalog-paper-section">
            <h2
              class="mb-3.5 text-center text-[0.72rem] font-bold uppercase tracking-[0.16em] text-muted-foreground/55"
            >
              {label}
            </h2>
            <div
              class="catalog-tile-grid grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-5"
            >
              {#each items as item, index (item.bank.id)}
                <CatalogTile
                  title={item.label}
                  description={`${item.bank.totalQuestions} questions`}
                  meta={formatDuration(item.bank.defaultDuration)}
                  size="sm"
                  {index}
                  onclick={() => onOpenTheory(item)}
                />
              {/each}
            </div>
          </section>
        {/if}
      {/each}
    </div>
  {/if}
{:else if screen.kind === "mains-paper"}
  {#if mainsPapers.length === 0}
    <div
      class="catalog-grid-start catalog-grid-frame mx-auto w-full max-w-5xl text-center"
    >
      <p class="text-sm text-muted-foreground">No papers in this section.</p>
    </div>
  {:else}
    <div class="catalog-grid-start catalog-grid-frame mx-auto w-full max-w-5xl">
      <div
        class="catalog-tile-grid grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-5"
      >
        {#each mainsPapers as item, index (item.bank.id)}
          <CatalogTile
            title={item.label}
            description={`${item.bank.totalQuestions} questions`}
            meta={formatDuration(item.bank.defaultDuration)}
            size="sm"
            {index}
            onclick={() => onOpenTheory(item)}
          />
        {/each}
      </div>
    </div>
  {/if}
{/if}
