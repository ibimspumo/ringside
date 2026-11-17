<script lang="ts" module>
  export type GridMode = "strategy" | "range" | "ev" | "equity";
</script>

<script lang="ts">
  import { CLASSES, classCombos, classLabel } from "./range";
  import type { SolverClass, SolverNode } from "./types";

  /**
   * 13×13 view of the acting player. Strategy: each cell splits into the actions' colours by
   * frequency and fills from the bottom by how many of its combos reach the node. Range, EV and
   * equity shade the cells instead. Hover and click tell the parent which hand to detail.
   */
  let {
    node,
    labels,
    colors,
    mode = "strategy",
    hero = null,
    selected = -1,
    onHover,
    onSelect,
  }: {
    node: SolverNode;
    labels: string[];
    colors: string[];
    mode?: GridMode;
    hero?: string[] | null;
    selected?: number;
    onHover?: (i: number | null) => void;
    onSelect?: (i: number) => void;
  } = $props();

  const byClass = $derived(new Map(node.classes.map((c) => [c.class, c])));
  const heroClass = $derived.by(() => {
    if (!hero || hero.length !== 2) return -1;
    const combo = node.hands.find((h) => {
      const a = h.combo.slice(0, 2);
      const b = h.combo.slice(2, 4);
      return (a === hero[0] && b === hero[1]) || (a === hero[1] && b === hero[0]);
    });
    return combo ? combo.class : -1;
  });
  const maxEv = $derived(Math.max(0.01, ...node.classes.map((c) => Math.abs(c.ev))));

  function bands(c: SolverClass): string {
    let at = 0;
    const stops: string[] = [];
    c.strategy.forEach((s, k) => {
      const from = at * 100;
      at += s;
      stops.push(`${colors[k]} ${from.toFixed(2)}% ${(at * 100).toFixed(2)}%`);
    });
    return `linear-gradient(to right, ${stops.join(", ")})`;
  }

  function shade(c: SolverClass): string {
    if (mode === "equity") return `color-mix(in srgb, var(--sd) ${Math.round(c.equity * 100)}%, var(--surface-2))`;
    const t = c.ev / maxEv;
    return t >= 0
      ? `color-mix(in srgb, var(--pos) ${Math.round(t * 85)}%, var(--surface-2))`
      : `color-mix(in srgb, var(--neg) ${Math.round(-t * 85)}%, var(--surface-2))`;
  }

  // One tab stop for the whole grid; arrow keys move between the cells.
  let cursor = $state(0);
  function nav(e: KeyboardEvent, i: number) {
    const step: Record<string, number> = { ArrowLeft: -1, ArrowRight: 1, ArrowUp: -13, ArrowDown: 13 };
    const d = step[e.key];
    if (d === undefined) return;
    e.preventDefault();
    e.stopPropagation();
    const next = i + d;
    if (next < 0 || next >= CLASSES || (Math.abs(d) === 1 && Math.floor(next / 13) !== Math.floor(i / 13))) return;
    cursor = next;
    ((e.currentTarget as HTMLElement).parentElement?.children[next] as HTMLElement | undefined)?.focus();
  }

  const reach = (c: SolverClass, i: number) => Math.min(1, c.combos / classCombos(i));
  const pct = (x: number) => `${Math.round(x * 100)} %`;
  const value = (c: SolverClass) =>
    mode === "equity" ? String(Math.round(c.equity * 100)) : mode === "ev" ? c.ev.toLocaleString("de-DE", { maximumFractionDigits: 1 }) : "";
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="matrix" onmouseleave={() => onHover?.(null)}>
  {#each Array.from({ length: CLASSES }, (_, i) => i) as i (i)}
    {@const c = byClass.get(i)}
    <button
      class="cell"
      class:in={!!c}
      class:hero={i === heroClass}
      class:sel={i === selected}
      onmouseenter={() => onHover?.(c ? i : null)}
      onfocus={() => ((cursor = i), onHover?.(c ? i : null))}
      onkeydown={(e) => nav(e, i)}
      onclick={() => c && onSelect?.(i)}
      tabindex={i === cursor ? 0 : -1}
      aria-label={c ? `${c.label}: ${c.strategy.map((s, k) => `${labels[k]} ${pct(s)}`).join(", ")}` : `${classLabel(i)}: nicht in der Range`}
    >
      {#if c}
        {#if mode === "strategy"}
          <i class="bar" style="height:{Math.max(8, reach(c, i) * 100)}%; background:{bands(c)}"></i>
        {:else if mode === "range"}
          <i class="bar range" style="height:{Math.max(6, reach(c, i) * 100)}%"></i>
        {:else}
          <i class="bar" style="height:100%; background:{shade(c)}"></i>
        {/if}
      {/if}
      <span class="k">{classLabel(i)}</span>
      {#if c && value(c)}<span class="v num">{value(c)}</span>{/if}
    </button>
  {/each}
</div>

<style>
  .matrix {
    container-type: inline-size;
    position: relative;
    display: grid;
    grid-template-columns: repeat(13, minmax(0, 1fr));
    gap: 2px;
  }
  .cell {
    position: relative;
    aspect-ratio: 1;
    min-width: 0;
    padding: 0;
    border: 0;
    border-radius: 3px;
    overflow: hidden;
    background: var(--surface-2);
    color: var(--text-3);
    text-align: left;
    cursor: default;
  }
  .cell.in {
    background: color-mix(in srgb, var(--surface-3) 70%, var(--surface-2));
    color: #f4f3ef;
    cursor: pointer;
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
  .cell:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: -1px;
    z-index: 3;
  }
  .cell.in:hover {
    outline: 1px solid var(--text);
    z-index: 2;
  }
  .bar {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
  }
  .bar.range {
    background: color-mix(in srgb, var(--amber) 80%, var(--surface-2));
  }
  .k {
    position: relative;
    display: block;
    padding: 3px 4px;
    font-family: var(--font-num);
    font-size: 11px;
    font-weight: 600;
    line-height: 1;
    text-shadow:
      0 0 2px rgba(0, 0, 0, 0.9),
      0 1px 3px rgba(0, 0, 0, 0.6);
  }
  .cell:not(.in) .k {
    text-shadow: none;
  }
  .v {
    position: absolute;
    right: 3px;
    bottom: 2px;
    font-size: 9.5px;
    color: rgba(255, 255, 255, 0.85);
    text-shadow: 0 0 2px rgba(0, 0, 0, 0.9);
  }
  @container (max-width: 420px) {
    .k {
      padding: 2px;
      font-size: 9px;
    }
    .v {
      display: none;
    }
  }
</style>
