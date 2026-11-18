<script lang="ts">
  import ArticleView from "../learn/wissen/Article.svelte";
  import { ARTICLE_BY_ID } from "../learn/wissen/articles";
  import { loadRead, storeRead } from "../learn/wissen/read";
  import Button from "../ui/Button.svelte";
  import Card from "../ui/Card.svelte";
  import GradeBadge from "../ui/GradeBadge.svelte";
  import PlayingCard from "../ui/PlayingCard.svelte";
  import SegmentedControl from "../ui/SegmentedControl.svelte";
  import CardText from "../ui/CardText.svelte";
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

<svelte:window onkeydown={(e) => e.key === "Escape" && onBack()} />

{#snippet backIcon()}<svg viewBox="0 0 24 24" width="14" height="14" aria-hidden="true"><path d="M19 12H5M11 6l-6 6 6 6" /></svg>{/snippet}

<div class="read" bind:this={top}>
  <Card pad="lg" aria-labelledby="mine-title">
    <div class="mine" style:--accent="var(--grade-{verdict.grade})">
    <header class="top">
      <Button variant="quiet" icon={backIcon} kbd="Esc" onclick={onBack}>Zurück zum Bericht</Button>
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
        <p class="eyebrow">Dein Spot zum Thema {conceptName(spot.concept)} <GradeBadge grade={verdict.grade} size="sm" /></p>
        <h2 id="mine-title"><CardText hands text={spot.action} /></h2>
        {#if spot.better}<p class="better"><span>Besser</span><b><CardText hands text={spot.better} /></b></p>{/if}
        {#if spot.numbers.length}
          <p class="lead">
            Deine Zahlen aus diesem Spot. Der Artikel unten rechnet mit eigenen Beispielen – vergleiche sie mit deinen:
          </p>
          <Figures numbers={spot.numbers} />
        {/if}
        <div class="row">
          <Button onclick={onDrill}>Diesen Spot üben</Button>
          {#if practice.articles.length > 1 && current}
            <SegmentedControl
              options={practice.articles.map((a) => ({ value: a.id, label: a.title }))}
              value={current}
              label="Artikel zum Thema"
              onchange={(id) => (chosen = id)}
            />
          {/if}
        </div>
      </div>
    </div>
    </div>
  </Card>

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
    padding-left: var(--sp-3);
    box-shadow: inset 3px 0 0 var(--accent);
  }
  .top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 8px 16px;
    margin: calc(-1 * var(--sp-2)) 0 0 calc(-1 * var(--sp-3));
  }
  svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .where {
    font-size: var(--fs-md);
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
    font-size: var(--fs-md);
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
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    margin: 0;
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  h2 {
    margin: 0;
    font-size: var(--fs-lg);
    font-weight: var(--fw-semibold);
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
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .lead {
    margin: 4px 0 0;
    font-size: var(--fs-md);
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
    font-size: var(--fs-md);
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
