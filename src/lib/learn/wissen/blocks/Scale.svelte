<script lang="ts">
  import { PALETTES, STAT_META } from "../../../stats";
  import type { Palette, StatId } from "../../../types";
  import { num } from "../math";

  let {
    stat,
    max: baseMax,
    low,
    high,
    labels,
    ratio = false,
    palette = "traffic",
  }: { stat: StatId; max: number; low: number; high: number; labels: [string, string, string]; ratio?: boolean; palette?: Palette } = $props();

  const p = $derived(PALETTES[palette] ?? PALETTES.traffic);
  // Own thresholds can sit beyond the article's axis; keep the upper zone visible.
  const max = $derived(Math.max(baseMax, high * 1.25));
  const at = (v: number) => `${(v / max) * 100}%`;
  const fmt = (v: number) => (ratio ? `${num(v, 1)}×` : `${v} %`);
</script>

<div class="scale">
  <div class="plate">
    <span class="short">{STAT_META[stat].short}</span>
    {#if STAT_META[stat].name !== STAT_META[stat].short}<span class="name">{STAT_META[stat].name}</span>{/if}
  </div>
  <div class="track">
    <span class="zone" style="left:0; width:{at(low)}; --c:{p.low}"></span>
    <span class="zone" style="left:{at(low)}; width:{at(high - low)}; --c:{p.mid}"></span>
    <span class="zone" style="left:{at(high)}; right:0; --c:{p.high}"></span>
    <span class="tick num" style="left:{at(low)}">{fmt(low)}</span>
    <span class="tick num" style="left:{at(high)}">{fmt(high)}</span>
  </div>
  <div class="labels">
    <span style="width:{at(low)}; --c:{p.low}">{labels[0]}</span>
    <span style="width:{at(high - low)}; --c:{p.mid}">{labels[1]}</span>
    <span style="flex:1; --c:{p.high}">{labels[2]}</span>
  </div>
</div>

<style>
  .scale {
    display: grid;
    gap: 10px;
  }
  .plate {
    display: flex;
    align-items: baseline;
    gap: 12px;
    margin-bottom: 18px;
  }
  .short {
    font-family: var(--font-num);
    font-size: 44px;
    font-weight: 600;
    line-height: 1;
  }
  .name {
    color: var(--text-2);
  }
  .track {
    position: relative;
    height: 14px;
    border-radius: 99px;
    overflow: visible;
    margin-top: 16px;
  }
  .zone {
    position: absolute;
    top: 0;
    bottom: 0;
    background: color-mix(in srgb, var(--c) 70%, transparent);
  }
  .zone:first-child {
    border-radius: 99px 0 0 99px;
  }
  .zone:nth-child(3) {
    border-radius: 0 99px 99px 0;
  }
  .tick {
    position: absolute;
    bottom: calc(100% + 4px);
    transform: translateX(-50%);
    font-size: 13px;
    color: var(--text-2);
  }
  .tick::after {
    content: "";
    position: absolute;
    left: 50%;
    top: calc(100% + 1px);
    height: 20px;
    border-left: 1.5px solid var(--bg);
  }
  .labels {
    display: flex;
    font-size: 13px;
  }
  .labels span {
    text-align: center;
    color: color-mix(in srgb, var(--c) 75%, var(--text));
    min-width: 0;
  }
</style>
