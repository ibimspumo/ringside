<script lang="ts">
  import { api } from "../api";
  import type { LearnTarget } from "../app/Leaks.svelte";
  import DecisionCard from "./DecisionCard.svelte";
  import { RATING, SOURCE } from "./labels";
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
  } = $props();

  const STREET = { preflop: "Pre", flop: "Flop", turn: "Turn", river: "River" } as const;
  const current = $derived(decisions?.find((d) => d.action_index === selected) ?? null);
  const pending = $derived(decisions?.filter((d) => d.status === "pending").length ?? 0);
  const handId = $derived(decisions?.[0]?.hand_id ?? null);
  const solving = $derived(analysis.status?.solving && analysis.status.solving.hand_id === handId ? analysis.status.solving : null);
</script>

{#if locked}
  <div class="cta">
    <p>Die Analyse ist gesperrt, solange PokerStars läuft.</p>
  </div>
{:else if decisions === null}
  <div class="cta"><p class="faint">Lädt …</p></div>
{:else if decisions.length === 0 && analyzed}
  <div class="cta">
    <p>In dieser Hand gibt es nichts zu bewerten: Du hast nicht selbst gehandelt, oder es ist kein No-Limit Hold'em.</p>
  </div>
{:else if decisions.length === 0}
  <div class="cta">
    <button class="btn primary big" disabled={busy} onclick={onAnalyze}>
      <svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true"><path d="M5 19.5V13M10 19.5V8.5M15 19.5v-8M20 19.5V4.5" /></svg>
      {busy ? "Analysiere …" : "Diese Hand analysieren"}
    </button>
    <p>Ringside prüft jede deiner Entscheidungen in dieser Hand. Vor dem Flop ist das sofort fertig, nach dem Flop rechnet der Solver meist unter einer Minute.</p>
  </div>
{:else}
  <div class="panel-wrap">
    {#if pending > 0}
      <div class="solving" role="status">
        <span>
          {#if solving}
            Solver rechnet {pending === 1 ? "deine Flop-Entscheidung" : `deine ${pending} Entscheidungen nach dem Flop`} …
            <b class="num">{Math.round((solving.iteration / solving.max_iterations) * 100)} %</b>
          {:else}
            {pending === 1 ? "Eine Entscheidung" : `${pending} Entscheidungen`} nach dem Flop {pending === 1 ? "wartet" : "warten"} auf den Solver – diese Hand ist als Nächste dran.
          {/if}
        </span>
        <div class="row">
          <div class="bar"><i style="width:{solving ? Math.min(100, (solving.iteration / solving.max_iterations) * 100) : 4}%"></i></div>
          {#if handId !== null && analysis.status?.marked.includes(handId)}
            <button class="stop" onclick={() => handId !== null && api.analysisCancelHand(handId)} title="Diese Hand nicht mehr vorziehen. Sie wird später mit den übrigen gerechnet, wenn die Analyse aller Hände läuft.">
              Abbrechen
            </button>
          {/if}
        </div>
      </div>
    {/if}
    <div class="steps" role="tablist" aria-label="Deine Entscheidungen">
      {#each decisions as d (d.action_index)}
        <button
          role="tab"
          aria-selected={d.action_index === selected}
          class:on={d.action_index === selected}
          style="--c:{d.rating ? RATING[d.rating].color : 'var(--text-3)'}"
          class:open={!d.rating}
          onclick={() => onSelect(d.action_index)}
          title={d.rating ? `${RATING[d.rating].label} · ${SOURCE[d.source ?? "model"].label}` : d.status === "pending" ? "Wird gerechnet" : d.status === "forced" ? "Keine Entscheidung" : "Nicht bewertet"}
        >
          <i></i>{STREET[d.street]} · {d.played.split(" ")[0]}
        </button>
      {/each}
    </div>
    {#if current}
      <DecisionCard decision={current} {onLearn} />
    {/if}
  </div>
{/if}

<style>
  .cta {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 14px 16px;
    border: 1px dashed var(--line-2);
    border-radius: var(--r-lg);
    background: var(--surface);
    font-size: 13px;
    color: var(--text-2);
  }
  .cta p {
    margin: 0;
    line-height: 1.45;
  }
  .big {
    align-self: flex-start;
    height: 36px;
    padding: 0 16px;
    font-size: 14px;
    gap: 8px;
  }
  .big svg path {
    fill: none;
    stroke: currentColor;
    stroke-width: 2.2;
    stroke-linecap: round;
  }
  .panel-wrap {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .solving {
    display: flex;
    flex-direction: column;
    gap: 5px;
    font-size: 12.5px;
    color: var(--text-2);
  }
  .solving b {
    color: var(--amber);
    font-weight: 600;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .row .bar {
    flex: 1;
  }
  .stop {
    height: 22px;
    padding: 0 8px;
    border: 1px solid var(--line-2);
    border-radius: 999px;
    background: transparent;
    color: var(--text-2);
    font-size: 11.5px;
    cursor: pointer;
  }
  .stop:hover {
    color: var(--text);
  }
  .bar {
    height: 3px;
    border-radius: 2px;
    background: var(--surface-3);
    overflow: hidden;
  }
  .bar i {
    display: block;
    height: 100%;
    background: var(--amber);
    transition: width 0.3s ease;
  }
  @media (prefers-reduced-motion: reduce) {
    .bar i {
      transition: none;
    }
  }
  .steps {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .steps button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 26px;
    padding: 0 9px;
    border: 1px solid var(--line);
    border-radius: 999px;
    background: var(--surface);
    color: var(--text-2);
    font-size: 12px;
    cursor: pointer;
  }
  .steps button:hover {
    color: var(--text);
  }
  .steps button.on {
    border-color: var(--c);
    color: var(--text);
    background: var(--surface-2);
  }
  .steps i {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--c);
  }
  .steps .open i {
    background: transparent;
    border: 1.5px dashed var(--c);
  }
</style>
