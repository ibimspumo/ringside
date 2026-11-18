<script lang="ts">
  import { api } from "../api";
  import type { LearnTarget } from "../app/Leaks.svelte";
  import AskCoach from "../report/AskCoach.svelte";
  import Button from "../ui/Button.svelte";
  import EmptyState from "../ui/EmptyState.svelte";
  import GradeIcon from "../ui/GradeIcon.svelte";
  import ProgressBar from "../ui/ProgressBar.svelte";
  import DecisionCard from "./DecisionCard.svelte";
  import { GRADE_OF, RATING, SOURCE } from "./labels";
  import { analysis } from "./store.svelte";
  import type { Decision } from "./types";

  /** Beside the replayer: analyse the hand with one click, then step through its decisions. */
  let {
    decisions,
    selected,
    busy,
    locked,
    analyzed = false,
    onAnalyze,
    onSelect,
    onLearn,
    caption = null,
    waiting = null,
    main = true,
    onCoachSetup = () => {},
  }: {
    decisions: Decision[] | null;
    selected: number | null;
    busy: boolean;
    locked: boolean;
    /** Analysed just now and nothing came out: no decision of the hero to rate. */
    analyzed?: boolean;
    onAnalyze: () => void;
    onSelect: (action: number) => void;
    onLearn?: (target: LearnTarget) => void;
    /** A line above the verdict, e.g. that the table shows the moment of this decision. */
    caption?: string | null;
    /** Instead of a verdict while the replay has not reached the first decision. */
    waiting?: string | null;
    /** Whether analysing this hand is the main action of the screen. */
    main?: boolean;
    /** Opens the coach's setup, for "Frag den Coach" without a key. */
    onCoachSetup?: () => void;
  } = $props();

  /** The question offered first: why the recommendation beats what was played, or why it was right. */
  function presets(d: Decision): string[] {
    const first =
      d.best && d.best !== d.played && d.rating && !["best", "mix", "good"].includes(d.rating)
        ? `Warum ist „${d.best}“ hier besser als „${d.played}“?`
        : `Warum ist „${d.played}“ hier richtig?`;
    return [first, "Was sagt mir die Strategie?"];
  }

  const STREET = { preflop: "Preflop", flop: "Flop", turn: "Turn", river: "River" } as const;
  const current = $derived(decisions?.find((d) => d.action_index === selected) ?? null);
  const pending = $derived(decisions?.filter((d) => d.status === "pending").length ?? 0);
  const handId = $derived(decisions?.[0]?.hand_id ?? null);
  const solving = $derived(analysis.status?.solving && analysis.status.solving.hand_id === handId ? analysis.status.solving : null);
</script>

{#if locked}
  <p class="paused">Die Analyse pausiert, bis du PokerStars schließt. Nachspielen geht weiter.</p>
{:else if decisions === null}
  <EmptyState compact tone="loading" title="Lädt die Bewertung …" />
{:else if decisions.length === 0 && analyzed}
  <div class="cta">
    <p>In dieser Hand gibt es nichts zu bewerten: Du hast nicht selbst gehandelt, oder es ist kein No-Limit Hold'em.</p>
  </div>
{:else if decisions.length === 0}
  <div class="cta">
    <strong>Wie gut hast du diese Hand gespielt?</strong>
    <p>Ringside prüft jede deiner Entscheidungen. Vor dem Flop ist das sofort fertig, nach dem Flop rechnet der Solver meist unter einer Minute.</p>
    <Button variant={main ? "primary" : "secondary"} loading={busy} disabled={busy} onclick={onAnalyze}>
      {#snippet icon()}<svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true"><path d="M5 19.5V13M10 19.5V8.5M15 19.5v-8M20 19.5V4.5" /></svg>{/snippet}
      {busy ? "Analysiere …" : "Diese Hand analysieren"}
    </Button>
  </div>
{:else}
  <div class="panel-wrap">
    {#if pending > 0}
      <div class="solving" role="status">
        <span>
          {#if solving}
            Solver rechnet {pending === 1 ? "deine Entscheidung nach dem Flop" : `deine ${pending} Entscheidungen nach dem Flop`} …
            <b class="num">{Math.round((solving.iteration / solving.max_iterations) * 100)} %</b>
          {:else}
            {pending === 1 ? "Eine Entscheidung" : `${pending} Entscheidungen`} nach dem Flop {pending === 1 ? "wartet" : "warten"} auf den Solver – diese Hand ist als Nächste dran.
          {/if}
        </span>
        <div class="row">
          <ProgressBar tone="warn" label="Fortschritt des Solvers" value={solving ? Math.min(1, solving.iteration / solving.max_iterations) : null} />
          {#if handId !== null && analysis.status?.marked.includes(handId)}
            <Button
              size="sm"
              variant="quiet"
              onclick={() => handId !== null && api.analysisCancelHand(handId)}
              title="Diese Hand nicht mehr vorziehen. Sie wird später mit den übrigen gerechnet, wenn die Analyse aller Hände läuft."
            >
              Nicht vorziehen
            </Button>
          {/if}
        </div>
      </div>
    {/if}
    <div class="steps" role="tablist" aria-label="Deine Entscheidungen in dieser Hand">
      {#each decisions as d (d.action_index)}
        {@const label = d.rating
          ? `${RATING[d.rating].label} · ${SOURCE[d.source ?? "model"].label}`
          : d.status === "pending"
            ? "Wird gerechnet"
            : d.status === "forced"
              ? "Keine Entscheidung"
              : "Nicht bewertet"}
        <button
          role="tab"
          aria-selected={d.action_index === selected}
          class:on={d.action_index === selected}
          style:--c={d.rating ? `var(--grade-${GRADE_OF[d.rating]})` : "var(--text-3)"}
          onclick={() => onSelect(d.action_index)}
          title="{STREET[d.street]}: {d.played} · {label}"
        >
          {#if d.rating}<GradeIcon grade={GRADE_OF[d.rating]} size={15} />{:else}<i class:wait={d.status === "pending"}></i>{/if}
          <span class="street">{STREET[d.street]}</span>
          <b>{d.played.split(" ")[0]}</b>
        </button>
      {/each}
    </div>
    {#if current}
      {#if caption}<p class="caption">{caption}</p>{/if}
      <DecisionCard decision={current} {onLearn} />
      {#if current.status === "rated" && handId !== null}
        {#key `${handId}:${current.action_index}`}
          <AskCoach
            compact
            about={{ kind: "hand", hand_id: handId, action_index: current.action_index }}
            presets={presets(current)}
            onSetup={onCoachSetup}
          />
        {/key}
      {/if}
    {:else if waiting}
      <p class="waiting">{waiting}</p>
    {/if}
  </div>
{/if}

<style>
  .paused,
  .waiting {
    margin: 0;
    padding: var(--sp-3) var(--sp-4);
    border: 1px dashed var(--line-2);
    border-radius: var(--r-md);
    color: var(--text-2);
    font-size: var(--fs-md);
  }
  .cta {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--sp-2);
    padding: var(--sp-4);
    border: 1px dashed var(--line-2);
    border-radius: var(--r-md);
    background: var(--surface);
    color: var(--text-2);
    font-size: var(--fs-md);
  }
  .cta strong {
    color: var(--text);
    font-size: var(--fs-base);
  }
  .cta p {
    margin: 0 0 var(--sp-1);
    line-height: var(--lh-base);
  }
  .cta svg path {
    fill: none;
    stroke: currentColor;
    stroke-width: 2.2;
    stroke-linecap: round;
  }
  .panel-wrap {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }
  .solving {
    display: flex;
    flex-direction: column;
    gap: 6px;
    color: var(--text-2);
    font-size: var(--fs-sm);
  }
  .solving b {
    color: var(--amber);
    font-weight: var(--fw-semibold);
  }
  .row {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
  }
  .row > :global(:first-child) {
    flex: 1;
  }
  .caption {
    margin: 0 0 calc(-1 * var(--sp-1));
    color: var(--text);
    font-size: var(--fs-md);
  }
  .steps {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .steps button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 30px;
    padding: 0 var(--sp-3) 0 var(--sp-2);
    border: 1px solid var(--line-2);
    border-radius: var(--r-pill);
    background: var(--surface);
    color: var(--text);
    font: inherit;
    font-size: var(--fs-md);
    cursor: pointer;
    transition:
      color var(--dur-fast) var(--ease-standard),
      border-color var(--dur-fast) var(--ease-standard),
      background-color var(--dur-fast) var(--ease-standard);
  }
  .steps button:hover {
    color: var(--text);
    border-color: color-mix(in srgb, var(--c) 55%, var(--line-2));
  }
  .steps button:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: 2px;
  }
  .steps button.on {
    border-color: var(--c);
    background: color-mix(in srgb, var(--c) 14%, var(--surface));
    color: var(--text);
  }
  .steps b {
    font-weight: var(--fw-semibold);
  }
  .street {
    color: var(--text-2);
  }
  .on .street {
    color: var(--text-2);
  }
  .steps i {
    width: 13px;
    height: 13px;
    border-radius: 50%;
    border: 1.5px dashed var(--text-3);
  }
  .steps i.wait {
    border-style: solid;
    border-color: var(--amber);
    border-right-color: transparent;
    animation: spin var(--dur-loop) var(--ease-spin) infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
