<script lang="ts">
  import { untrack } from "svelte";
  import { STAT_META } from "../stats";
  import type { StatId, Threshold } from "../types";

  interface Props {
    id: StatId;
    t: Threshold;
    colors: { low: string; mid: string; high: string };
    /** Values of the players at the preview table, drawn as ticks. */
    marks: number[];
    /** Show number fields for exact values. */
    exact: boolean;
    /** What a low and a high value says about a player. */
    meaning?: [string, string];
    onchange: () => void;
  }

  let { id, t, colors, marks, exact, meaning, onchange }: Props = $props();

  const ratio = $derived(!!STAT_META[id].ratio);
  const step = $derived(ratio ? 0.1 : 1);
  /** Room for the interesting part of the scale; fixed while dragging so the track doesn't move. */
  const base = untrack(() => (STAT_META[id].ratio ? 6 : Math.min(100, Math.max(30, Math.ceil((t.high * 2.2) / 10) * 10))));
  const max = $derived(Math.max(base, t.high * 1.1));
  const pct = (v: number) => `${(Math.min(Math.max(v, 0), max) / max) * 100}%`;
  const show = (v: number) => (ratio ? v.toLocaleString("de-DE", { maximumFractionDigits: 1 }) : Math.round(v).toString());

  function setLow(v: number) {
    t.low = Math.min(v, t.high);
    onchange();
  }
  function setHigh(v: number) {
    t.high = Math.max(v, t.low);
    onchange();
  }
</script>

<div class="item" title={STAT_META[id].help}>
  <div class="top">
    <span class="name">{STAT_META[id].name}</span>
    {#if STAT_META[id].short !== STAT_META[id].name}<span class="abbr num">{STAT_META[id].short}</span>{/if}
    {#if exact}
      <span class="fields">
        <input class="field num" type="number" step={ratio ? 0.1 : 0.5} bind:value={t.low} onchange={onchange} aria-label="{STAT_META[id].name} unter" />
        <input class="field num" type="number" step={ratio ? 0.1 : 0.5} bind:value={t.high} onchange={onchange} aria-label="{STAT_META[id].name} über" />
      </span>
    {/if}
  </div>
  <div class="scale" style="--lo:{pct(t.low)}; --hi:{pct(t.high)}; --c-low:{colors.low}; --c-mid:{colors.mid}; --c-high:{colors.high}">
    <div class="track">
      <i class="z low"></i><i class="z mid"></i><i class="z high"></i>
      {#each marks as m, i (i)}<b class="mark" style="left:{pct(m)}"></b>{/each}
    </div>
    <input
      type="range"
      min="0"
      {max}
      {step}
      value={t.low}
      aria-label="{STAT_META[id].name}: Farbe wechselt unter"
      oninput={(e) => setLow(Number(e.currentTarget.value))}
    />
    <input
      type="range"
      min="0"
      {max}
      {step}
      value={t.high}
      aria-label="{STAT_META[id].name}: Farbe wechselt über"
      oninput={(e) => setHigh(Number(e.currentTarget.value))}
    />
    <span class="v num" style="left:{pct(t.low)}">{show(t.low)}</span>
    <span class="v num" style="left:{pct(t.high)}">{show(t.high)}</span>
  </div>
  {#if meaning}
    <div class="means"><span style="color:{colors.low}">{meaning[0]}</span><span style="color:{colors.high}">{meaning[1]}</span></div>
  {/if}
</div>

<style>
  .item {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .top {
    display: flex;
    align-items: baseline;
    gap: 7px;
  }
  .name {
    font-size: 13px;
    font-weight: 500;
    color: var(--text);
  }
  .abbr {
    font-size: 11px;
    color: var(--text-3);
  }
  .fields {
    display: flex;
    gap: 6px;
    margin-left: auto;
  }
  .means {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    font-size: 11.5px;
    line-height: 1.3;
  }
  .scale {
    position: relative;
    height: 30px;
  }
  .track {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 6px;
    height: 6px;
    border-radius: 3px;
    overflow: hidden;
  }
  .z {
    position: absolute;
    top: 0;
    bottom: 0;
    opacity: 0.85;
  }
  .z.low {
    left: 0;
    width: var(--lo);
    background: var(--c-low);
  }
  .z.mid {
    left: var(--lo);
    width: calc(var(--hi) - var(--lo));
    background: var(--c-mid);
  }
  .z.high {
    left: var(--hi);
    right: 0;
    background: var(--c-high);
  }
  .mark {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 2px;
    margin-left: -1px;
    background: rgba(14, 15, 18, 0.75);
  }
  .v {
    position: absolute;
    top: 0;
    transform: translateX(-50%);
    font-size: 11.5px;
    line-height: 1;
    color: var(--text);
    pointer-events: none;
  }
  input[type="range"] {
    position: absolute;
    left: -7px;
    right: -7px;
    width: calc(100% + 14px);
    bottom: 0;
    height: 18px;
    margin: 0;
    background: transparent;
    pointer-events: none;
    -webkit-appearance: none;
    appearance: none;
  }
  input[type="range"]::-webkit-slider-runnable-track {
    background: transparent;
    height: 18px;
  }
  input[type="range"]::-webkit-slider-thumb {
    -webkit-appearance: none;
    pointer-events: auto;
    width: 14px;
    height: 14px;
    margin-top: 2px;
    border-radius: 50%;
    background: var(--text);
    border: 2px solid var(--surface);
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.5);
    cursor: ew-resize;
  }
  input[type="range"]::-moz-range-thumb {
    pointer-events: auto;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: var(--text);
    border: 2px solid var(--surface);
    cursor: ew-resize;
  }
  input[type="range"]:focus-visible {
    outline: none;
  }
  input[type="range"]:focus-visible::-webkit-slider-thumb {
    outline: 2px solid var(--focus);
    outline-offset: 2px;
  }
  .field {
    height: 26px;
    width: 58px;
    padding: 0 6px;
    text-align: right;
  }
</style>
