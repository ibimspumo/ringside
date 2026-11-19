<script lang="ts">
  import type { Snippet } from "svelte";
  import { bb, pct } from "./format";
  import GradeIcon from "./GradeIcon.svelte";
  import { GRADE, type Grade, type Source } from "./poker";
  import Reason from "./Reason.svelte";
  import SourceNote from "./SourceNote.svelte";

  /**
   * Result of one decision. The band on top carries grade, icon and price; for mistakes the first
   * line says what was better and what the played action cost, the reason follows below.
   */
  let {
    grade,
    played,
    better,
    cost,
    potShare,
    context,
    source,
    sourceHint,
    lead,
    reason,
    reasonCollapsed = false,
    children,
    actions,
  }: {
    grade: Grade;
    /** What was played, as written on the button: "Bet 2,4 BB". */
    played: string;
    /** The better action for grades below "good": "Check". */
    better?: string;
    /** Lost big blinds against the best action. */
    cost?: number;
    /** The same loss as share of the pot, 0–1. */
    potShare?: number;
    /** Where the decision was: "River · Q♠8♠". */
    context?: string;
    source?: Source;
    /** What exactly the source computed here, shown on hover instead of the general line. */
    sourceHint?: string;
    /** Replaces the grade's standard line when the strategy is not a solution's. */
    lead?: string;
    /** Why the best move is best, shown as "Warum?" right under the verdict. */
    reason?: string;
    /** "Warum?" one click away, for places where the table beside it needs the room. */
    reasonCollapsed?: boolean;
    /** The reason in one or two sentences. */
    children?: Snippet;
    actions?: Snippet;
  } = $props();

  const g = $derived(GRADE[grade]);
  const costly = $derived(cost !== undefined && cost > 0.005);
</script>

<article class="ui-verdict {grade}" class:bad={g.bad} aria-label="{g.label}{context ? `, ${context}` : ''}">
  <header>
    <GradeIcon {grade} size={22} />
    <div class="what">
      <strong>{g.label}</strong>
      {#if context}<span>{context}</span>{/if}
    </div>
    {#if costly && grade !== "best" && grade !== "mix"}<span class="price" title="So viel bringt diese Entscheidung im Schnitt weniger als die beste – nicht das Ergebnis der Hand"
        ><small>kostet im Schnitt</small><b class="num">−{bb(cost ?? 0, { digits: 2 })}</b></span
      >{/if}
  </header>

  <div class="body">
    {#if g.bad && better}
      <p class="fix">
        Besser: <b>{better}</b> – dein {played}
        {#if costly}
          bringt im Schnitt <b class="num">{bb(cost ?? 0, { digits: 2 })}</b> weniger{#if potShare !== undefined}{" "}<span class="share">({pct(potShare)} des Pots)</span>{/if}.
        {:else}
          ist schwächer.
        {/if}
      </p>
    {:else}
      <p class="lead">
        <b>{played}</b> – {lead ?? g.lead}
        {#if better && !g.bad && grade !== "best"}
          Am meisten bringt <b>{better}</b>.
        {/if}
      </p>
    {/if}
    {#if reason}<Reason text={reason} collapsed={reasonCollapsed} />{/if}
    {#if children}<div class="why">{@render children()}</div>{/if}
    {#if source || actions}
      <footer>
        {#if source}<SourceNote kind={source} hint={sourceHint} />{/if}
        {#if actions}<div class="actions">{@render actions()}</div>{/if}
      </footer>
    {/if}
  </div>
</article>

<style>
  .ui-verdict {
    --c: var(--grade-best);
    --tint: 7%;
    --band: 15%;
    overflow: hidden;
    border-radius: var(--r-md);
    border: 1px solid color-mix(in srgb, var(--c) 32%, var(--line));
    background: color-mix(in srgb, var(--c) var(--tint), var(--surface));
  }
  .mix {
    --c: var(--grade-mix);
  }
  .good {
    --c: var(--grade-good);
  }
  .inaccurate {
    --c: var(--grade-inaccurate);
  }
  .mistake {
    --c: var(--grade-mistake);
    --tint: 9%;
    --band: 22%;
  }
  .blunder {
    --c: var(--grade-blunder);
    --tint: 11%;
    --band: 30%;
  }
  .bad {
    border-width: 1px 1px 1px 4px;
    border-color: var(--c);
  }
  header {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    padding: var(--sp-2) var(--sp-4) var(--sp-2) var(--sp-3);
    background: color-mix(in srgb, var(--c) var(--band), transparent);
  }
  /* In a narrow card the context moves under the grade instead of being cut off. */
  .what {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 0 var(--sp-2);
    min-width: 0;
    flex: 1;
  }
  strong {
    color: color-mix(in srgb, var(--c) 55%, var(--text));
    font-size: var(--fs-lg);
    font-weight: var(--fw-bold);
  }
  .what span {
    color: var(--text-2);
    font-size: var(--fs-md);
  }
  .price {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    color: var(--text);
    line-height: var(--lh-tight);
  }
  .price small {
    color: var(--text-2);
    font-size: var(--fs-xs);
  }
  .price b {
    font-size: var(--fs-lg);
    font-weight: var(--fw-semibold);
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    padding: var(--sp-3) var(--sp-4) var(--sp-4);
  }
  p {
    margin: 0;
  }
  .fix {
    color: var(--text);
    font-size: var(--fs-base);
    font-weight: var(--fw-medium);
    line-height: 1.4;
  }
  .fix b {
    font-weight: var(--fw-bold);
  }
  .share {
    color: var(--text-2);
    font-weight: var(--fw-regular);
    white-space: nowrap;
  }
  .lead {
    color: var(--text);
    font-size: var(--fs-base);
  }
  .why {
    color: var(--text-2);
    font-size: var(--fs-md);
    line-height: 1.5;
  }
  .why :global(p) {
    margin: 0;
  }
  footer {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-3);
    margin-top: var(--sp-1);
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: var(--sp-2);
    margin-left: auto;
  }
</style>
