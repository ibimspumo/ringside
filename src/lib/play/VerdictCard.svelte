<script lang="ts">
  import Term from "../solver/Term.svelte";
  import { GRADE, THRESHOLD_TEXT, choiceColors, choiceLabel, num, reason, sourceLine } from "./labels";
  import type { HandView, Lesson, Verdict } from "./types";

  /** A graded move, or (without `chosen`) the hint before the move. */
  let { view, lesson, title }: { view: HandView; lesson: Verdict | Lesson; title?: string } = $props();

  const verdict = $derived("chosen" in lesson ? (lesson as Verdict) : null);
  const j = $derived(verdict?.judgement ?? null);
  const colors = $derived(choiceColors(lesson.choices));
  const why = $derived(reason(lesson, verdict?.chosen ?? null, view.hero_cards));
  const best = $derived(j?.best ?? (lesson.strategy ? lesson.strategy.indexOf(Math.max(...lesson.strategy)) : -1));
  const stack = $derived(view.stack_bb);
</script>

<article class="verdict" style:--g={j ? GRADE[j.grade].color : "var(--focus)"} aria-live="polite">
  <header>
    {#if j}
      <span class="grade">{GRADE[j.grade].label}</span>
      {#if verdict?.hinted}<span class="tag">mit Tipp · zählt halb</span>{/if}
    {:else if verdict}
      <span class="grade none">Nicht bewertet</span>
    {:else}
      <span class="grade hint">{title ?? "Tipp"}</span>
    {/if}
    <span class="street">{lesson.street === "preflop" ? "Preflop" : lesson.street === "flop" ? "Flop" : lesson.street === "turn" ? "Turn" : "River"} · {lesson.class}</span>
  </header>

  {#if j}<p class="lead">{GRADE[j.grade].lead}</p>{/if}
  <p class="why">{why}</p>

  {#if lesson.strategy}
    <div class="bars" role="table" aria-label="Strategie der Lösung für deine Hand">
      <div class="row head" role="row">
        <span role="columnheader">Zug</span>
        <span role="columnheader">So oft spielt es die Lösung</span>
        {#if lesson.evs}<span role="columnheader" class="ev"><Term term="ev">EV</Term></span>{/if}
      </div>
      {#each lesson.choices as c, k (k)}
        {@const f = lesson.strategy[k] ?? 0}
        {@const l = choiceLabel(c)}
        <div class="row" role="row" class:chosen={verdict?.chosen === k} class:best={k === best}>
          <span class="label" role="cell">
            {l.label}{#if l.size}<small> {l.size}</small>{/if}
            {#if verdict?.chosen === k}<em>deine Wahl</em>{/if}
          </span>
          <span class="bar" role="cell" aria-label="{Math.round(f * 100)} %">
            <i style="width:{Math.max(f * 100, f > 0 ? 1.5 : 0)}%; background:{colors[k]}"></i>
            <b class="num">{Math.round(f * 100)} %</b>
          </span>
          {#if lesson.evs}<span class="ev num" role="cell">{num(lesson.evs[k], 2)}</span>{/if}
        </div>
      {/each}
    </div>
  {/if}

  {#if j && j.loss_bb !== null}
    <p class="loss">
      <Term term="evloss">EV-Verlust</Term>
      <b class="num">{num(j.loss_bb, 2)} BB</b>
      <span class="faint num">= {num(j.loss_pct ?? 0, 1)} % des Pots</span>
      <span class="faint rule" title={THRESHOLD_TEXT[j.grade]}>· {GRADE[j.grade].label}: {THRESHOLD_TEXT[j.grade]}</span>
    </p>
  {:else if j}
    <p class="loss faint">Hier sind nur Häufigkeiten gespeichert: bewertet wird danach, wie oft die Lösung deinen Zug spielt.</p>
  {/if}

  <p class="source">{sourceLine(lesson.source, view.config, stack, lesson.exploitability_pct)}</p>
</article>

<style>
  .verdict {
    position: relative;
    display: grid;
    gap: 10px;
    padding: 16px 18px 14px 20px;
    border-radius: var(--r);
    background: var(--surface);
    border: 1px solid var(--line-2);
    overflow: hidden;
    animation: rise 0.22s ease-out;
  }
  .verdict::before {
    content: "";
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    width: 4px;
    background: var(--g);
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
  }
  header {
    display: flex;
    align-items: baseline;
    gap: 10px;
    flex-wrap: wrap;
  }
  .grade {
    font-size: 21px;
    font-weight: 700;
    color: var(--g);
    letter-spacing: -0.01em;
  }
  .grade.none,
  .grade.hint {
    color: var(--text);
  }
  .tag {
    font-size: 11px;
    padding: 1px 8px;
    border-radius: 999px;
    background: var(--surface-3);
    color: var(--text-2);
  }
  .street {
    margin-left: auto;
    font-size: 12px;
    color: var(--text-3);
  }
  p {
    margin: 0;
  }
  .lead {
    font-size: 15px;
    font-weight: 600;
  }
  .why {
    color: var(--text-2);
    max-width: 68ch;
  }
  .bars {
    display: grid;
    gap: 4px;
  }
  .row {
    display: grid;
    grid-template-columns: minmax(150px, 1.1fr) 2fr 64px;
    align-items: center;
    gap: 10px;
    font-size: 13px;
  }
  .row.head {
    font-size: 11px;
    color: var(--text-3);
  }
  .label small {
    color: var(--text-3);
    margin-left: 4px;
  }
  .label em {
    font-style: normal;
    font-size: 11px;
    margin-left: 6px;
    padding: 0 6px;
    border-radius: 999px;
    box-shadow: inset 0 0 0 1px var(--g);
    color: var(--text);
  }
  .best .label {
    font-weight: 600;
  }
  .bar {
    position: relative;
    height: 20px;
    border-radius: 4px;
    background: var(--surface-2);
    overflow: hidden;
  }
  .bar i {
    position: absolute;
    inset: 0 auto 0 0;
    border-radius: 4px;
  }
  .bar b {
    position: absolute;
    right: 6px;
    top: 50%;
    transform: translateY(-50%);
    font-size: 12px;
    font-weight: 500;
    color: var(--text);
    text-shadow: 0 1px 2px rgba(0, 0, 0, 0.6);
  }
  .ev {
    text-align: right;
    color: var(--text-2);
  }
  .chosen .ev {
    color: var(--text);
  }
  .loss {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 6px;
    font-size: 13px;
  }
  .rule {
    font-size: 12px;
  }
  .source {
    font-size: 11.5px;
    color: var(--text-3);
  }
  @media (prefers-reduced-motion: reduce) {
    .verdict {
      animation: none;
    }
  }
</style>
