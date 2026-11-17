<script lang="ts">
  import type { Snippet } from "svelte";
  import type { Feedback } from "../types";
  import { GRADING, explain, fmtBB } from "./catalog";
  import { preview } from "./preview";
  import Ring from "./Ring.svelte";

  let {
    feedback,
    onNext,
    nextLabel = "Weiter",
    children,
  }: { feedback: Feedback; onNext?: () => void; nextLabel?: string; children?: Snippet } = $props();

  const verdict = $derived(feedback.correct ? "ok" : feedback.score >= 0.5 ? "close" : "miss");
  const TITLE = { ok: "Richtig", close: "Knapp daneben", miss: "Daneben" };
  const tone = $derived(verdict === "ok" ? "var(--pos)" : verdict === "close" ? "var(--amber)" : "var(--neg)");
  const g = $derived(GRADING[feedback.grading]);
  let nextBtn = $state<HTMLButtonElement>();
  let root = $state<HTMLElement>();

  // The explanation comes into view and the next step is one key press away.
  $effect(() => {
    const calm = matchMedia("(prefers-reduced-motion: reduce)").matches || preview("answer") !== null;
    root?.scrollIntoView({ block: "nearest", behavior: calm ? "auto" : "smooth" });
    nextBtn?.focus({ preventScroll: true });
  });
</script>

<section class="fb {verdict}" style="--tone:{tone}" aria-live="polite" bind:this={root}>
  <header>
    <span class="icon" aria-hidden="true">
      <svg viewBox="0 0 24 24" width="22" height="22">
        {#if verdict === "ok"}
          <path d="M5 12.5l4.5 4.5L19 7.5" />
        {:else if verdict === "close"}
          <path d="M6 12h12" />
        {:else}
          <path d="M7 7l10 10M17 7 7 17" />
        {/if}
      </svg>
    </span>
    <div class="what">
      <h3>{TITLE[verdict]}</h3>
      <p>Lösung: <b class="num">{feedback.truth_text}</b></p>
    </div>
    <Ring value={feedback.score} size={54} stroke={5} {tone} label="{Math.round(feedback.score * 100)} von 100 Punkten">
      <b class="num score">{Math.round(feedback.score * 100)}</b>
    </Ring>
    {#if onNext}
      <button class="btn primary next" bind:this={nextBtn} onclick={onNext}>{nextLabel} <kbd>Enter</kbd></button>
    {/if}
  </header>

  {#if feedback.ev_loss_bb !== null}
    <div class="ev" class:none={feedback.ev_loss_bb <= 0.005} title={explain("EV-Verlust")}>
      {#if feedback.ev_loss_bb <= 0.005}
        Beste Entscheidung: Sie kostet dich auf Dauer nichts.
      {:else}
        Deine Wahl kostet im Schnitt <b class="num">{fmtBB(feedback.ev_loss_bb)}</b> pro Hand gegenüber der besten.
      {/if}
    </div>
  {/if}

  {#if feedback.facts.length}
    <dl class="facts">
      {#each feedback.facts as f, i (i)}
        <div class="fact" style="--i:{i}">
          <dt>{#if explain(f.label)}<span class="term" title={explain(f.label)}>{f.label}</span>{:else}{f.label}{/if}</dt>
          <dd class="num">{f.value}</dd>
          {#if f.formula}<dd class="formula num">{f.formula}</dd>{/if}
        </div>
      {/each}
    </dl>
  {/if}

  {@render children?.()}

  <footer class="grading" title={g.text}>
    <span class="badge {feedback.grading}">{g.label}</span>
    <span class="faint">{feedback.model_note ?? g.text}</span>
  </footer>
</section>

<style>
  .fb {
    display: grid;
    gap: 14px;
    padding: 16px 18px;
    border-radius: var(--r);
    border: 1px solid color-mix(in srgb, var(--tone) 45%, transparent);
    background: linear-gradient(180deg, color-mix(in srgb, var(--tone) 9%, var(--surface)) 0%, var(--surface) 70%);
    animation: rise 0.32s cubic-bezier(0.2, 0.8, 0.2, 1) both;
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(10px);
    }
  }
  header {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto auto;
    align-items: center;
    gap: 14px;
  }
  .icon {
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    border-radius: 50%;
    background: var(--tone);
    color: var(--bg);
    animation: bump 0.45s cubic-bezier(0.2, 0.9, 0.3, 1.5) both;
  }
  .miss .icon {
    animation-name: nudge;
  }
  @keyframes bump {
    from {
      transform: scale(0.4);
    }
  }
  @keyframes nudge {
    20% {
      transform: translateX(-4px);
    }
    60% {
      transform: translateX(4px);
    }
  }
  .icon svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 2.6;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  h3 {
    margin: 0;
    font-family: var(--font-num);
    font-size: 20px;
    font-weight: 600;
    color: var(--tone);
  }
  .what p {
    margin: 2px 0 0;
    color: var(--text-2);
  }
  .what b {
    color: var(--text);
    font-size: 15px;
  }
  .score {
    font-size: 16px;
  }
  .ev {
    justify-self: start;
    padding: 4px 10px;
    border-radius: 99px;
    background: rgba(229, 72, 59, 0.12);
    color: #ff9d93;
    font-size: 13px;
  }
  .ev.none {
    background: rgba(120, 193, 66, 0.12);
    color: var(--pos);
  }
  .facts {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
    gap: 8px;
    margin: 0;
  }
  .fact {
    padding: 8px 10px;
    border-radius: var(--r-sm);
    background: var(--surface-2);
    animation: rise 0.3s ease-out both;
    animation-delay: calc(var(--i) * 45ms + 0.1s);
  }
  dt {
    font-size: 12px;
    color: var(--text-3);
  }
  dd {
    margin: 0;
    font-size: 16px;
    font-weight: 600;
  }
  .formula {
    margin-top: 2px;
    font-size: 12px;
    font-weight: 400;
    color: var(--text-2);
    overflow-wrap: anywhere;
  }
  .next {
    height: 40px;
    padding: 0 18px;
    font-size: 14px;
  }
  .term {
    text-decoration: underline dotted;
    text-decoration-color: var(--text-3);
    text-underline-offset: 3px;
    cursor: help;
  }
  .grading {
    display: flex;
    align-items: baseline;
    gap: 8px;
    min-width: 0;
    font-size: 12.5px;
  }
  .badge {
    flex-shrink: 0;
    padding: 1px 8px;
    border-radius: 99px;
    border: 1px solid var(--line-2);
    color: var(--text);
    font-size: 11.5px;
    font-weight: 600;
  }
  .badge.modell {
    border-style: dashed;
  }
  .badge.formel {
    background: var(--surface-3);
  }
  kbd {
    margin-left: 2px;
    padding: 0 5px;
    border-radius: 4px;
    border: 1px solid rgba(18, 20, 24, 0.25);
    font-family: var(--font-num);
    font-size: 11px;
    font-weight: 500;
    opacity: 0.7;
  }
  @container (max-width: 640px) {
    header {
      grid-template-columns: auto minmax(0, 1fr) auto;
    }
    .next {
      grid-column: 1 / -1;
    }
  }
</style>
