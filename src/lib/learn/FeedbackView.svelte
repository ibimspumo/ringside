<script lang="ts">
  import type { Feedback } from "../types";
  import { GRADING, explain, fmtBB, withTerms } from "./catalog";
  import { preview } from "./preview";
  import { input } from "./input";
  import Ring from "./Ring.svelte";

  let {
    feedback,
    onNext,
    nextLabel = "Weiter",
    estimate = null,
    unit = "",
  }: {
    feedback: Feedback;
    onNext?: () => void;
    nextLabel?: string;
    /** The number the player entered, for a plain sentence instead of answer and gap tiles. */
    estimate?: number | null;
    unit?: string;
  } = $props();

  const verdict = $derived(feedback.correct ? "ok" : feedback.score >= 0.5 ? "close" : "miss");
  const TITLE = { ok: "Richtig", close: "Knapp daneben", miss: "Daneben" };
  const tone = $derived(verdict === "ok" ? "var(--pos)" : verdict === "close" ? "var(--amber)" : "var(--neg)");
  const g = $derived(GRADING[feedback.grading]);
  /** "EV All-in", "EV Fold": spelled out as one sentence instead of separate tiles. */
  const evFacts = $derived(feedback.facts.filter((f) => /^EV [^-]/.test(f.label)));
  const bbValue = (v: string) => Number.parseFloat(v.replace("−", "-").replace(",", "."));
  const evSentence = $derived.by(() => {
    if (evFacts.length < 2) return "";
    const said = evFacts.map((f) => `${f.label.slice(3)} bringt im Schnitt ${f.value}`).join(", ") + ".";
    // Short stacks often lose either way; say so, or "best" reads like winning.
    const allLose = evFacts.every((f) => bbValue(f.value) < 0);
    return allLose ? `${said} Beide Wege kosten Chips, ${feedback.truth_text} kostet weniger.` : said;
  });
  const truthNumber = $derived("number" in feedback.truth ? feedback.truth.number : null);
  const gapSentence = $derived.by(() => {
    if (estimate === null || truthNumber === null) return "";
    const d = estimate - truthNumber;
    const n = Math.abs(d).toLocaleString("de-DE", { maximumFractionDigits: 1 });
    if (Math.abs(d) < 0.05) return "Genau getroffen.";
    return `Du lagst ${unit === "%" ? `${n} Prozentpunkte` : `um ${n}`} zu ${d > 0 ? "hoch" : "niedrig"}.`;
  });
  /** "31,8 %" as a frequency a beginner can picture. */
  const tenths = $derived(unit === "%" && truthNumber !== null ? Math.round(truthNumber / 10) : null);

  /** Tiles the sentence or the slider already say. */
  const SAID = new Set(["Deine Antwort", "Abweichung"]);
  const facts = $derived(
    feedback.facts.filter(
      (f) => !(evSentence && evFacts.includes(f)) && !(gapSentence && (SAID.has(f.label) || f.value === feedback.truth_text)),
    ),
  );
  let nextBtn = $state<HTMLButtonElement>();
  let root = $state<HTMLElement>();

  // The explanation comes into view and the next step is one key press away.
  $effect(() => {
    const calm = matchMedia("(prefers-reduced-motion: reduce)").matches || preview("answer") !== null;
    root?.scrollIntoView({ block: "nearest", behavior: calm ? "auto" : "smooth" });
    // Show the focus ring only to people who answered with the keyboard, like :focus-visible would.
    nextBtn?.focus({ preventScroll: true, focusVisible: input.keys } as FocusOptions);
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
      <p>Lösung: <b class="num">{feedback.truth_text}</b>{#if gapSentence}<span class="gap">{gapSentence}</span>{/if}</p>
      {#if tenths !== null}
        <p class="plain">
          Zum Vorstellen: {feedback.truth_text} sind etwa {tenths} von 10 Fällen{tenths === 0 ? ", also fast nie" : tenths === 10 ? ", also fast immer" : ""}.
        </p>
      {/if}
    </div>
    <Ring value={feedback.score} size={60} stroke={5} {tone} label="{Math.round(feedback.score * 100)} von 100 Punkten">
      <b class="num score">{Math.round(feedback.score * 100)}</b>
      <span class="of">Punkte</span>
    </Ring>
    {#if onNext}
      <button class="btn primary next" bind:this={nextBtn} onclick={onNext}>{nextLabel} <kbd>Enter</kbd></button>
    {/if}
  </header>

  {#if feedback.ev_loss_bb !== null}
    <div class="ev" class:none={feedback.ev_loss_bb <= 0.005} title={explain("EV-Verlust")}>
      {#if feedback.ev_loss_bb <= 0.005}
        <b>Beste Entscheidung.</b>
      {:else}
        <b>Deine Wahl kostet im Schnitt <span class="num">{fmtBB(feedback.ev_loss_bb)}</span> mehr als {feedback.truth_text}.</b>
      {/if}
      {evSentence}
    </div>
  {/if}

  {#snippet factList()}
    <dl class="facts">
      {#each facts as f, i (i)}
        <div class="fact" style="--i:{i}">
          <dt>{#if explain(f.label)}<span class="term" title={explain(f.label)}>{f.label}</span>{:else}{f.label}{/if}</dt>
          <dd class="num">{f.value}</dd>
          {#if f.formula}<dd class="formula num">{f.formula}</dd>{/if}
        </div>
      {/each}
    </dl>
  {/snippet}
  {#if facts.length && evSentence}
    <details class="more">
      <summary>Details zur Lösung <span class="faint">{facts.map((f) => f.label).join(" · ")}</span></summary>
      {@render factList()}
    </details>
  {:else if facts.length}
    {@render factList()}
  {/if}


  <footer class="grading" title={g.text}>
    <span class="badge {feedback.grading}">{g.label}</span>
    <span class="dim"
      >{#if feedback.grading === "modell"}<b class="plain-lead">Vereinfachtes Modell.</b>
      {/if}</span
    >
    <span class="dim"
      >{#each withTerms(feedback.model_note ?? g.text) as p, i (i)}{#if p.tip}<span class="term" title={p.tip}>{p.text}</span
          >{:else}{p.text}{/if}{/each}</span
    >
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
  .plain {
    margin-top: 2px;
    font-size: 13px;
  }
  .gap {
    margin-left: 10px;
    color: var(--text);
  }
  .score {
    font-size: 17px;
  }
  .of {
    font-size: 9.5px;
    color: var(--text-2);
  }
  .ev {
    padding: 9px 12px;
    border-radius: var(--r-sm);
    border-left: 3px solid #ff9d93;
    background: rgba(229, 72, 59, 0.1);
    color: var(--text);
    font-size: 13.5px;
    line-height: 1.5;
  }
  .ev b {
    color: #ff9d93;
  }
  .ev.none {
    border-left-color: var(--pos);
    background: rgba(120, 193, 66, 0.1);
  }
  .ev.none b {
    color: var(--pos);
  }
  .more summary {
    display: flex;
    align-items: baseline;
    gap: 10px;
    padding: 2px 0;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    list-style: none;
  }
  .more summary::-webkit-details-marker {
    display: none;
  }
  .more summary::before {
    content: "";
    align-self: center;
    width: 6px;
    height: 6px;
    border-right: 1.6px solid currentColor;
    border-bottom: 1.6px solid currentColor;
    transform: rotate(-45deg);
    transition: transform 0.15s;
  }
  .more[open] summary::before {
    transform: rotate(45deg);
  }
  .more summary .faint {
    font-weight: 400;
    font-size: 12.5px;
  }
  .more[open] .facts {
    margin-top: 10px;
  }
  .facts {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(140px, 1fr));
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
    color: var(--text-2);
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
  .plain-lead {
    color: var(--text);
    font-weight: 600;
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
