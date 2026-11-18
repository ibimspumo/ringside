<script lang="ts">
  import { int } from "../ui/format";
  import type { StackPoint } from "./types";

  /** The hero's stack over the hands of a tournament; the costliest mistakes are numbered on the line, like in their list. */
  let { points }: { points: StackPoint[] } = $props();

  const W = 1000;
  const H = 220;
  const PAD = { l: 52, r: 14, t: 14, b: 26 };

  /** Chips at the start of each hand, and after the last one. */
  const line = $derived.by(() => {
    const known = points.filter((p) => p.stack !== null);
    const out = known.map((p) => ({ n: p.n, chips: p.stack! / 100, p }));
    const last = known.at(-1);
    if (last) out.push({ n: last.n + 1, chips: Math.max(0, (last.stack! + last.net) / 100), p: last });
    return out;
  });
  const maxChips = $derived(Math.max(1, ...line.map((d) => d.chips)));
  const maxN = $derived(Math.max(2, ...line.map((d) => d.n)));
  const x = (n: number) => PAD.l + ((n - 1) / (maxN - 1)) * (W - PAD.l - PAD.r);
  const y = (c: number) => PAD.t + (1 - c / maxChips) * (H - PAD.t - PAD.b);
  const path = $derived(line.map((d, i) => `${i ? "L" : "M"}${x(d.n).toFixed(1)},${y(d.chips).toFixed(1)}`).join(""));
  const area = $derived(line.length ? `${path}L${x(line.at(-1)!.n).toFixed(1)},${y(0)}L${x(line[0].n).toFixed(1)},${y(0)}Z` : "");
  const marks = $derived(line.filter((d) => d.p.mistake !== null && d.n === d.p.n));
  const ticks = $derived([0, 0.5, 1].map((f) => Math.round((maxChips * f) / 100) * 100));
  const start = $derived(line[0]?.chips ?? 0);
  const peak = $derived(line.reduce((a, d) => (d.chips > a.chips ? d : a), line[0] ?? { n: 1, chips: 0 }));
  const end = $derived(line.at(-1)?.chips ?? 0);
</script>

{#if line.length > 1}
  <figure class="stack">
    <svg viewBox="0 0 {W} {H}" preserveAspectRatio="none" role="img" aria-label="Dein Stack über {int(points.length)} Hände: Start {int(start)} Chips, höchstens {int(peak.chips)}, am Ende {int(end)}.">
      {#each ticks as t (t)}
        <line class="grid" x1={PAD.l} x2={W - PAD.r} y1={y(t)} y2={y(t)} />
      {/each}
      <path class="area" d={area} />
      <path class="line" d={path} vector-effect="non-scaling-stroke" />
    </svg>
    <div class="labels" aria-hidden="true">
      {#each ticks as t (t)}<span class="tick num" style:top="{(y(t) / H) * 100}%">{int(t)}</span>{/each}
      <span class="axis">Hand 1</span><span class="axis end">Hand {int(maxN - 1)}</span>
    </div>
    {#each marks as m (m.n)}
      <span
        class="mark num"
        style:left="{(x(m.n) / W) * 100}%"
        style:top="{(y(m.chips) / H) * 100}%"
        title="Teurer Fehler {m.p.mistake} in Hand {m.n}: −{m.p.loss_bb.toLocaleString('de-DE', { maximumFractionDigits: 1 })} BB"
        >{m.p.mistake}</span
      >
    {/each}
  </figure>
{/if}

<style>
  .stack {
    position: relative;
    margin: 0;
    height: 220px;
  }
  svg {
    display: block;
    width: 100%;
    height: 100%;
    overflow: visible;
  }
  .grid {
    stroke: var(--line);
    stroke-width: 1;
    vector-effect: non-scaling-stroke;
  }
  .area {
    fill: color-mix(in srgb, var(--text) 7%, transparent);
  }
  .line {
    fill: none;
    stroke: var(--text);
    stroke-width: 1.8;
    stroke-linejoin: round;
  }
  .labels {
    position: absolute;
    inset: 0;
    pointer-events: none;
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .tick {
    position: absolute;
    left: 0;
    width: 46px;
    text-align: right;
    transform: translateY(-50%);
  }
  .axis {
    position: absolute;
    bottom: 0;
    left: 52px;
  }
  .axis.end {
    left: auto;
    right: 14px;
  }
  .mark {
    position: absolute;
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    border: 2px solid var(--bg);
    border-radius: 50%;
    background: var(--grade-blunder);
    color: var(--text-on-light);
    font-size: var(--fs-xs);
    font-weight: var(--fw-bold);
    transform: translate(-50%, -50%);
    cursor: default;
  }
</style>
