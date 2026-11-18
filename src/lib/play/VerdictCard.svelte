<script lang="ts">
  import { SOURCE, reliabilityText } from "../analysis/labels";
  import Term from "../solver/Term.svelte";
  import CardText from "../ui/CardText.svelte";
  import Reason from "../ui/Reason.svelte";
  import SourceNote from "../ui/SourceNote.svelte";
  import { handNumbers } from "../solver/practice";
  import { classLabel } from "../solver/range";
  import { GRADE, THRESHOLD_TEXT, choiceColors, choiceLabel, headlineOf, num, reason, sourceLine } from "./labels";
  import type { HandView, Lesson, Street, Verdict } from "./types";
  import { pct, sameEv, signed } from "./words";

  /**
   * A graded move, or (without `chosen`) the hint before the move. `n` numbers the move in
   * the hand, so the card says which decision it belongs to.
   */
  let { view, lesson, title, n = null }: { view: HandView; lesson: Verdict | Lesson; title?: string; n?: number | null } = $props();

  const STREET: Record<Street, string> = { preflop: "Preflop", flop: "Flop", turn: "Turn", river: "River" };
  const verdict = $derived("chosen" in lesson ? (lesson as Verdict) : null);
  const j = $derived(verdict?.judgement ?? null);
  const colors = $derived(choiceColors(lesson.choices));
  const headline = $derived(verdict ? headlineOf(verdict, view.hero_cards) : null);
  const why = $derived(reason(lesson, verdict?.chosen ?? null, view.hero_cards));
  const best = $derived(j?.best ?? (lesson.strategy ? lesson.strategy.indexOf(Math.max(...lesson.strategy)) : -1));
  const topEv = $derived(lesson.evs ? Math.max(...lesson.evs) : null);
  /** Moves worth the same as the best one to the hundredth of a BB. */
  const equal = $derived(lesson.evs && topEv !== null ? lesson.evs.map((e) => sameEv(e, topEv)) : []);
  const equalCount = $derived(equal.filter(Boolean).length);
  const chosenText = $derived.by(() => {
    if (!verdict) return "";
    const l = choiceLabel(verdict.choices[verdict.chosen]);
    return `${l.label}${l.size ? ` ${l.size}` : ""}`;
  });
  /** After the flop the strategy is per combo; the solver's grid cell mixes the whole class. */
  const numbers = $derived(lesson.node ? handNumbers(lesson.node, view.hero_cards) : null);
</script>

<article class="verdict" class:bad={j?.grade === "mistake" || j?.grade === "blunder"} style:--g={j ? GRADE[j.grade].color : "var(--focus)"} aria-live="polite">
  <header>
    {#if j}
      <span class="grade"><i class="mark" aria-hidden="true">{GRADE[j.grade].mark}</i>{GRADE[j.grade].label}</span>
      {#if verdict?.hinted}<span class="tag">mit Tipp · zählt halb</span>{/if}
    {:else if verdict}
      <span class="grade none">Nicht bewertet</span>
    {:else}
      <span class="grade hint">{title ?? "Tipp"}</span>
    {/if}
    <span class="street">
      {#if verdict}{n ? `Dein ${n}. Zug · ` : ""}{STREET[lesson.street]} · {chosenText}{:else}{STREET[lesson.street]} · {lesson.class}{/if}
    </span>
  </header>

  {#if headline}
    <p class="lead"><CardText text={headline} /></p>
  {:else if j}
    <p class="lead">{GRADE[j.grade].lead}</p>
  {/if}
  {#if j?.grade !== "mix"}<p class="why"><CardText text={why} /></p>{/if}
  {#if lesson.why}<Reason text={lesson.why} />{/if}

  {#if lesson.strategy}
    <div class="bars" role="table" aria-label="Strategie der Lösung für deine Hand">
      <div class="row head" role="row">
        <span role="columnheader">Zug</span>
        {#if lesson.source === "estimate"}
          <span role="columnheader">Empfehlung der Einschätzung (gleich gute Züge teilen sich)</span>
        {:else}
          <span role="columnheader">So oft spielt es die Lösung mit <CardText text={lesson.street === "preflop" ? lesson.class : view.hero_cards.join("")} /></span>
        {/if}
        {#if lesson.evs}<span role="columnheader" class="ev"><Term term="ev">EV</Term></span>{/if}
      </div>
      {#each lesson.choices as c, k (k)}
        {@const f = lesson.strategy[k] ?? 0}
        {@const l = choiceLabel(c)}
        <div class="row" role="row" class:chosen={verdict?.chosen === k} class:best={k === best && equalCount < 2}>
          <span class="label" role="cell">
            {l.label}{#if l.size}<small> {l.size}</small>{/if}
            {#if verdict?.chosen === k}<em>deine Wahl</em>{/if}
          </span>
          <span class="bar" role="cell" aria-label={pct(f)}>
            <i style="width:{Math.max(f * 100, f > 0 ? 1.5 : 0)}%; background:{colors[k]}"></i>
            <b class="num">{pct(f)}</b>
          </span>
          {#if lesson.evs}
            <span class="ev num" role="cell" title={equal[k] && equalCount > 1 ? "Gleich viel wert wie die anderen markierten Züge" : undefined}>
              {#if equal[k] && equalCount > 1}<span class="same">≈</span>{/if}{signed(lesson.evs[k])}
            </span>
          {/if}
        </div>
      {/each}
      {#if equalCount > 1 && topEv !== null}
        <p class="equal">
          {lesson.choices
            .filter((_, k) => equal[k])
            .map((c) => choiceLabel(c).label)
            .join(" und ")} sind gleich viel wert: {equalCount === 2 ? "beide" : "alle"} ≈ {signed(topEv)} BB. Die Lösung
          <Term term="mixed">mischt</Term> sie, damit sie schwer zu lesen ist.
        </p>
      {/if}
    </div>
  {/if}

  {#if numbers?.hand && numbers.cls}
    {@const cls = numbers.cls}
    <p class="cls">
      Im Strategie-Raster steht die ganze Klasse <b class="num">{classLabel(cls.class)}</b> (alle Farben gemischt):
      {lesson.choices
        .map((c, k) => ({ c, f: cls.strategy[k] ?? 0 }))
        .filter((x) => x.f >= 0.005)
        .map((x) => `${choiceLabel(x.c).label} ${pct(x.f)}`)
        .join(", ")}. Bewertet wird deine genaue Hand.
    </p>
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

  {#if lesson.source === "estimate"}
    <SourceNote kind="estimate" variant="block">
      {SOURCE.estimate.explain}{#if lesson.reliability !== null}{" "}<b class="num">{reliabilityText(lesson.reliability, lesson.street)}</b>{/if}
    </SourceNote>
  {:else}
    <p class="source">{sourceLine(lesson.source, view.config, view.stack_bb, lesson.exploitability_pct)}</p>
  {/if}
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
    animation: rise var(--dur) var(--ease-enter);
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
  .mark {
    display: inline-grid;
    place-items: center;
    min-width: 24px;
    height: 24px;
    padding: 0 4px;
    margin-right: 8px;
    border-radius: 999px;
    background: var(--g);
    color: var(--bg);
    font-style: normal;
    font-size: 13px;
    vertical-align: 3px;
  }
  .verdict.bad {
    background: color-mix(in srgb, var(--g) 9%, var(--surface));
    border-color: color-mix(in srgb, var(--g) 45%, transparent);
  }
  .verdict.bad .lead {
    font-size: 16px;
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
    grid-template-columns: minmax(150px, 1.1fr) 2fr 76px;
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
  .same {
    margin-right: 4px;
    color: var(--text-3);
  }
  .equal,
  .cls {
    font-size: 12.5px;
    color: var(--text-2);
  }
  .cls b {
    color: var(--text);
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
