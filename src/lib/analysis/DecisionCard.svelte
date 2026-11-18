<script lang="ts">
  import type { LearnTarget } from "../app/Leaks.svelte";
  import ChartsButton, { chartable } from "../charts/ChartsButton.svelte";
  import { openInSolver } from "../solver/handoff.svelte";
  import type { SolverPrefill } from "../solver/types";
  import Button from "../ui/Button.svelte";
  import Verdict from "../ui/Verdict.svelte";
  import { gradeOf, kitSource } from "./grade";
  import { EV_LOSS_EXPLAIN, LEARN, RATING_RULE, SOURCE, SPOT, SPOT_HINT, bb, pct } from "./labels";
  import { analysis } from "./store.svelte";
  import type { Choice, Decision } from "./types";

  /** One decision: the verdict in words first, then what the source plays and why it may differ. */
  let { decision: d, onLearn }: { decision: Decision; onLearn?: (target: LearnTarget) => void } = $props();

  const STREET = { preflop: "Preflop", flop: "Flop", turn: "Turn", river: "River" } as const;

  const split = $derived.by(() => {
    const at = d.why.indexOf(" Grund: ");
    return at < 0 ? { head: d.why, reason: "" } : { head: d.why.slice(0, at), reason: d.why.slice(at + 8) };
  });
  const context = $derived(SPOT[d.spot] === STREET[d.street] ? STREET[d.street] : `${STREET[d.street]} · ${SPOT[d.spot]}`);
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
  const learn = $derived(onLearn && d.rating && d.rating !== "best" ? LEARN[d.spot] : []);

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

{#if d.status === "rated" && d.rating}
  <Verdict
    grade={gradeOf(d.rating)}
    played={d.played}
    better={d.best ?? undefined}
    cost={d.ev_loss_bb ?? undefined}
    {context}
    source={kitSource(d.source)}
    sourceHint={d.source ? SOURCE[d.source].explain : undefined}
  >
    {#if split.reason}<p>{split.reason}</p>{:else if !d.best}<p>{split.head}</p>{/if}

    <div class="strategy">
      <div class="mix" role="img" aria-label="Was {d.source ? SOURCE[d.source].label : 'die Berechnung'} mit deiner Hand spielt">
        {#each d.strategy.filter((c) => c.freq >= 0.005 || c.played) as c (c.label)}
          <span class="seg {tone(c)}" class:mine={c.played} style:flex-grow={Math.max(c.freq, 0.015)} title="{c.label}: {pct(c.freq)}"></span>
        {/each}
      </div>
      <ul class="options">
        <li class="head" aria-hidden="true">
          <span></span>
          <span>So spielt die Lösung deine Hand</span>
          <span class="num">Anteil</span>
          <abbr title={EV_LOSS_EXPLAIN}>Verlust</abbr>
        </li>
        {#each d.strategy as c (c.label)}
          <li class:mine={c.played}>
            <span class="dot {tone(c)}"></span>
            <span class="label">{c.label}{#if c.played}<em>du</em>{/if}</span>
            <span class="num freq">{pct(c.freq)}</span>
            <span class="num ev" class:best={gap(c) === "beste"} title={EV_LOSS_EXPLAIN}>{gap(c)}</span>
          </li>
        {/each}
      </ul>
    </div>

    {#if d.note}
      <details>
        <summary>Was angenommen wurde</summary>
        <p>{d.note}</p>
      </details>
    {/if}

    {#snippet actions()}
      {#if prefill || chartable(d) || learn.length}
        {#each learn as l (l.label)}
          <Button size="sm" variant="quiet" onclick={() => onLearn?.(l.target)}>{l.label}</Button>
        {/each}
        <ChartsButton decision={d} variant="primary" />
        {#if prefill}
          <Button variant="primary" onclick={() => openInSolver(prefill)} title="Öffnet die Lösung im Solver und spielt die Hand bis zu dieser Entscheidung durch.">
            Im Solver öffnen
          </Button>
        {/if}
      {/if}
    {/snippet}
  </Verdict>
  <p class="rule" title={RATING_RULE}>Wie bewertet wird</p>
{:else}
  <article class="card" class:wait={d.status === "pending"}>
    <header>
      {#if d.status === "pending"}
        <span class="pill"><i class="spin"></i>{solving ? "Wird gerechnet" : "Wartet auf den Solver"}</span>
      {:else}
        <span class="pill"><i class="none"></i>Nicht bewertet</span>
      {/if}
      <span class="spot" title={SPOT_HINT[d.spot]}>{context}</span>
    </header>
    {#if d.status === "pending"}
      <p class="head">Dein {d.played.split(" ")[0]} wird gerade nachgerechnet.</p>
      {#if solving}
        <div class="progress" aria-label="Fortschritt des Solvers">
          <i style:width="{Math.min(100, (solving.iteration / solving.max_iterations) * 100)}%"></i>
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
  </article>
{/if}

<style>
  .strategy {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    margin-top: var(--sp-1);
  }
  .mix {
    display: flex;
    gap: 2px;
    height: 12px;
  }
  .seg {
    flex-basis: 0;
    min-width: 4px;
    border-radius: var(--r-xs);
  }
  .seg.mine {
    box-shadow:
      0 0 0 2px var(--surface),
      0 0 0 3.5px var(--text);
  }
  .fold {
    background: var(--act-fold);
  }
  .passive {
    background: var(--act-call);
  }
  .raise {
    background: var(--act-bet-3);
  }
  .allin {
    background: var(--act-allin);
  }
  .options {
    display: grid;
    gap: 1px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .options li {
    display: grid;
    grid-template-columns: 10px minmax(0, 1fr) 48px 76px;
    align-items: center;
    gap: var(--sp-2);
    min-height: 24px;
    padding: 0 6px;
    border-radius: var(--r-xs);
    color: var(--text-2);
  }
  .options li.head {
    min-height: 20px;
    color: var(--text-3);
    font-size: var(--fs-xs);
  }
  .options li.head > :nth-child(n + 3) {
    text-align: right;
  }
  .options li.mine {
    background: color-mix(in srgb, var(--text) 8%, transparent);
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
    border-radius: var(--r-xs);
    background: var(--text);
    color: var(--bg);
    font-size: var(--fs-xs);
    font-style: normal;
    font-weight: var(--fw-semibold);
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
  abbr {
    text-decoration: underline dotted;
    text-underline-offset: 3px;
    cursor: help;
  }
  details {
    margin-top: var(--sp-1);
  }
  summary {
    width: fit-content;
    color: var(--text-3);
    font-size: var(--fs-sm);
    cursor: pointer;
    transition: color var(--dur-fast) var(--ease-standard);
  }
  summary:hover {
    color: var(--text-2);
  }
  details p {
    margin: 6px 0 0;
    font-size: var(--fs-sm);
  }
  .rule {
    width: fit-content;
    margin: 6px 0 0 auto;
    color: var(--text-3);
    font-size: var(--fs-xs);
    text-decoration: underline dotted;
    text-underline-offset: 3px;
    cursor: help;
  }

  .card {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    padding: var(--sp-3) var(--sp-4);
    border: 1px dashed var(--line-2);
    border-radius: var(--r-md);
    background: var(--surface);
    font-size: var(--fs-md);
  }
  .card.wait {
    border-color: color-mix(in srgb, var(--amber) 45%, var(--line));
  }
  header {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px var(--sp-3);
  }
  .pill {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    color: var(--text);
    font-weight: var(--fw-semibold);
  }
  .pill i {
    width: 12px;
    height: 12px;
    border-radius: 50%;
  }
  .spin {
    border: 2px solid var(--amber);
    border-right-color: transparent;
    animation: spin var(--dur-loop) var(--ease-spin) infinite;
  }
  .none {
    border: 1.5px dashed var(--text-3);
  }
  .spot {
    color: var(--text-2);
    cursor: help;
  }
  .head {
    margin: 0;
    color: var(--text);
    font-size: var(--fs-base);
    font-weight: var(--fw-semibold);
  }
  .reason {
    margin: 0;
    color: var(--text-2);
    line-height: var(--lh-base);
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
    transition: width var(--dur-slow) var(--ease-standard);
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
