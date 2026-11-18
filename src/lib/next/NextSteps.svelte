<script lang="ts">
  import type { Snippet } from "svelte";
  import type { LearnTarget } from "../app/Leaks.svelte";
  import Badge from "../ui/Badge.svelte";
  import Button from "../ui/Button.svelte";
  import Term from "../ui/Term.svelte";
  import { WORDS } from "../ui/words";
  import type { NextStep, StepHands } from "./types";

  /**
   * „Was als Nächstes?“ from the one ranked list of the backend. `first` shows the top entry as
   * the page's next step, `list` the entries in their order. Pages with their own focus pass a
   * filtered list, never a list of their own.
   */
  let {
    steps,
    mode = "list",
    limit = Infinity,
    label = "Dein nächster Schritt",
    primary = true,
    compact = false,
    onLearn,
    onHands,
    note,
  }: {
    steps: NextStep[];
    mode?: "first" | "list";
    limit?: number;
    /** Small heading of the first step. */
    label?: string;
    /** The first step's „Üben“ is the page's main action. */
    primary?: boolean;
    /** Narrow column: no frame and no bar, actions under the text. */
    compact?: boolean;
    onLearn: (target: LearnTarget) => void;
    onHands?: (hands: StepHands) => void;
    /** A line under an entry, e.g. what it cost in one tournament. */
    note?: Snippet<[NextStep]>;
  } = $props();

  const shown = $derived(mode === "first" ? steps.slice(0, 1) : steps.slice(0, limit));
  const top = $derived(Math.max(...shown.map((s) => s.score), 0.01));
  const cap = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);

  /** The title split around its poker word, so only that word carries the explanation. */
  function parts(s: NextStep): [string, string, string] | null {
    if (!s.term) return null;
    const w = WORDS[s.term].word;
    const i = s.title.indexOf(w);
    return i < 0 ? null : [s.title.slice(0, i), w, s.title.slice(i + w.length)];
  }
</script>

{#snippet arrow()}<svg viewBox="0 0 24 24" width="14" height="14" aria-hidden="true"><path d="M5 12h14M13 6l6 6-6 6" /></svg>{/snippet}

{#snippet title(s: NextStep)}
  {@const p = parts(s)}
  {#if p && s.term}{p[0]}<Term id={s.term}>{p[1]}</Term>{p[2]}{:else}{s.title}{/if}
{/snippet}

{#snippet actions(s: NextStep, main: boolean)}
  {#if s.practice}
    {@const target = s.practice}
    <Button variant={main ? "primary" : "secondary"} size={main ? "md" : "sm"} icon={arrow} onclick={() => onLearn(target)}>Üben</Button>
  {/if}
  {#if s.hands && onHands}
    {@const hands = s.hands}
    <Button variant={s.practice ? "quiet" : main ? "primary" : "secondary"} size={main ? "md" : "sm"} onclick={() => onHands(hands)}>Hände ansehen</Button>
  {/if}
  {#if s.read}
    {@const article = s.read}
    <Button variant="quiet" size={main ? "md" : "sm"} onclick={() => onLearn({ article })}>Nachlesen</Button>
  {/if}
{/snippet}

{#if mode === "first" && shown[0]}
  {@const s = shown[0]}
  <div class="first" data-step={s.id}>
    <span class="label">{label}{#if s.estimate}<Badge tone="warn" size="sm" dashed title="Die Zahl stützt sich auf eine Schätzung, nicht auf eine Lösung.">geschätzt</Badge>{/if}</span>
    <b class="step-title">{@render title(s)}</b>
    <p class="number num">{cap(s.number)}.</p>
    <p class="why">{s.why}</p>
    {#if note}{@render note(s)}{/if}
    <div class="go">{@render actions(s, primary)}</div>
  </div>
{:else if mode === "list" && shown.length}
  <ol class="rows" class:compact>
    {#each shown as s, i (s.id)}
      <li class="row" data-step={s.id}>
        <span class="rank num" aria-hidden="true">{i + 1}</span>
        <div class="what">
          <b class="step-title">{@render title(s)}</b>
          <span class="number num">{cap(s.number)}{#if s.estimate}&nbsp;<Badge tone="warn" size="sm" dashed>geschätzt</Badge>{/if}</span>
          <span class="why">{s.why}</span>
          {#if note}{@render note(s)}{/if}
        </div>
        {#if !compact}<span class="bar" aria-hidden="true"><span class="fill" class:estimate={s.estimate} style:width="{Math.max(4, (s.score / top) * 100)}%"></span></span>{/if}
        <div class="go">{@render actions(s, primary && i === 0)}</div>
      </li>
    {/each}
  </ol>
{/if}

<style>
  .first {
    display: grid;
    gap: 6px;
    justify-items: start;
  }
  .label {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-size: var(--fs-xs);
    font-weight: var(--fw-semibold);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-2);
  }
  .first .step-title {
    font-size: var(--fs-lg);
    font-weight: var(--fw-semibold);
    line-height: 1.25;
  }
  .number {
    margin: 0;
    font-size: var(--fs-md);
    color: var(--text);
  }
  .why {
    margin: 0;
    font-size: var(--fs-sm);
    color: var(--text-2);
    line-height: 1.45;
  }
  .go {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
  }
  .first .go {
    margin-top: 6px;
  }
  .rows {
    display: grid;
    gap: 6px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .row {
    display: grid;
    grid-template-columns: 34px minmax(0, 1fr) minmax(90px, 160px) auto;
    align-items: center;
    gap: 8px 16px;
    padding: 12px 14px;
    border-radius: var(--r);
    border: 1px solid var(--line);
    background: var(--surface);
  }
  .rank {
    display: grid;
    place-items: center;
    width: 34px;
    height: 34px;
    border-radius: 50%;
    background: color-mix(in srgb, var(--neg) 14%, var(--surface-2));
    color: var(--text);
    font-weight: var(--fw-semibold);
  }
  .what {
    display: grid;
    gap: 3px;
    min-width: 0;
  }
  .row .step-title {
    font-size: var(--fs-base);
    font-weight: var(--fw-semibold);
  }
  .bar {
    height: 6px;
    border-radius: 3px;
    background: var(--surface-2);
    overflow: hidden;
  }
  .fill {
    display: block;
    height: 100%;
    border-radius: 3px;
    background: var(--neg);
  }
  .fill.estimate {
    background: repeating-linear-gradient(135deg, var(--neg) 0 4px, color-mix(in srgb, var(--neg) 45%, transparent) 4px 8px);
  }
  .compact {
    gap: 14px;
  }
  .compact .row {
    grid-template-columns: 22px minmax(0, 1fr);
    gap: 6px 10px;
    padding: 0;
    border: 0;
    background: none;
  }
  .compact .rank {
    width: 22px;
    height: 22px;
    font-size: var(--fs-md);
  }
  .compact .go {
    grid-column: 2;
  }
  @container (max-width: 760px) {
    .row {
      grid-template-columns: 34px minmax(0, 1fr);
    }
    .bar,
    .row .go {
      grid-column: 2;
    }
  }
</style>
