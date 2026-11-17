<script lang="ts">
  import type { ResultPoint } from "../types";
  import { date, int, signed } from "./format";

  let { points, height = 300 }: { points: ResultPoint[]; height?: number } = $props();

  const M = { top: 24, right: 16, bottom: 28, left: 56 };
  const SERIES = [
    { key: "net_bb", label: "Gesamt", color: "var(--text)", width: 2 },
    { key: "showdown_bb", label: "Mit Showdown", color: "var(--sd)", width: 1.5 },
    { key: "non_showdown_bb", label: "Ohne Showdown", color: "var(--neg)", width: 1.5 },
  ] as const;

  let width = $state(0);
  let hover = $state<number | null>(null);

  /** Cumulative curves start at zero before the first hand. */
  const data = $derived<ResultPoint[]>(
    points.length === 0
      ? []
      : [{ n: 0, played_at: points[0].played_at, net_bb: 0, showdown_bb: 0, non_showdown_bb: 0 }, ...points],
  );

  function niceStep(span: number, count: number): number {
    const raw = span / count;
    const mag = 10 ** Math.floor(Math.log10(raw));
    const f = raw / mag;
    return (f <= 1 ? 1 : f <= 2 ? 2 : f <= 2.5 ? 2.5 : f <= 5 ? 5 : 10) * mag;
  }

  const chart = $derived.by(() => {
    if (data.length === 0 || width === 0) return null;
    const w = width - M.left - M.right;
    const h = height - M.top - M.bottom;
    let lo = 0;
    let hi = 0;
    for (const p of data) {
      lo = Math.min(lo, p.net_bb, p.showdown_bb, p.non_showdown_bb);
      hi = Math.max(hi, p.net_bb, p.showdown_bb, p.non_showdown_bb);
    }
    if (hi - lo < 1) {
      hi += 1;
      lo -= 1;
    }
    const yStep = niceStep(hi - lo, Math.max(3, Math.round(h / 56)));
    lo = Math.floor(lo / yStep) * yStep;
    hi = Math.ceil(hi / yStep) * yStep;
    const maxN = Math.max(data[data.length - 1].n, 1);
    const xStep = niceStep(maxN, Math.max(2, Math.round(w / 110)));

    const x = (n: number) => M.left + (n / maxN) * w;
    const y = (v: number) => M.top + h - ((v - lo) / (hi - lo)) * h;

    const yTicks: number[] = [];
    for (let v = lo; v <= hi + yStep / 2; v += yStep) yTicks.push(v);
    const xTicks: number[] = [];
    for (let n = 0; n <= maxN; n += xStep) xTicks.push(n);

    const lines = SERIES.map((s) => ({
      ...s,
      d: data.map((p, i) => `${i ? "L" : "M"}${x(p.n).toFixed(1)},${y(p[s.key]).toFixed(1)}`).join(""),
    }));
    return { x, y, w, h, yTicks, xTicks, lines, yStep };
  });

  function tick(v: number, step: number): string {
    const digits = step < 1 ? 1 : 0;
    return v === 0 ? "0" : signed(v, digits);
  }

  function move(e: PointerEvent) {
    if (!chart || data.length === 0) return;
    const rect = (e.currentTarget as SVGElement).getBoundingClientRect();
    const px = e.clientX - rect.left;
    if (px < M.left - 8 || px > M.left + chart.w + 8) {
      hover = null;
      return;
    }
    const maxN = data[data.length - 1].n || 1;
    const n = ((px - M.left) / chart.w) * maxN;
    let a = 0;
    let b = data.length - 1;
    while (b - a > 1) {
      const mid = (a + b) >> 1;
      if (data[mid].n < n) a = mid;
      else b = mid;
    }
    hover = Math.abs(data[a].n - n) <= Math.abs(data[b].n - n) ? a : b;
  }

  const hovered = $derived(hover !== null && chart ? data[hover] : null);
</script>

<div class="chart" bind:clientWidth={width} style="height:{height}px">
  {#if data.length === 0}
    <div class="none">Noch keine Hände in diesem Zeitraum. Spiel ein paar Hände oder wähle oben einen längeren Zeitraum.</div>
  {:else if chart}
    <svg
      {width}
      {height}
      role="img"
      aria-label="Ergebnis-Verlauf in Big Blinds"
      onpointermove={move}
      onpointerleave={() => (hover = null)}
    >
      {#each chart.yTicks as v (v)}
        <line class="grid" class:zero={v === 0} x1={M.left} x2={M.left + chart.w} y1={chart.y(v)} y2={chart.y(v)} />
        <text class="ylab" x={M.left - 10} y={chart.y(v)} dominant-baseline="central" text-anchor="end">{tick(v, chart.yStep)}</text>
      {/each}
      {#each chart.xTicks as n (n)}
        <text
          class="xlab"
          x={chart.x(n)}
          y={M.top + chart.h + 18}
          text-anchor={n === 0 ? "start" : "middle"}>{int(n)}</text
        >
      {/each}
      <text class="axis" x={M.left - 10} y={M.top + chart.h + 18} text-anchor="end">Hände</text>
      <text class="axis" x={M.left - 10} y={M.top - 14} dominant-baseline="central" text-anchor="end">BB</text>

      {#each [...chart.lines].reverse() as l (l.key)}
        <path d={l.d} stroke={l.color} stroke-width={l.width} fill="none" stroke-linejoin="round" />
      {/each}

      {#if data.length === 2}
        {#each chart.lines as l (l.key)}
          <circle cx={chart.x(data[1].n)} cy={chart.y(data[1][l.key])} r="3" fill={l.color} />
        {/each}
      {/if}

      {#if hovered}
        <line class="cross" x1={chart.x(hovered.n)} x2={chart.x(hovered.n)} y1={M.top} y2={M.top + chart.h} />
        {#each chart.lines as l (l.key)}
          <circle class="dot" cx={chart.x(hovered.n)} cy={chart.y(hovered[l.key])} r="3.5" fill={l.color} />
        {/each}
      {/if}
    </svg>

    {#if hovered}
      {@const left = chart.x(hovered.n)}
      <div class="tip" style="top:{M.top + 6}px; {left > width - 200 ? `right:${width - left + 12}px` : `left:${left + 12}px`}">
        <header>Hand <span class="num">{int(hovered.n)}</span> · {date(hovered.played_at)}</header>
        {#each SERIES as s (s.key)}
          <div class="row">
            <span><i style="background:{s.color}"></i>{s.label}</span>
            <b class:pos={hovered[s.key] > 0} class:neg={hovered[s.key] < 0}>{signed(hovered[s.key])} BB</b>
          </div>
        {/each}
      </div>
    {/if}
  {/if}
</div>

<style>
  .chart {
    position: relative;
    width: 100%;
  }
  svg {
    display: block;
    touch-action: none;
  }
  .grid {
    stroke: var(--line);
    shape-rendering: crispEdges;
  }
  .grid.zero {
    stroke: var(--line-2);
  }
  text {
    fill: var(--text-2);
    font-family: var(--font-num);
    font-size: 11.5px;
    font-variant-numeric: tabular-nums;
  }
  .axis {
    fill: var(--text-2);
    font-family: var(--font-ui);
    font-size: 11px;
    font-weight: 600;
  }
  .cross {
    stroke: var(--text-3);
    stroke-dasharray: 3 3;
    shape-rendering: crispEdges;
  }
  .dot {
    stroke: var(--surface);
    stroke-width: 2;
  }
  .tip i {
    display: inline-block;
    width: 10px;
    height: 2px;
    margin-right: 7px;
    vertical-align: 3px;
  }
  .none {
    display: grid;
    place-items: center;
    height: 100%;
    padding: 0 24px;
    color: var(--text-2);
    text-align: center;
  }
</style>
