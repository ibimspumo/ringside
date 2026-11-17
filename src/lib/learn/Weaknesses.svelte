<script lang="ts">
  import type { ConceptCount } from "../coach/types";
  import type { LearnKind, LearnProgress } from "../types";
  import { SKILLS, nextLevel, trainableKind, weakConcepts } from "./catalog";
  import { ARTICLE_BY_ID } from "./wissen/articles";

  /** Concepts the coach found shaky in stored reports, weakest first. Renders nothing without them. */
  let {
    concepts,
    progress,
    limit = 4,
    onTrain,
    onRead,
  }: {
    concepts: ConceptCount[];
    progress: LearnProgress | null;
    limit?: number;
    onTrain: (kind: LearnKind, level: number) => void;
    onRead: (id: string) => void;
  } = $props();

  const shown = $derived(weakConcepts(concepts).slice(0, limit));

  function counts(c: ConceptCount): string {
    const parts = [];
    if (c.mistakes) parts.push(`${c.mistakes} Fehler`);
    if (c.questionable) parts.push(`${c.questionable} fraglich`);
    parts.push(`${c.spots} ${c.spots === 1 ? "Spot" : "Spots"} in ${c.tournaments} ${c.tournaments === 1 ? "Turnier" : "Turnieren"}`);
    return parts.join(" · ");
  }
</script>

{#if shown.length}
  <section class="weak panel" aria-labelledby="weak-title">
    <header>
      <h2 id="weak-title">Deine Schwachstellen</h2>
      <span class="faint small">Aus deinen Coach-Berichten, die schwächste zuerst</span>
    </header>
    <ol>
      {#each shown as c, i (c.id)}
        {@const kind = trainableKind(c)}
        {@const article = c.articles.map((a) => ARTICLE_BY_ID[a]).find(Boolean)}
        <li>
          <span class="rank num" aria-hidden="true">{i + 1}</span>
          <span class="name">
            <b>{c.name}</b>
            <span class="faint small">{counts(c)}</span>
          </span>
          <span class="go">
            {#if kind}
              <button class="btn" onclick={() => onTrain(kind, nextLevel(progress?.skills.find((s) => s.id === kind)))}>
                Üben: {SKILLS[kind].short}
              </button>
            {/if}
            {#if article}
              <button class="btn ghost" onclick={() => onRead(article.id)}>{article.title}</button>
            {/if}
          </span>
        </li>
      {/each}
    </ol>
  </section>
{/if}

<style>
  .weak {
    display: grid;
    gap: 10px;
    padding: 16px 18px;
  }
  header {
    display: flex;
    align-items: baseline;
    flex-wrap: wrap;
    gap: 4px 12px;
  }
  h2 {
    margin: 0;
    font-size: 16px;
    font-weight: 600;
  }
  .small {
    font-size: 12.5px;
  }
  ol {
    display: grid;
    gap: 6px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  li {
    display: grid;
    grid-template-columns: 22px minmax(0, 1fr) auto;
    align-items: center;
    gap: 12px;
    padding: 9px 10px;
    border-radius: var(--r-sm);
    background: var(--surface-2);
  }
  .rank {
    color: var(--text-3);
    font-weight: 600;
    text-align: center;
  }
  .name {
    display: grid;
    gap: 2px;
    min-width: 0;
  }
  .go {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 6px;
  }
  @media (max-width: 640px) {
    li {
      grid-template-columns: 22px minmax(0, 1fr);
    }
    .go {
      grid-column: 2;
      justify-content: flex-start;
    }
  }
</style>
