<script lang="ts">
  import type { LearnTarget } from "../app/Leaks.svelte";
  import ChartsButton, { chartable } from "../charts/ChartsButton.svelte";
  import { openInSolver } from "../solver/handoff.svelte";
  import type { SolverPrefill } from "../solver/types";
  import { EV_LOSS_EXPLAIN, LEARN, RATING, RATING_RULE, SOURCE, SPOT, SPOT_HINT, bb, pct } from "./labels";
  import { analysis } from "./store.svelte";
  import type { Choice, Decision } from "./types";

  /** One decision: the verdict in words first, then what the source plays and why it may differ. */
  let { decision: d, onLearn }: { decision: Decision; onLearn?: (target: LearnTarget) => void } = $props();

  const STREET = { preflop: "Preflop", flop: "Flop", turn: "Turn", river: "River" } as const;

  const split = $derived.by(() => {
    const at = d.why.indexOf(" Grund: ");
    return at < 0 ? { head: d.why, reason: "" } : { head: d.why.slice(0, at), reason: d.why.slice(at + 8) };
  });
  const bestEv = $derived(Math.max(...d.strategy.map((c) => c.ev_bb ?? -Infinity)));
  const solving = $derived(analysis.status?.solving?.hand_id === d.hand_id ? analysis.status.solving : null);
  const prefill = $derived.by((): SolverPrefill | null => {
    if (!d.solver) return null;
    try {
      return JSON.parse(d.solver) as SolverPrefill;
    } catch {
      return null;
    }
  });

  function tone(c: Choice): string {
    const word = c.label.split(" ")[0];
    if (word === "Fold") return "fold";
    if (word === "Check" || word === "Call" || word === "Limp") return "passive";
    if (word === "All-in") return "allin";
    return "raise";
  }

  function gap(c: Choice): string {
    if (c.ev_bb === null || !isFinite(bestEv)) return "";
    const loss = bestEv - c.ev_bb;
    return loss < 0.005 ? "beste" : `−${bb(loss, 2)}`;
  }
</script>

<article class="card" style="--tone:{d.rating ? RATING[d.rating].color : 'var(--text-3)'}">
  <header>
    {#if d.rating}
      <span class="pill" title={RATING_RULE}><i></i>{RATING[d.rating].label}</span>
    {:else if d.status === "pending"}
      <span class="pill wait"><i></i>{solving ? "Wird gerechnet" : "Wartet"}</span>
    {:else}
      <span class="pill none"><i></i>Nicht bewertet</span>
    {/if}
    <span class="spot" title={SPOT_HINT[d.spot]}>{SPOT[d.spot]}{SPOT[d.spot] === STREET[d.street] ? "" : ` · ${STREET[d.street]}`}</span>
    {#if d.source}
      <span class="source" title={SOURCE[d.source].explain}>{SOURCE[d.source].label}</span>
    {/if}
  </header>

  {#if d.status === "rated"}
    <p class="head">{split.head}</p>
    {#if split.reason}<p class="reason"><b>Grund:</b> {split.reason}</p>{/if}

    <div class="mix" role="img" aria-label="Was {d.source ? SOURCE[d.source].label : 'die Berechnung'} mit deiner Hand spielt">
      {#each d.strategy.filter((c) => c.freq >= 0.005 || c.played) as c (c.label)}
        <span class="seg {tone(c)}" class:mine={c.played} style="flex-grow:{Math.max(c.freq, 0.015)}" title="{c.label}: {pct(c.freq)}"></span>
      {/each}
    </div>
    <ul class="options">
      {#each d.strategy as c (c.label)}
        <li class:mine={c.played}>
          <span class="dot {tone(c)}"></span>
          <span class="label">{c.label}{#if c.played}<em>du</em>{/if}</span>
          <span class="num freq">{pct(c.freq)}</span>
          <span class="num ev" class:best={gap(c) === "beste"} title={EV_LOSS_EXPLAIN}>{gap(c)}</span>
        </li>
      {/each}
    </ul>
    {#if d.ev_loss_bb !== null && d.ev_loss_bb >= 0.005}
      <p class="loss">
        <abbr title={EV_LOSS_EXPLAIN}>EV-Verlust</abbr>
        <b class="num">{bb(d.ev_loss_bb, 2)}</b>
        <span>im Schnitt, gegenüber der besten Aktion</span>
      </p>
    {/if}
  {:else if d.status === "pending"}
    <p class="head">Dein {d.played.split(" ")[0]} wird gerade nachgerechnet.</p>
    {#if solving}
      <div class="progress" aria-label="Fortschritt des Solvers">
        <i style="width:{Math.min(100, (solving.iteration / solving.max_iterations) * 100)}%"></i>
      </div>
      <p class="reason">
        Der Solver rechnet diese Hand ab dem Flop durch ({Math.round(solving.elapsed_ms / 1000)} s). Meist reicht weniger als eine
        Minute.
      </p>
    {:else}
      <p class="reason">Diese Hand steht ganz vorn in der Warteschlange und wird als Nächstes gerechnet.</p>
    {/if}
  {:else}
    <p class="head">Diese Entscheidung bewertet Ringside nicht.</p>
    <p class="reason">{d.reason}</p>
  {/if}

  {#if d.note}
    <details>
      <summary>Was angenommen wurde</summary>
      <p>{d.note}</p>
      {#if d.source}<p class="faint">{SOURCE[d.source].explain}</p>{/if}
    </details>
  {/if}

  {#if (prefill && d.status === "rated") || (onLearn && d.rating && d.rating !== "best") || chartable(d)}
    <div class="actions">
      <ChartsButton decision={d} />
      {#if prefill && d.status === "rated"}
        <button class="btn" onclick={() => openInSolver(prefill)} title="Öffnet den Flop dieser Hand mit denselben Ranges im Solver.">
          Im Solver öffnen
        </button>
      {/if}
      {#if onLearn && d.rating && d.rating !== "best"}
        {#each LEARN[d.spot] as l (l.label)}
          <button class="btn ghost" onclick={() => onLearn(l.target)}>{l.label}</button>
        {/each}
      {/if}
    </div>
  {/if}
</article>

<style>
  .card {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 14px 16px 14px;
    border: 1px solid var(--line);
    border-left: 3px solid var(--tone);
    border-radius: var(--r-lg);
    background: var(--surface);
    font-size: 13px;
  }
  header {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 10px;
  }
  .pill {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-weight: 600;
    color: var(--tone);
    cursor: help;
  }
  .pill i {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: var(--tone);
  }
  .pill.wait i {
    background: transparent;
    border: 2px solid var(--amber);
    border-right-color: transparent;
    animation: spin 1s linear infinite;
  }
  .pill.none i {
    background: transparent;
    border: 1.5px dashed var(--text-3);
  }
  .spot {
    color: var(--text-2);
    cursor: help;
  }
  .source {
    margin-left: auto;
    padding: 1px 8px;
    border-radius: 999px;
    font-size: 11.5px;
    color: var(--text-2);
    box-shadow: inset 0 0 0 1px var(--line-2);
    cursor: help;
  }
  .head {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
    line-height: 1.35;
    color: var(--text);
    text-wrap: pretty;
  }
  .reason {
    margin: -4px 0 0;
    line-height: 1.45;
    color: var(--text-2);
  }
  .reason b {
    font-weight: 600;
    color: var(--text);
  }
  .mix {
    display: flex;
    gap: 2px;
    height: 14px;
    margin-top: 2px;
  }
  .seg {
    flex-basis: 0;
    border-radius: 3px;
    min-width: 4px;
  }
  .seg.mine {
    box-shadow:
      0 0 0 2px var(--surface),
      0 0 0 3.5px var(--text);
  }
  .fold {
    background: #4b525e;
  }
  .passive {
    background: var(--sd);
  }
  .raise {
    background: var(--amber);
  }
  .allin {
    background: var(--neg);
  }
  .options {
    display: grid;
    gap: 2px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .options li {
    display: grid;
    grid-template-columns: 10px minmax(0, 1fr) 46px 72px;
    align-items: center;
    gap: 8px;
    padding: 2px 4px;
    border-radius: 4px;
    color: var(--text-2);
  }
  .options li.mine {
    background: var(--surface-2);
    color: var(--text);
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 2px;
  }
  .label em {
    margin-left: 7px;
    padding: 0 5px;
    border-radius: 3px;
    font-size: 10.5px;
    font-style: normal;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--bg);
    background: var(--text);
  }
  .freq,
  .ev {
    text-align: right;
  }
  .ev {
    color: var(--text-3);
    cursor: help;
  }
  .ev.best {
    color: var(--pos);
  }
  .loss {
    display: flex;
    align-items: baseline;
    flex-wrap: wrap;
    gap: 4px 8px;
    margin: 0;
    color: var(--text-2);
  }
  .loss b {
    font-size: 15px;
    color: var(--tone);
  }
  abbr {
    text-decoration: underline dotted;
    text-underline-offset: 3px;
    cursor: help;
  }
  .progress {
    height: 4px;
    border-radius: 2px;
    background: var(--surface-3);
    overflow: hidden;
  }
  .progress i {
    display: block;
    height: 100%;
    background: var(--amber);
    transition: width 0.3s ease;
  }
  details {
    color: var(--text-2);
  }
  summary {
    cursor: pointer;
    color: var(--text-3);
    font-size: 12.5px;
  }
  summary:hover {
    color: var(--text-2);
  }
  details p {
    margin: 6px 0 0;
    line-height: 1.45;
    font-size: 12.5px;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .actions .btn {
    height: 30px;
    font-size: 12.5px;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .pill.wait i {
      animation: none;
    }
    .progress i {
      transition: none;
    }
  }
</style>
