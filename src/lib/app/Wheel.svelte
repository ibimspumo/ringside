<script lang="ts">
  import { STAT_META, fmtUnit, stat, toneColor } from "../stats";
  import type { HudConfig, StatValue } from "../types";

  let {
    stats,
    hud,
    size = 22,
    hands = null,
  }: { stats: StatValue[]; hud: HudConfig; size?: number; hands?: string | null } = $props();

  const R = 40;
  const GAP = 14;

  function arc(i: number): string {
    const a0 = ((-90 + i * 90 + GAP / 2) * Math.PI) / 180;
    const a1 = ((-90 + (i + 1) * 90 - GAP / 2) * Math.PI) / 180;
    const p = (a: number) => `${(50 + R * Math.cos(a)).toFixed(2)} ${(50 + R * Math.sin(a)).toFixed(2)}`;
    return `M${p(a0)} A${R} ${R} 0 0 1 ${p(a1)}`;
  }

  const segments = $derived(
    hud.graphic.segments.slice(0, 4).map((id, i) => {
      const s = stat(stats, id);
      return { id, d: arc(i), color: toneColor(hud, s), label: `${STAT_META[id].short} ${fmtUnit(s)}` };
    }),
  );
  const title = $derived(segments.map((s) => s.label).join(" · "));
  /** Thicker strokes at small sizes keep the arcs readable. */
  const width = $derived(size <= 24 ? 17 : size <= 48 ? 13 : 10);
</script>

<svg class="wheel" width={size} height={size} viewBox="0 0 100 100" role="img" aria-label={title}>
  <title>{title}</title>
  {#each segments as s, i (i)}
    <path d={s.d} stroke={s.color} stroke-width={width} fill="none" />
  {/each}
  {#if hands}
    <text x="50" y="50" text-anchor="middle" dominant-baseline="central" font-size={hands.length > 3 ? 26 : 32}>{hands}</text>
  {/if}
</svg>

<style>
  .wheel {
    display: block;
    flex-shrink: 0;
  }
  text {
    fill: var(--text);
    font-family: var(--font-num);
    font-weight: 600;
  }
</style>
