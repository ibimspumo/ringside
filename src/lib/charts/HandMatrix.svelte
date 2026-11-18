<script lang="ts" module>
  export type MatrixCell = {
    key: string;
    combos: number;
    /** CSS background of the cell. */
    bg: string;
    /** Some action is played: light ink, which every action colour carries at 4.5:1 or more. */
    filled: boolean;
    /** Small number at the bottom right, e.g. the share of a mixed hand. */
    badge: number | null;
    /** The hand never reaches this spot. */
    out?: boolean;
    /** Faded: it does not take the action picked in the action bar. */
    dim?: boolean;
    /** What a screen reader hears. */
    label: string;
  };
</script>

<script lang="ts">
  import { untrack, type Snippet } from "svelte";
  import { floating, type Anchor } from "../app/format";
  import Hint from "./Hint.svelte";
  import { TERM } from "./words";

  /**
   * 13×13 starting-hand grid. Hovering shows a small card at the pointer that follows it; a click
   * (or Enter) pins a larger card next to the hand, Esc or a click elsewhere closes it. Arrow keys
   * move through the hands.
   */
  let {
    cells,
    tip,
    card,
    pin = null,
  }: {
    cells: MatrixCell[];
    tip: Snippet<[number]>;
    card: Snippet<[number]>;
    /** Hand to pin, e.g. the one played in a hand history. */
    pin?: string | null;
  } = $props();

  let grid = $state<HTMLDivElement>();
  let cardEl = $state<HTMLDivElement>();
  let hover = $state<{ i: number; anchor: Anchor; side: "right" | "top" } | null>(null);
  let pinned = $state<{ i: number; anchor: Anchor } | null>(null);
  let focusIdx = $state(0);

  // A new spot fades the grid in, so a change of seat or stack reads as a change, not a flicker.
  // Two equal animations, swapped, restart it.
  let swap = $state(false);
  const look = $derived(cells.map((c) => c.bg).join("|"));
  $effect(() => {
    void look;
    swap = !untrack(() => swap);
  });

  const cellEl = (i: number) => grid?.querySelector<HTMLElement>(`[data-i="${i}"]`) ?? null;

  function rectOf(i: number): Anchor | null {
    const r = cellEl(i)?.getBoundingClientRect();
    return r ? { left: r.left, top: r.top, right: r.right, bottom: r.bottom } : null;
  }

  function move(e: PointerEvent) {
    const t = (e.target as HTMLElement).closest<HTMLElement>("[data-i]");
    if (!t) return (hover = null);
    const i = Number(t.dataset.i);
    if (pinned?.i === i) return (hover = null);
    // A box around the pointer, so the card sits beside the cursor and flips at the window edge.
    const { clientX: x, clientY: y } = e;
    hover = { i, anchor: { left: x - 10, right: x + 14, top: y + 14, bottom: y + 14 }, side: "right" };
  }

  function toggle(i: number) {
    hover = null;
    const anchor = rectOf(i);
    pinned = pinned?.i === i || !anchor ? null : { i, anchor };
  }

  function key(e: KeyboardEvent, i: number) {
    const step: Record<string, number> = { ArrowRight: 1, ArrowLeft: -1, ArrowDown: 13, ArrowUp: -13 };
    if (e.key === "Escape" && pinned) {
      e.stopPropagation();
      pinned = null;
      return;
    }
    const d = step[e.key];
    if (d === undefined) return;
    e.preventDefault();
    const next = i + d;
    if (next < 0 || next > 168 || (Math.abs(d) === 1 && Math.floor(next / 13) !== Math.floor(i / 13))) return;
    focusIdx = next;
    cellEl(next)?.focus();
  }

  function focused(e: FocusEvent, i: number) {
    focusIdx = i;
    const el = e.currentTarget as HTMLElement;
    if (!el.matches(":focus-visible") || pinned?.i === i) return;
    const anchor = rectOf(i);
    if (anchor) hover = { i, anchor, side: "top" };
  }

  function outside(e: PointerEvent) {
    if (!pinned) return;
    const t = e.target as Node;
    if (grid?.contains(t) || cardEl?.contains(t)) return;
    pinned = null;
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape" && pinned) {
      pinned = null;
      cellEl(focusIdx)?.focus();
    }
  }

  // The card stays beside its hand while the page (or any scrolling parent) moves.
  function rescroll() {
    if (pinned) requestAnimationFrame(() => pinned && (pinned = { i: pinned.i, anchor: rectOf(pinned.i) ?? pinned.anchor }));
  }
  $effect(() => {
    window.addEventListener("scroll", rescroll, { capture: true, passive: true });
    return () => window.removeEventListener("scroll", rescroll, { capture: true });
  });

  // A hand to show (from a hand history or the preview): pin its card once the layout is final.
  $effect(() => {
    const key = pin;
    if (!key) return;
    const i = untrack(() => cells.findIndex((c) => c.key === key));
    if (i < 0) return;
    focusIdx = i;
    const t = setTimeout(() => {
      cellEl(i)?.scrollIntoView({ block: "nearest" });
      const anchor = rectOf(i);
      if (anchor) pinned = { i, anchor };
    }, 300);
    return () => clearTimeout(t);
  });

</script>

<svelte:window onpointerdown={outside} onkeydown={onKey} onresize={rescroll} />

<div
  class="matrix"
  class:swap
  role="grid"
  aria-label="Starthände: Paare auf der Diagonale, suited oben rechts, offsuit unten links"
  tabindex="-1"
  bind:this={grid}
  onpointermove={move}
  onpointerleave={() => (hover = null)}
>
  {#each cells as c, i (i)}
    <button
      type="button"
      class="cell"
      class:filled={c.filled}
      class:out={c.out}
      class:dim={c.dim}
      class:pinned={pinned?.i === i}
      style={c.out ? undefined : `background:${c.bg}`}
      data-i={i}
      tabindex={i === focusIdx ? 0 : -1}
      aria-label={c.label}
      aria-pressed={pinned?.i === i}
      onclick={() => toggle(i)}
      onkeydown={(e) => key(e, i)}
      onfocus={(e) => focused(e, i)}
      onblur={() => hover?.side === "top" && (hover = null)}
    >
      <span class="k">{c.key}</span>
      {#if c.badge !== null}<span class="v num">{c.badge}</span>{/if}
    </button>
  {/each}
</div>

{#if hover}
  <div class="tip hand-tip" use:floating={{ anchor: hover.anchor, side: hover.side }}>
    {@render tip(hover.i)}
    <footer>{hover.side === "top" ? "Enter" : "Klick"}: Details anheften</footer>
  </div>
{/if}

{#if pinned}
  {@const c = cells[pinned.i]}
  <div class="popover hand-card" bind:this={cardEl} role="dialog" aria-label="Details zu {c.key}" use:floating={{ anchor: pinned.anchor, side: "right" }}>
    <header>
      <b class="name">{c.key}</b>
      <span class="combos"><Hint text={TERM.combos}>{c.combos} Kombos</Hint></span>
      <button type="button" class="icon-btn close" aria-label="Schließen (Esc)" title="Schließen (Esc)" onclick={() => (pinned = null)}>
        <svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true"><path d="M6 6l12 12M18 6L6 18" /></svg>
      </button>
    </header>
    {@render card(pinned.i)}
  </div>
{/if}

<style>
  .matrix {
    display: grid;
    grid-template-columns: repeat(13, minmax(0, 1fr));
    gap: 2px;
    outline: none;
    animation: settle-a var(--dur-slow) var(--ease-out);
  }
  .matrix.swap {
    animation-name: settle-b;
  }
  @keyframes settle-a {
    from {
      opacity: 0.4;
    }
  }
  @keyframes settle-b {
    from {
      opacity: 0.4;
    }
  }
  .cell {
    position: relative;
    aspect-ratio: 1;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    padding: 3px 4px;
    border: 0;
    border-radius: 3px;
    overflow: hidden;
    cursor: pointer;
    min-width: 0;
    color: var(--text-2);
    background: var(--surface-2);
    font: inherit;
    text-align: left;
    transition:
      opacity var(--dur) var(--ease-out),
      box-shadow var(--dur-fast) var(--ease-out);
  }
  .cell.filled {
    color: var(--text);
  }
  .cell.out {
    background: transparent;
    box-shadow: inset 0 0 0 1px var(--line-2);
    color: var(--text-2);
  }
  .cell.dim {
    opacity: 0.18;
  }
  .cell:hover {
    outline: 1px solid var(--text);
    z-index: 1;
  }
  .cell:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: 1px;
    z-index: 2;
  }
  .cell.pinned {
    outline: 2px solid var(--text);
    outline-offset: 1px;
    z-index: 2;
  }
  .k {
    font-family: var(--font-num);
    font-size: 11px;
    font-weight: 600;
    line-height: 1;
  }
  .v {
    align-self: flex-end;
    font-size: 11px;
    line-height: 1;
  }

  .hand-tip {
    width: 228px;
    animation: pop-in var(--dur-fast) var(--ease-out);
    transition:
      left 90ms var(--ease-out),
      top 90ms var(--ease-out);
  }
  @keyframes pop-in {
    from {
      opacity: 0;
      transform: translateY(4px);
    }
  }
  .hand-tip footer {
    margin-top: 7px;
    padding-top: 6px;
    border-top: 1px solid var(--line-2);
    font-size: 11.5px;
    color: var(--text-2);
  }

  .hand-card {
    animation: card-in var(--dur) var(--ease-out);
    width: 320px;
    padding: 14px 16px 16px;
    border-radius: var(--r);
    background: var(--surface-2);
    border: 1px solid var(--line-2);
    box-shadow:
      0 18px 48px rgba(0, 0, 0, 0.55),
      0 0 0 1px rgba(0, 0, 0, 0.3);
    font-size: 13px;
    line-height: 1.5;
  }
  @keyframes card-in {
    from {
      opacity: 0;
      transform: scale(0.97);
    }
  }
  .hand-card header {
    display: flex;
    align-items: baseline;
    gap: 10px;
    margin-bottom: 8px;
  }
  .name {
    font-family: var(--font-num);
    font-size: 26px;
    font-weight: 600;
    line-height: 1;
    letter-spacing: 0.01em;
  }
  .combos {
    color: var(--text-2);
    font-size: 12.5px;
  }
  .close {
    margin-left: auto;
    align-self: center;
  }
  .close svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linecap: round;
  }

  @container (max-width: 520px) {
    .cell {
      padding: 2px;
    }
    .k {
      font-size: 9px;
    }
    .v {
      display: none;
    }
  }
</style>
