<script lang="ts">
  import type { LearnTarget } from "../app/Leaks.svelte";
  import ChartsButton from "../charts/ChartsButton.svelte";
  import { mixText, playsLine, type MoveKind, type Who } from "../play/words";
  import { openInSolver } from "../solver/handoff.svelte";
  import type { SolverPrefill } from "../solver/types";
  import Button from "../ui/Button.svelte";
  import ProgressBar from "../ui/ProgressBar.svelte";
  import SourceNote from "../ui/SourceNote.svelte";
  import Verdict from "../ui/Verdict.svelte";
  import { EV_LOSS_EXPLAIN, GRADE_OF, KIT_SOURCE, LEARN, RATING_RULE, SOURCE, SPOT, SPOT_HINT, bb, pct, reliabilityText } from "./labels";
  import { analysis } from "./store.svelte";
  import type { Choice, Decision, Source } from "./types";

  /**
   * One decision: the verdict with the practice's grades and sentences first, then what the source
   * plays with the hand, and where the numbers come from.
   */
  let { decision: d, onLearn }: { decision: Decision; onLearn?: (target: LearnTarget) => void } = $props();

  const STREET = { preflop: "Preflop", flop: "Flop", turn: "Turn", river: "River" } as const;
  /** The kit's lines speak of "the solution"; the other sources say who plays it. */
  const LEAD: Partial<Record<Source, { best: string; mix: string }>> = {
    equilibrium: { best: "Genau so spielt es das Gleichgewicht.", mix: "Das Gleichgewicht spielt hier beides – deine Wahl gehört dazu." },
    model: { best: "Genau so spielt es das Modell.", mix: "Das Modell spielt hier beides – deine Wahl gehört dazu." },
    estimate: { best: "Nach der Einschätzung die beste Wahl.", mix: "Nach der Einschätzung so gut wie die beste Wahl." },
  };
  const WHO: Record<Source, Who> = { model: "model", equilibrium: "equilibrium", solver: "solution", estimate: "solution", forced: "solution" };

  const grade = $derived(d.rating ? GRADE_OF[d.rating] : null);
  const context = $derived(SPOT[d.spot] === STREET[d.street] ? STREET[d.street] : `${SPOT[d.spot]} · ${STREET[d.street]}`);
  const kind = (c: Choice): MoveKind => (c.kind || "call") as MoveKind;
  const played = $derived(d.strategy.find((c) => c.played) ?? null);
  const main = $derived(d.strategy.length ? d.strategy.reduce((a, b) => (b.freq > a.freq ? b : a)) : null);
  const bestEv = $derived(Math.max(...d.strategy.map((c) => c.ev_bb ?? -Infinity)));
  const potShare = $derived(d.ev_loss_bb !== null && d.pot_bb ? d.ev_loss_bb / d.pot_bb : undefined);
  const solving = $derived(analysis.status?.solving?.hand_id === d.hand_id ? analysis.status.solving : null);

  /** The reason under the verdict: the practice's sentences, then what the source adds. */
  const reason = $derived.by(() => {
    if (!d.source || !played) return d.why;
    if (d.source === "estimate") {
      // The estimate rates the kind of move, not its size.
      const word = (c: Choice) => (c.kind === "fold" ? "Folden" : c.kind === "check" ? "Checken" : c.kind === "call" ? "Callen" : "Setzen oder Erhöhen");
      const kinds = [...new Set(d.strategy.filter((c) => c.freq > 0).map(word))];
      const lead = kinds.length ? `Nach der Einschätzung ist hier ${kinds.join(" oder ")} am besten; die Größe bewertet sie nicht.` : "";
      return `${lead} ${d.why}`.trim();
    }
    const who = WHO[d.source];
    if (d.rating === "mix" && main) {
      return mixText(who, "deiner Hand", { kind: kind(main), f: main.freq, ev: main.ev_bb }, { kind: kind(played), f: played.freq, ev: played.ev_bb });
    }
    const plays = playsLine(who, "deiner Hand", d.strategy.map((c) => ({ kind: kind(c), f: c.freq })));
    return d.why ? `${plays} ${d.why}` : plays;
  });

  const prefill = $derived.by((): SolverPrefill | null => {
    if (!d.solver) return null;
    try {
      return JSON.parse(d.solver) as SolverPrefill;
    } catch {
      return null;
    }
  });

  function tone(c: Choice): string {
    if (c.kind === "fold") return "fold";
    if (c.kind === "all_in") return "allin";
    if (c.kind === "bet" || c.kind === "raise") return "raise";
    return "passive";
  }

  function gap(c: Choice): string {
    if (c.ev_bb === null || !isFinite(bestEv)) return "";
    const loss = bestEv - c.ev_bb;
    return loss < 0.005 ? "beste" : `−${bb(loss, 2)}`;
  }
</script>

{#if d.status === "rated" && grade && d.source}
  {@const kit = KIT_SOURCE[d.source]}
  <div class="decision">
    <Verdict
      {grade}
      played={d.played}
      better={d.best && d.best !== d.played ? d.best : undefined}
      cost={d.ev_loss_bb ?? undefined}
      {potShare}
      {context}
      lead={grade === "best" || grade === "mix" ? LEAD[d.source]?.[grade] : undefined}
    >
      <p>{reason}</p>
      {#snippet actions()}
        <ChartsButton decision={d} />
        {#if prefill}
          <Button size="sm" onclick={() => prefill && openInSolver(prefill)} title="Öffnet die Lösung im Solver und spielt die Hand bis zu dieser Entscheidung durch.">Im Solver öffnen</Button>
        {/if}
        {#if onLearn && grade !== "best" && grade !== "mix"}
          {#each LEARN[d.spot] as l (l.label)}
            <Button size="sm" variant="quiet" onclick={() => onLearn(l.target)}>{l.label}</Button>
          {/each}
        {/if}
      {/snippet}
    </Verdict>

    <div class="mix" role="img" aria-label="Was {SOURCE[d.source].label} mit deiner Hand spielt">
      {#each d.strategy.filter((c) => c.freq >= 0.005 || c.played) as c (c.label)}
        <span class="seg {tone(c)}" class:mine={c.played} style="flex-grow:{Math.max(c.freq, 0.015)}" title="{c.label}: {pct(c.freq)}"></span>
      {/each}
    </div>
    <ul class="options">
      {#each d.strategy as c (c.label)}
        <li class:mine={c.played}>
          <span class="dot {tone(c)}"></span>
          <span class="label">{c.label}{#if c.played}<em>du</em>{/if}</span>
          <span class="num freq">{d.source === "estimate" ? "" : pct(c.freq)}</span>
          <span class="num ev" class:best={gap(c) === "beste"} title={EV_LOSS_EXPLAIN}>{gap(c)}</span>
        </li>
      {/each}
    </ul>

    {#if kit}
      <SourceNote kind={kit} variant="block">
        {SOURCE[d.source].explain}{#if d.source === "estimate" && d.reliability !== null}{" "}<b class="num">{reliabilityText(d.reliability, d.street)}</b>{/if}
      </SourceNote>
    {/if}
    <details>
      <summary>Was angenommen wurde</summary>
      <p>{d.note}</p>
      <p class="faint">{RATING_RULE}</p>
    </details>
  </div>
{:else if d.status === "pending"}
  <div class="decision plain">
    <p class="head">Dein {d.played.split(" ")[0]} wird gerade nachgerechnet.</p>
    {#if solving}
      <ProgressBar value={Math.min(1, solving.iteration / solving.max_iterations)} label="Fortschritt des Solvers" />
      <p class="reason">Der Solver rechnet diese Hand ab dem Flop durch ({Math.round(solving.elapsed_ms / 1000)} s). Meist reicht weniger als eine Minute.</p>
    {:else}
      <p class="reason">Diese Hand steht ganz vorn in der Warteschlange und wird als Nächstes gerechnet.</p>
    {/if}
    <SourceNote kind="solver" variant="block">{SOURCE.solver.explain}</SourceNote>
  </div>
{:else if d.status === "forced"}
  <div class="decision plain">
    <p class="head">{d.played} – keine Entscheidung</p>
    <p class="reason">{d.why} Zählt nicht in deine Bewertung.</p>
  </div>
{:else}
  <div class="decision plain">
    <p class="head">Diese Entscheidung bewertet Ringside nicht.</p>
    <p class="reason">{d.reason}</p>
    <p class="reason faint" title={SPOT_HINT[d.spot]}>{context}</p>
  </div>
{/if}

<style>
  .decision {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    font-size: var(--fs-md);
  }
  .plain {
    gap: var(--sp-2);
    padding: var(--sp-3) var(--sp-4);
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    background: var(--surface);
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
    line-height: 1.45;
  }
  .mix {
    display: flex;
    gap: 2px;
    height: 12px;
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
    background: var(--act-fold);
  }
  .passive {
    background: var(--act-call);
  }
  .raise {
    background: var(--act-raise);
  }
  .allin {
    background: var(--act-allin);
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
    gap: var(--sp-2);
    padding: 2px var(--sp-1);
    border-radius: var(--r-sm);
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
    font-size: var(--fs-xs);
    font-style: normal;
    font-weight: var(--fw-semibold);
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
  details {
    color: var(--text-2);
  }
  summary {
    cursor: pointer;
    color: var(--text-3);
    font-size: var(--fs-sm);
  }
  summary:hover {
    color: var(--text-2);
  }
  details p {
    margin: var(--sp-2) 0 0;
    line-height: 1.45;
    font-size: var(--fs-sm);
  }
</style>
