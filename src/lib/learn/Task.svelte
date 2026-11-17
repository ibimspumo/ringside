<script lang="ts">
  import type { Snippet } from "svelte";
  import type { Exercise, Feedback, LearnAnswer } from "../types";
  import CardPicker from "./CardPicker.svelte";
  import { GRADING, SKILLS, errorText, fmtPct } from "./catalog";
  import FeedbackView from "./FeedbackView.svelte";
  import Stage from "./Stage.svelte";
  import { preview, previewAnswer } from "./preview";
  import { onMount } from "svelte";

  /**
   * One exercise: question, picture, the matching input, then feedback.
   * Remount it (`{#key}`) for the next exercise.
   */
  let {
    exercise,
    check,
    onGraded,
    onNext,
    nextLabel = "Weiter",
    showStage = true,
    after,
  }: {
    exercise: Exercise;
    check: (answer: LearnAnswer) => Promise<Feedback>;
    onGraded?: (feedback: Feedback, answer: LearnAnswer) => void;
    onNext?: () => void;
    nextLabel?: string;
    /** Off when a replayer already shows the spot. */
    showStage?: boolean;
    /** Extra content below the facts, shown once graded. */
    after?: Snippet<[Feedback]>;
  } = $props();

  let feedback = $state<Feedback | null>(null);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let picked = $state<string[]>([]);
  let value = $state<number | null>(null);
  let chosen = $state<string | null>(null);
  let seat = $state<number | null>(null);

  const spec = $derived(exercise.answer);
  const prompt = $derived(exercise.prompt);
  const meta = $derived(SKILLS[exercise.kind]);

  /** Shortcut letters for decisions; everything else goes by number. */
  const LETTER: Record<string, string> = { shove: "A", call: "C", fold: "F", split: "S" };
  const options = $derived(spec.type === "choice" ? spec.options : []);
  const keyOf = (id: string, i: number) => LETTER[id] ?? String(i + 1);
  const decision = $derived(options.some((o) => o.id === "fold"));

  const ready = $derived(
    spec.type === "cards" ? picked.length > 0 : spec.type === "number" ? value !== null && Number.isFinite(value) : false,
  );

  async function submit(answer: LearnAnswer) {
    if (busy || feedback) return;
    busy = true;
    error = null;
    try {
      const fb = await check(answer);
      feedback = fb;
      onGraded?.(fb, answer);
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = false;
    }
  }

  onMount(() => {
    if (preview("answer") !== "1") return;
    const a = previewAnswer(spec);
    if ("choice" in a) chosen = a.choice;
    if ("seat" in a) seat = a.seat;
    if ("cards" in a) picked = a.cards;
    if ("number" in a) value = a.number;
    submit(a);
  });

  function submitDraft() {
    if (!ready) return;
    if (spec.type === "cards") submit({ cards: picked });
    else if (spec.type === "number" && value !== null) submit({ number: value });
  }

  function choose(id: string) {
    chosen = id;
    submit({ choice: id });
  }

  function pickSeat(s: number) {
    seat = s;
    submit({ seat: s });
  }

  function key(e: KeyboardEvent) {
    if (e.metaKey || e.ctrlKey || e.altKey) return;
    const t = e.target as HTMLElement | null;
    const tag = t?.tagName;
    if (e.key === "Enter") {
      // Buttons handle their own Enter; number fields submit.
      if (tag === "BUTTON" || tag === "TEXTAREA" || tag === "SELECT") return;
      e.preventDefault();
      if (feedback) onNext?.();
      else submitDraft();
      return;
    }
    if (feedback || tag === "INPUT" || tag === "TEXTAREA") return;
    const k = e.key.toUpperCase();
    const i = options.findIndex((o, n) => keyOf(o.id, n) === k);
    if (i >= 0) {
      e.preventDefault();
      choose(options[i].id);
    }
  }

  const truthChoice = $derived(feedback && "choice" in feedback.truth ? feedback.truth.choice : null);
  const truthSeat = $derived(feedback && "seat" in feedback.truth ? feedback.truth.seat : null);
  const unit = $derived(spec.type === "number" ? spec.unit : "");
  const isPct = $derived(unit === "%");
</script>

<svelte:window onkeydown={key} />

<div class="task">
  <header>
    <span class="kind">{meta.short}</span>
    <span class="lvl" title={meta.levels[exercise.level - 1]}>
      {#each [1, 2, 3] as n (n)}<i class:on={n <= exercise.level}></i>{/each}
      Stufe {exercise.level}
    </span>
    <span class="badge {exercise.grading}" title={GRADING[exercise.grading].text}>{GRADING[exercise.grading].label}</span>
    {#if exercise.hand_id !== null}<span class="badge own">Deine Hand</span>{/if}
  </header>

  <h2 class="q">{exercise.question}</h2>
  {#if !feedback && exercise.model_note && (prompt.type === "push_fold" || prompt.type === "push_fold_table")}
    <p class="model-note"><span class="badge modell">Modell</span> {exercise.model_note}</p>
  {/if}

  {#if showStage}
    <Stage
      {prompt}
      choice={chosen}
      {truthChoice}
      onChoose={prompt.type === "showdown" && !feedback && !busy ? choose : undefined}
      seatPickable={spec.type === "seat" && !feedback && !busy}
      {seat}
      {truthSeat}
      onSeat={pickSeat}
    />
  {/if}

  <div class="input">
    {#if spec.type === "cards"}
      <CardPicker from={spec.from} bind:picked highlight={feedback ? feedback.highlight : null} />
    {:else if spec.type === "number"}
      <div class="number" class:pct={isPct}>
        {#if isPct}
          <div class="rail">
            <span class="end num">{spec.min} %</span>
            <div class="track">
              <input
                type="range"
                min={spec.min}
                max={spec.max}
                step={spec.step}
                value={value ?? (spec.min + spec.max) / 2}
                class:unset={value === null}
                disabled={!!feedback}
                aria-label="Schätzung in Prozent"
                oninput={(e) => (value = Number(e.currentTarget.value))}
              />
              {#if feedback && "number" in feedback.truth}
                {@const t = feedback.truth.number}
                <span class="mark truth" style="left:{(t / spec.max) * 100}%"><b class="num">Lösung {fmtPct(t, 1)}</b></span>
                {#if value !== null}<span class="mark you" style="left:{(value / spec.max) * 100}%"><b class="num">Du {fmtPct(value, 1)}</b></span>{/if}
              {/if}
            </div>
            <span class="end num">{spec.max} %</span>
          </div>
        {/if}
        <label class="field-wrap">
          <input
            class="field num"
            type="number"
            min={spec.min}
            max={spec.max}
            step={spec.step}
            inputmode="decimal"
            placeholder={isPct ? "z. B. 32,5" : "Anzahl"}
            value={value ?? ""}
            disabled={!!feedback}
            aria-label="Antwort in {unit}"
            oninput={(e) => {
              const v = e.currentTarget.valueAsNumber;
              value = Number.isFinite(v) ? v : null;
            }}
          />
          <span class="unit-label">{unit}</span>
        </label>
      </div>
    {:else if spec.type === "choice"}
      <div class="choices" class:decision role="group" aria-label="Antworten">
        {#each options as o, i (o.id)}
          {#if !(prompt.type === "showdown" && o.id !== "split")}
            <button
              class="choice {o.id}"
              class:on={chosen === o.id}
              class:right={truthChoice === o.id}
              class:wrong={truthChoice !== null && chosen === o.id && truthChoice !== o.id}
              disabled={!!feedback || busy}
              onclick={() => choose(o.id)}
            >
              {o.label}
              <kbd>{keyOf(o.id, i)}</kbd>
            </button>
          {/if}
        {/each}
      </div>
    {:else if spec.type === "seat" && !feedback}
      <p class="hint faint">Tippe den Sitz auf dem Tisch an.</p>
    {/if}
  </div>

  {#if error}
    <p class="error" role="alert">{error}</p>
  {/if}

  {#if !feedback && (spec.type === "cards" || spec.type === "number")}
    <div class="actions">
      <button class="btn primary" disabled={!ready || busy} onclick={submitDraft}>Prüfen <kbd>Enter</kbd></button>
    </div>
  {/if}

  {#if feedback}
    <FeedbackView {feedback} {onNext} {nextLabel}>
      {@render after?.(feedback)}
    </FeedbackView>
  {/if}
</div>

<style>
  .task {
    display: grid;
    gap: 16px;
    min-width: 0;
  }
  header {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 12px;
    font-size: 12.5px;
    color: var(--text-2);
  }
  .kind {
    color: var(--text);
    font-weight: 600;
  }
  .lvl {
    display: inline-flex;
    align-items: center;
    gap: 3px;
  }
  .lvl i {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--surface-3);
  }
  .lvl i.on {
    background: var(--text-2);
  }
  .lvl i:last-of-type {
    margin-right: 4px;
  }
  .badge {
    padding: 1px 8px;
    border-radius: 99px;
    border: 1px solid var(--line-2);
    font-size: 11.5px;
    font-weight: 600;
    color: var(--text);
  }
  .badge.modell {
    border-style: dashed;
  }
  .badge.formel {
    background: var(--surface-3);
  }
  .badge.own {
    border-color: transparent;
    background: var(--text);
    color: var(--bg);
  }
  .q {
    margin: 0;
    max-width: 70ch;
    font-size: 17px;
    font-weight: 500;
    line-height: 1.45;
    text-wrap: pretty;
  }
  .model-note {
    margin: -6px 0 0;
    max-width: 80ch;
    font-size: 12.5px;
    line-height: 1.5;
    color: var(--text-2);
  }
  .model-note .badge {
    margin-right: 4px;
    font-size: 10.5px;
    color: var(--text-2);
  }
  .input {
    min-width: 0;
  }
  .number {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 12px 18px;
    max-width: 760px;
  }
  .number:not(.pct) {
    max-width: 220px;
  }
  .rail {
    flex: 1 1 360px;
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }
  .end {
    font-size: 12.5px;
    color: var(--text-2);
  }
  .track {
    position: relative;
    flex: 1;
    min-width: 0;
  }
  input[type="range"] {
    display: block;
    width: 100%;
    height: 28px;
    margin: 0;
  }
  input[type="range"].unset {
    opacity: 0.55;
  }
  .field-wrap {
    position: relative;
    display: block;
    width: 150px;
  }
  .field-wrap .field {
    width: 100%;
    height: 44px;
    padding-right: 40px;
    font-size: 22px;
    font-weight: 600;
  }
  .unit-label {
    position: absolute;
    right: 12px;
    top: 50%;
    transform: translateY(-50%);
    color: var(--text-2);
    font-size: 14px;
    pointer-events: none;
  }
  .mark {
    position: absolute;
    top: 50%;
    transform: translate(-50%, -50%);
    display: grid;
    justify-items: center;
    pointer-events: none;
    animation: drop 0.4s cubic-bezier(0.2, 0.9, 0.3, 1.3) both;
  }
  .mark::before {
    content: "";
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: var(--c);
    box-shadow: 0 0 0 3px var(--surface);
  }
  .mark b {
    position: absolute;
    top: 20px;
    font-size: 12px;
    font-weight: 600;
    white-space: nowrap;
    color: var(--c);
  }
  .mark.truth {
    --c: var(--pos);
  }
  .mark.you {
    --c: var(--text);
    animation-delay: 0.08s;
  }
  .mark.you b {
    top: auto;
    bottom: 20px;
  }
  .number.pct:has(.mark) {
    padding: 18px 0 14px;
  }
  @keyframes drop {
    from {
      opacity: 0;
      transform: translate(-50%, -80%);
    }
  }
  .choices {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .choice {
    display: inline-flex;
    align-items: center;
    gap: 10px;
    min-height: 42px;
    padding: 0 16px;
    border-radius: var(--r-sm);
    border: 1px solid var(--line-2);
    background: var(--surface-2);
    font-size: 14px;
    font-weight: 600;
    cursor: pointer;
    transition:
      background 0.12s,
      transform 0.12s,
      border-color 0.12s;
  }
  .choice:hover:not(:disabled) {
    background: var(--surface-3);
    transform: translateY(-1px);
  }
  .choice:disabled {
    cursor: default;
  }
  .decision .choice {
    flex: 1 1 160px;
    justify-content: center;
    min-height: 56px;
    font-size: 17px;
  }
  .decision .choice.shove,
  .decision .choice.call {
    background: var(--text);
    border-color: var(--text);
    color: var(--bg);
  }
  .decision .choice.shove:hover:not(:disabled),
  .decision .choice.call:hover:not(:disabled) {
    background: #fff;
  }
  .choice.on {
    border-color: var(--text);
    box-shadow: 0 0 0 2px var(--text);
  }
  .choice.right {
    border-color: var(--pos);
    box-shadow: 0 0 0 2px var(--pos);
  }
  .choice.wrong {
    border-color: var(--neg);
    box-shadow: 0 0 0 2px var(--neg);
  }
  .choice:disabled:not(.on, .right, .wrong) {
    opacity: 0.45;
  }
  kbd {
    padding: 0 5px;
    border-radius: 4px;
    border: 1px solid currentColor;
    font-family: var(--font-num);
    font-size: 11px;
    font-weight: 500;
    opacity: 0.5;
  }
  .hint {
    margin: 0;
    text-align: center;
    font-size: 13px;
  }
  .actions {
    display: flex;
    gap: 8px;
  }
  .actions .btn {
    height: 38px;
    padding: 0 18px;
  }
  .error {
    margin: 0;
    padding: 8px 12px;
    border-radius: var(--r-sm);
    background: rgba(229, 72, 59, 0.12);
    color: #ff9d93;
    font-size: 13px;
  }
</style>
