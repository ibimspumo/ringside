<script lang="ts" module>
  export type GridMode = "strategy" | "range" | "ev" | "equity";
</script>

<script lang="ts">
  import RangeGrid, { type RangeCell } from "../ui/RangeGrid.svelte";
  import { pct } from "../ui/format";
  import HandDetail from "./HandDetail.svelte";
  import { CLASSES, classCombos } from "./range";
  import type { SolverClass, SolverNode } from "./types";

  /**
   * 13×13 view of the acting player on the shared RangeGrid. Strategy: each cell splits into the
   * actions' colours by frequency and fills from the bottom by how many of its combos reach the
   * node. Range, EV and equity shade the cells instead. Hover shows a short card, a click pins
   * the full one.
   */
  let {
    node,
    labels,
    colors,
    mode = "strategy",
    hero = null,
    selected = -1,
    onSelect,
    onClose,
  }: {
    node: SolverNode;
    labels: string[];
    colors: string[];
    mode?: GridMode;
    hero?: string[] | null;
    selected?: number;
    onSelect?: (i: number) => void;
    onClose?: () => void;
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

  function shade(c: SolverClass): string {
    if (mode === "equity") return `color-mix(in srgb, var(--sd) ${Math.round(c.equity * 100)}%, var(--surface-2))`;
    const t = c.ev / maxEv;
    return t >= 0
      ? `color-mix(in srgb, var(--pos) ${Math.round(t * 85)}%, var(--surface-2))`
      : `color-mix(in srgb, var(--neg) ${Math.round(-t * 85)}%, var(--surface-2))`;
  }

  const reach = (c: SolverClass, i: number) => Math.min(1, c.combos / classCombos(i));
  const value = (c: SolverClass) =>
    mode === "equity" ? String(Math.round(c.equity * 100)) : mode === "ev" ? c.ev.toLocaleString("de-DE", { maximumFractionDigits: 1 }) : undefined;

  const cells = $derived(
    Array.from({ length: CLASSES }, (_, i): RangeCell => {
      const c = byClass.get(i);
      if (!c) return null;
      const note = c.strategy.map((s, k) => `${labels[k]} ${pct(s)}`).join(", ");
      if (mode === "strategy") return { mix: c.strategy.map((s, k) => ({ color: colors[k], share: s })), weight: reach(c, i), note };
      if (mode === "range") return { color: "var(--act-bet-1)", weight: reach(c, i), note };
      return { color: shade(c), weight: 1, value: value(c), note };
    }),
  );
</script>

<RangeGrid
  {cells}
  label="Strategie als 13 mal 13 Raster"
  hero={heroClass}
  {selected}
  minFill={mode === "range" ? 0.06 : 0.08}
  onSelect={onSelect && ((i) => byClass.has(i) && onSelect?.(i))}
  {onClose}
>
  {#snippet tip(i)}<HandDetail {node} index={i} {colors} {labels} {hero} compact />{/snippet}
  {#snippet card(i)}<HandDetail {node} index={i} {colors} {labels} {hero} onClose={onClose} />{/snippet}
</RangeGrid>
