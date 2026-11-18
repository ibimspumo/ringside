<script lang="ts" module>
  /**
   * One of the 169 hand classes. `mix` splits the cell into the actions' colours by share (left
   * to right), `weight` fills it from the bottom by how much of the class is in the range;
   * `color` is a single fill for heatmaps. `null` means not in the range.
   */
  export type RangeCell = {
    mix?: { color: string; share: number }[];
    color?: string;
    /** 0–1, default 1. */
    weight?: number;
    /** Small figure in the corner, e.g. EV or equity. */
    value?: string;
    /** One line for screen readers and the default hover card. */
    note?: string;
  } | null;
</script>

<script lang="ts">
  import type { Snippet } from "svelte";
  import { classLabel } from "../solver/range";
  import { pointerAnchor, rectOf, type Anchor } from "./floating";
  import Popover from "./Popover.svelte";

  /**
   * The 13×13 range grid of every view. Hover or keyboard focus shows a card beside the cell
   * (`tip`), a click selects the class and pins `card` beside the grid until it is closed.
   * The class label stays on a dark plate with a fixed colour, so it reads on every fill.
   */
  let {
    cells,
    label,
    hero = -1,
    selected = -1,
    dimmed,
    minFill = 0,
    size = "md",
    only,
    columns = 13,
    onSelect,
    onClose,
    tip,
    card,
    clickHint,
    overlay,
  }: {
    cells: RangeCell[];
    /** Names the grid for screen readers. */
    label: string;
    /** Class of the hero's hand, outlined. */
    hero?: number;
    selected?: number;
    /** Classes to push back, e.g. outside a filter; their fill fades, the label stays readable. */
    dimmed?: (i: number) => boolean;
    /** Smallest visible fill height for a class with any weight, 0–1. */
    minFill?: number;
    size?: "sm" | "md";
    /** Show just these classes, e.g. specimens; arrow keys then move in reading order. */
    only?: number[];
    columns?: number;
    onSelect?: (i: number) => void;
    onClose?: () => void;
    tip?: Snippet<[number]>;
    card?: Snippet<[number]>;
    /** Last line of the hover card for selectable classes; defaults to the pin hint when `card` is set. */
    clickHint?: string;
    /** Drawn over the cells in a 13×13 viewBox, e.g. an outline of another range. */
    overlay?: Snippet;
  } = $props();

  const hint = $derived(clickHint ?? (card ? "Klick hält die Hand fest" : undefined));

  const interactive = $derived(!!(onSelect || tip));
  const labels = Array.from({ length: 169 }, (_, i) => classLabel(i));

  function fill(c: NonNullable<RangeCell>): string {
    if (!c.mix?.length) return c.color ?? "var(--text-3)";
    let at = 0;
    const stops: string[] = [];
    for (const m of c.mix) {
      if (m.share <= 0) continue;
      const from = at * 100;
      at += m.share;
      stops.push(`${m.color} ${from.toFixed(2)}% ${(at * 100).toFixed(2)}%`);
    }
    return stops.length ? `linear-gradient(to right, ${stops.join(", ")})` : "transparent";
  }

  function height(c: NonNullable<RangeCell>): number {
    const w = Math.max(0, Math.min(1, c.weight ?? 1));
    return w > 0 ? Math.max(w, minFill) * 100 : 0;
  }

  // One tab stop for the whole grid; arrow keys move between cells.
  let cursor = $state(-1);
  const first = $derived(only?.[0] ?? 0);
  let matrix = $state<HTMLElement>();
  let hover = $state<{ i: number; anchor: Anchor } | null>(null);
  let pinAt = $state<Anchor | null>(null);
  let pinSide = $state<"right" | "bottom">("right");
  const PIN_WIDTH = 340;

  function nav(e: KeyboardEvent, i: number) {
    // The pinned card closes from the grid too: after a click the focus stays on the cell.
    if (e.key === "Escape" && card && selected >= 0) {
      e.preventDefault();
      close();
      return;
    }
    const step: Record<string, number> = { ArrowLeft: -1, ArrowRight: 1, ArrowUp: -13, ArrowDown: 13 };
    const d = step[e.key];
    if (d === undefined) return;
    e.preventDefault();
    e.stopPropagation();
    if (only) {
      const at = only.indexOf(i) + Math.sign(d) * (Math.abs(d) === 1 ? 1 : columns);
      if (at < 0 || at >= only.length) return;
      cursor = only[at];
      (matrix?.children[at] as HTMLElement | undefined)?.focus();
      return;
    }
    const next = i + d;
    if (next < 0 || next >= 169 || (Math.abs(d) === 1 && Math.floor(next / 13) !== Math.floor(i / 13))) return;
    cursor = next;
    (matrix?.children[next] as HTMLElement | undefined)?.focus();
  }

  function point(e: PointerEvent) {
    if (!tip || e.pointerType === "touch") return;
    const cell = (e.target as Element).closest<HTMLElement>("[data-i]");
    if (!cell) return;
    hover = { i: Number(cell.dataset.i), anchor: pointerAnchor(e, cell) };
  }

  function focused(e: FocusEvent, i: number) {
    cursor = i;
    const el = e.currentTarget as HTMLElement;
    if (tip && el.matches(":focus-visible")) hover = { i, anchor: rectOf(el) };
  }

  // The pinned card sits beside the grid on the row of its class and follows it when the page scrolls.
  const childOf = (i: number) => matrix?.children[only ? only.indexOf(i) : i];

  function measure() {
    const cell = childOf(selected);
    if (!matrix || selected < 0 || !cell || !card) return void (pinAt = null);
    const m = rectOf(matrix);
    const c = rectOf(cell);
    // Beside the grid when there is room in the content area, otherwise under (or over) the
    // class's row, so the card never covers the navigation.
    const area = rectOf(matrix.closest("main") ?? document.documentElement);
    const room = PIN_WIDTH + 16;
    if (area.right - m.right >= room || m.left - area.left >= room) {
      pinAt = { left: m.left, right: m.right, top: c.top, bottom: c.bottom };
      pinSide = "right";
    } else {
      pinAt = { left: c.left, right: c.right, top: c.top, bottom: c.bottom };
      pinSide = "bottom";
    }
  }
  $effect(measure);
  $effect(() => {
    let frame = 0;
    const again = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(measure);
    };
    window.addEventListener("scroll", again, { capture: true, passive: true });
    window.addEventListener("resize", again);
    return () => {
      cancelAnimationFrame(frame);
      window.removeEventListener("scroll", again, { capture: true });
      window.removeEventListener("resize", again);
    };
  });

  function close() {
    const at = selected;
    onClose?.();
    (childOf(at) as HTMLElement | undefined)?.focus();
  }

  const aria = (i: number, c: RangeCell) => (c ? `${labels[i]}${c.note ? `: ${c.note}` : ""}` : `${labels[i]}: nicht in der Range`);
</script>

{#snippet face(i: number, c: RangeCell)}
  {#if c}<i class="fill" style:height="{height(c)}%" style:background={fill(c)}></i>{/if}
  <span class="k">{labels[i]}</span>
  {#if c?.value}<span class="v num">{c.value}</span>{/if}
{/snippet}

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="ui-range {size}"
  class:interactive
  role={interactive ? "grid" : "img"}
  aria-label={label}
  style:grid-template-columns="repeat({columns}, minmax(0, 1fr))"
  bind:this={matrix}
  onpointermove={point}
  onpointerleave={() => (hover = null)}
  onfocusout={(e) => !matrix?.contains(e.relatedTarget as Node | null) && (hover = null)}
>
  {#each only ?? cells.map((_, i) => i) as i (i)}
    {@const c = cells[i]}
    {#if interactive}
      <button
        type="button"
        class="cell"
        class:in={!!c}
        class:hero={i === hero}
        class:sel={i === selected}
        class:faded={dimmed?.(i)}
        data-i={i}
        tabindex={i === (cursor < 0 ? first : cursor) ? 0 : -1}
        aria-label={aria(i, c)}
        aria-pressed={onSelect ? i === selected : undefined}
        onfocus={(e) => focused(e, i)}
        onkeydown={(e) => nav(e, i)}
        onclick={() => c && onSelect?.(i)}>{@render face(i, c)}</button
      >
    {:else}
      <div class="cell" class:in={!!c} class:hero={i === hero} class:faded={dimmed?.(i)} data-i={i} title={aria(i, c)}>{@render face(i, c)}</div>
    {/if}
  {/each}
  {#if overlay}
    <svg class="layer" viewBox="0 0 13 13" preserveAspectRatio="none" aria-hidden="true">{@render overlay()}</svg>
  {/if}
</div>

{#if tip && hover && hover.i !== selected}
  <Popover anchor={hover.anchor} width={252}>
    {@render tip(hover.i)}
    {#if onSelect && hint && cells[hover.i]}<p class="ui-range-hint">{hint}</p>{/if}
  </Popover>
{/if}

{#if card && pinAt && selected >= 0}
  <Popover anchor={pinAt} side={pinSide} pinned width={PIN_WIDTH} label="Hand {labels[selected]}" onclose={close}>
    {@render card(selected)}
  </Popover>
{/if}

<style>
  .ui-range {
    position: relative;
    container-type: inline-size;
    display: grid;
    grid-template-columns: repeat(13, minmax(0, 1fr));
    gap: 2px;
  }
  .sm {
    gap: 1px;
  }
  .cell {
    position: relative;
    aspect-ratio: 1;
    min-width: 0;
    padding: 0;
    overflow: hidden;
    border: 0;
    border-radius: var(--r-xs);
    background: var(--grid-cell-out);
    font: inherit;
    text-align: left;
  }
  .sm .cell {
    border-radius: 2px;
  }
  .cell.in {
    background: var(--grid-cell);
  }
  .interactive .cell.in {
    cursor: pointer;
  }
  .fill {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
  }
  .faded .fill {
    opacity: var(--grid-dim);
  }
  .k {
    position: absolute;
    top: 2px;
    left: 2px;
    padding: 2px 3px 1px;
    border-radius: 2px;
    background: var(--grid-plate);
    color: var(--grid-label);
    font-family: var(--font-num);
    font-size: clamp(var(--fs-grid-min), 2.4cqi, var(--fs-xs));
    font-weight: var(--fw-semibold);
    line-height: 1;
    letter-spacing: -0.01em;
  }
  .cell:not(.in) .k {
    background: none;
    color: var(--grid-label-out);
    font-weight: var(--fw-medium);
  }
  .sm .k {
    top: 1px;
    left: 1px;
    padding: 1px 2px 0;
  }
  .v {
    position: absolute;
    right: 2px;
    bottom: 2px;
    padding: 1px 3px;
    border-radius: 2px;
    background: var(--grid-plate);
    color: var(--grid-label);
    font-size: clamp(var(--fs-grid-min), 2.1cqi, var(--fs-xs));
    line-height: 1;
  }
  @container (max-width: 360px) {
    .v {
      display: none;
    }
  }
  .cell.hero {
    outline: 2px solid var(--text);
    outline-offset: -1px;
    z-index: 1;
  }
  .cell.sel {
    outline: 2px solid var(--focus);
    outline-offset: -1px;
    z-index: 2;
  }
  .interactive .cell.in:hover {
    outline: 1px solid var(--text);
    outline-offset: -1px;
    z-index: 2;
  }
  .cell:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: -1px;
    z-index: 3;
  }
  .layer {
    position: absolute;
    inset: -1px;
    width: calc(100% + 2px);
    height: calc(100% + 2px);
    overflow: visible;
    pointer-events: none;
    z-index: 4;
  }
  :global(.ui-range-hint) {
    margin: var(--sp-2) 0 0;
    padding-top: 6px;
    border-top: 1px solid var(--line);
    color: var(--text-3);
    font-size: var(--fs-xs);
  }
</style>
