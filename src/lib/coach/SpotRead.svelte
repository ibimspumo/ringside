<script lang="ts">
  import ArticleView from "../learn/wissen/Article.svelte";
  import { ARTICLE_BY_ID } from "../learn/wissen/articles";
  import { loadRead, storeRead } from "../learn/wissen/read";
  import PlayingCard from "../PlayingCard.svelte";
  import CardText from "./CardText.svelte";
  import { conceptName } from "./concepts";
  import Figures from "./Figures.svelte";
  import { positionHint, verdictOf } from "./meaning";
  import type { Practice } from "./ReportView.svelte";
  import type { CoachSpot } from "./types";

  let {
    spot,
    n,
    total,
    title,
    practice,
    onBack,
    onDrill,
  }: {
    spot: CoachSpot;
    n: number;
    total: number;
    title: string;
    practice: Practice;
    onBack: () => void;
    onDrill: () => void;
  } = $props();

  let chosen = $state<string | null>(null);
  const current = $derived(chosen ?? practice.articles[0]?.id ?? null);
  let read = $state(loadRead());
  const article = $derived(current ? ARTICLE_BY_ID[current] : null);
  const verdict = $derived(verdictOf(spot.verdict));
  const stack = $derived(spot.stack_bb.toLocaleString("de-DE", { maximumFractionDigits: 1 }));

  function toggleRead(id: string) {
    const next = new Set(read);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    read = next;
    storeRead(next);
  }

  let top = $state<HTMLElement>();
  function open(id: string) {
    chosen = id;
    top?.scrollIntoView({ block: "start", behavior: "smooth" });
  }
</script>

<div class="read" bind:this={top}>
  <section class="panel mine v-{spot.verdict}" aria-labelledby="mine-title">
    <header class="top">
      <button class="btn ghost back" onclick={onBack}>
        <svg viewBox="0 0 24 24" width="14" height="14" aria-hidden="true"><path d="M19 12H5M11 6l-6 6 6 6" /></svg>
        Zurück zum Bericht
      </button>
      <span class="where">{title} · Entscheidung <span class="num">{n}</span> von <span class="num">{total}</span></span>
    </header>
    <div class="body">
      <div class="hand">
        <span class="cards">{#each spot.hero_cards as c (c)}<PlayingCard card={c} size={30} />{/each}</span>
        {#if spot.board.length}
          <span class="cards board">{#each spot.board as c (c)}<PlayingCard card={c} size={22} />{/each}</span>
        {/if}
        <span class="meta"
          ><abbr title={positionHint(spot.position)}>{spot.position ?? "–"}</abbr> · <span class="num">{stack} BB</span></span
        >
      </div>
      <div class="text">
        <p class="eyebrow">Dein Spot zum Thema {conceptName(spot.concept)} · <span class="vname">{verdict.name}</span></p>
        <h2 id="mine-title"><CardText text={spot.action} /></h2>
        {#if spot.better}<p class="better"><span>Besser</span><b><CardText text={spot.better} /></b></p>{/if}
        {#if spot.numbers.length}
          <p class="lead">
            Deine Zahlen aus diesem Spot. Der Artikel unten rechnet mit eigenen Beispielen – vergleiche sie mit deinen:
          </p>
          <Figures numbers={spot.numbers} />
        {/if}
        <div class="row">
          <button class="btn" onclick={onDrill}>Diesen Spot üben</button>
          {#if practice.articles.length > 1}
            <div class="seg" role="tablist" aria-label="Artikel zum Thema">
              {#each practice.articles as a (a.id)}
                <button role="tab" aria-selected={current === a.id} class:on={current === a.id} onclick={() => (chosen = a.id)}
                  >{a.title}</button
                >
              {/each}
            </div>
          {/if}
        </div>
      </div>
    </div>
  </section>

  {#if article}
    <p class="divider"><span>Allgemeine Erklärung mit eigenen Beispielen</span></p>
    <ArticleView {article} read={read.has(article.id)} onToggleRead={() => toggleRead(article.id)} onOpen={open} />
  {/if}
</div>

<style>
  .read {
    display: flex;
    flex-direction: column;
    gap: 16px;
    scroll-margin-top: 16px;
  }
  .mine {
    --accent: var(--text-3);
    padding: 14px 22px 20px;
    box-shadow: inset 3px 0 0 var(--accent);
  }
  .v-fehler {
    --accent: var(--neg);
  }
  .v-fraglich {
    --accent: var(--amber);
  }
  .v-stark {
    --accent: var(--pos);
  }
  .top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 8px 16px;
    margin-left: -10px;
  }
  .back svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .where {
    font-size: 12.5px;
    color: var(--text-2);
  }
  .body {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    gap: 22px;
    margin-top: 12px;
  }
  .hand {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 8px;
  }
  .cards {
    display: flex;
    gap: 3px;
  }
  .meta {
    font-size: 13px;
    color: var(--text-2);
  }
  abbr {
    text-decoration: underline dotted var(--text-3);
    text-underline-offset: 3px;
    cursor: help;
  }
  .text {
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-width: 0;
  }
  .eyebrow {
    margin: 0;
    font-size: 12.5px;
    color: var(--text-2);
  }
  .vname {
    color: var(--accent);
    font-weight: 600;
  }
  h2 {
    margin: 0;
    font-size: 17px;
    font-weight: 600;
    line-height: 1.4;
  }
  .better {
    display: flex;
    align-items: baseline;
    gap: 10px;
    align-self: flex-start;
    margin: 0;
    padding: 6px 12px 6px 10px;
    border-radius: var(--r-sm);
    background: var(--surface-2);
    border-left: 2px solid var(--text);
  }
  .better span {
    font-size: 12px;
    color: var(--text-3);
  }
  .lead {
    margin: 4px 0 0;
    font-size: 13px;
    color: var(--text-2);
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
    margin-top: 4px;
  }

  .divider {
    display: flex;
    align-items: center;
    gap: 12px;
    margin: 8px 0 -4px;
    font-size: 12.5px;
    color: var(--text-2);
  }
  .divider::before,
  .divider::after {
    content: "";
    flex: 1;
    height: 1px;
    background: var(--line-2);
  }

  @container (max-width: 700px) {
    .body {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
