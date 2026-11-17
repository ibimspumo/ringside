<script lang="ts">
  import type { ConceptCount } from "../coach/types";
  import type { LearnKind, LearnProgress } from "../types";
  import { SKILLS, nextLevel, trainableKind, weakConcepts } from "./catalog";
  import SkillBadge from "./SkillBadge.svelte";
  import { ARTICLE_BY_ID } from "./wissen/articles";

  /** Concepts the coach found shaky in stored reports, weakest first. Renders nothing without them. */
  let {
    concepts,
    progress,
    limit = 4,
    primary = false,
    onTrain,
    onRead,
  }: {
    concepts: ConceptCount[];
    progress: LearnProgress | null;
    limit?: number;
    /** The first card's practice button is the page's main action. */
    primary?: boolean;
    onTrain: (kind: LearnKind, level: number) => void;
    onRead: (id: string) => void;
  } = $props();

  const shown = $derived(weakConcepts(concepts).slice(0, limit));

  function counts(c: ConceptCount): string {
    const parts = [];
    if (c.mistakes) parts.push(`${c.mistakes} Fehler`);
    if (c.questionable) parts.push(`${c.questionable} ${c.questionable === 1 ? "fragliche Entscheidung" : "fragliche Entscheidungen"}`);
    const where = `in ${c.tournaments} ${c.tournaments === 1 ? "Turnier" : "Turnieren"}`;
    return parts.length ? `${parts.join(" und ")} ${where}` : `${c.spots} ${c.spots === 1 ? "Spot" : "Spots"} ${where}`;
  }
</script>

{#if shown.length}
  <section class="weak" aria-labelledby="weak-title">
    <header>
      <h2 id="weak-title">Deine Schwachstellen</h2>
      <span class="dim small">Aus deinen Coach-Berichten. Hier lohnt sich Üben gerade am meisten.</span>
    </header>
    <ol>
      {#each shown as c, i (c.id)}
        {@const kind = trainableKind(c)}
        {@const article = c.articles.map((a) => ARTICLE_BY_ID[a]).find(Boolean)}
        {@const skill = kind ? progress?.skills.find((s) => s.id === kind) : undefined}
        <li class="card">
          <div class="top">
            {#if kind}<SkillBadge {kind} {skill} size={38} />{:else}<span class="rank num" aria-hidden="true">{i + 1}</span>{/if}
            <span class="name">
              <b>{kind ? SKILLS[kind].short : c.name}</b>
              <span class="dim small">{counts(c)}</span>
            </span>
          </div>
          <div class="go">
            {#if kind}
              <button class="btn" class:primary={primary && i === 0} onclick={() => onTrain(kind, nextLevel(skill))}>
                Üben
                <svg viewBox="0 0 24 24" width="14" height="14" aria-hidden="true"><path d="M5 12h14M13 6l6 6-6 6" /></svg>
              </button>
            {/if}
            {#if article}
              <button class="link" onclick={() => onRead(article.id)}>Nachlesen: {article.title}</button>
            {/if}
          </div>
        </li>
      {/each}
    </ol>
  </section>
{/if}

<style>
  .weak {
    display: grid;
    gap: 10px;
  }
  header {
    display: flex;
    align-items: baseline;
    flex-wrap: wrap;
    gap: 4px 12px;
  }
  h2 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
  }
  .small {
    font-size: 12.5px;
  }
  ol {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
    gap: 10px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .card {
    display: grid;
    align-content: space-between;
    gap: 14px;
    padding: 14px 16px 14px 14px;
    border-radius: var(--r);
    border: 1px solid var(--line);
    border-top: 2px solid color-mix(in srgb, var(--neg) 70%, transparent);
    background: var(--surface);
  }
  .top {
    display: flex;
    align-items: center;
    gap: 12px;
    min-width: 0;
  }
  .rank {
    display: grid;
    place-items: center;
    width: 38px;
    height: 38px;
    flex-shrink: 0;
    border-radius: 50%;
    background: var(--surface-3);
    color: var(--text-2);
    font-weight: 600;
  }
  .name {
    display: grid;
    gap: 2px;
    min-width: 0;
  }
  .name b {
    font-size: 14px;
  }
  .go {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 16px;
  }
  .link {
    padding: 0;
    border: 0;
    background: none;
    color: var(--text-2);
    font-size: 13px;
    text-decoration: underline;
    text-decoration-color: var(--line-2);
    text-underline-offset: 3px;
    cursor: pointer;
  }
  .link:hover {
    color: var(--text);
  }
  svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
</style>
