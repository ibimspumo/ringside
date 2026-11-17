<script lang="ts">
  import { STAT_META, fmt, stat, toneColor } from "../stats";
  import type { HudConfig, Point, SeatView } from "../types";

  interface Props {
    seat: SeatView;
    hud: HudConfig;
    /** Avatar centre the popup belongs to. */
    anchor: Point;
    /** Distance from the avatar centre to either side. */
    gap: number;
    /** The overlay's size: the popup always stays fully inside it. */
    width: number;
    height: number;
  }

  let { seat, hud, anchor, gap, width, height }: Props = $props();
  const details = $derived(seat.stats.filter((s) => s.id !== "hands"));

  const MARGIN = 6;
  let w = $state(248);
  let h = $state(0);
  /** Beside the avatar, on the side with more room; vertically centred, then clamped. */
  const at = $derived.by(() => {
    const right = anchor.x + gap;
    const left = anchor.x - gap - w;
    const x = anchor.x < width / 2 ? (right + w <= width - MARGIN ? right : left) : left >= MARGIN ? left : right;
    const y = anchor.y - h / 2;
    return {
      x: Math.min(Math.max(x, MARGIN), Math.max(MARGIN, width - w - MARGIN)),
      y: Math.min(Math.max(y, MARGIN), Math.max(MARGIN, height - h - MARGIN)),
    };
  });
</script>

<div class="pop hit" style="left:{at.x}px; top:{at.y}px; max-height:{height - 2 * MARGIN}px" bind:offsetWidth={w} bind:offsetHeight={h}>
  <header>
    <strong>{seat.name}</strong>
    <span class="num">{fmt(stat(seat.stats, "hands"))} Hände</span>
  </header>
  {#each details as s (s.id)}
    <div class="line" title={STAT_META[s.id].help}>
      <span class="label">{STAT_META[s.id].name}</span>
      <b class="num" style="color:{toneColor(hud, s, 'var(--text-3)')}">{fmt(s)}{STAT_META[s.id].ratio || s.value === null ? "" : " %"}</b>
      <span class="num sample">{s.made}/{s.opp}</span>
    </div>
  {/each}
</div>

<style>
  .pop {
    position: absolute;
    z-index: 20;
    width: 248px;
    padding: 10px 12px 8px;
    border-radius: 10px;
    background: rgba(22, 24, 29, 0.98);
    box-shadow:
      0 0 0 1px rgba(255, 255, 255, 0.1) inset,
      0 18px 40px rgba(0, 0, 0, 0.55);
    pointer-events: auto;
    overflow-y: auto;
    font-size: 12.5px;
    color: var(--text);
  }
  header {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 8px;
    padding-bottom: 6px;
    margin-bottom: 4px;
    border-bottom: 1px solid var(--line-2);
  }
  header strong {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  header span {
    flex-shrink: 0;
    color: var(--text-2);
  }
  .line {
    display: grid;
    grid-template-columns: 1fr auto 54px;
    gap: 8px;
    padding: 2px 0;
  }
  .label {
    color: var(--text-2);
  }
  b {
    font-size: 14px;
    font-weight: 600;
  }
  .sample {
    text-align: right;
    color: var(--text-3);
  }
</style>
