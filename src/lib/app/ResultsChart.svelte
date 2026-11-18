<script lang="ts" module>
  export type ChartPoint = { n: number; played_at: string };
  export type Series<P> = { key: keyof P & string; label: string; color: string; width: number; dash?: string };
</script>

<script lang="ts" generics="P extends ChartPoint">
  import { date, floating, int, signed } from "./format";

  /** Cumulative series over `n` (hands, tournaments, …); each curve starts at zero. */
  let {
    points,
    series,
    height = 300,
    unit = "BB",
    xLabel = "Hände",
    pointLabel = "Hand",
    format = (v: number) => `${signed(v)} BB`,
    tick = (v: number, step: number) => (v === 0 ? "0" : signed(v, step < 1 ? 1 : 0)),
    empty = "Noch keine Hände in diesem Zeitraum. Spiel ein paar Hände oder wähle oben einen längeren Zeitraum.",
  }: {
    points: P[];
    series: Series<P>[];
    height?: number;
    unit?: string;
    xLabel?: string;
    pointLabel?: string;
    format?: (v: number) => string;
    tick?: (v: number, step: number) => string;
    empty?: string;
  } = $props();

  const M = { top: 24, right: 16, bottom: 28, left: 56 };
  /** Rough width of one tick character in the number face at 11.5 px. */
  const CHAR = 6.6;
  const val = (p: P, key: keyof P) => p[key] as number;

  let width = $state(0);
  let hover = $state<number | null>(null);
  /** Viewport position of the plot, taken on each move so the hover card can sit beside the crosshair. */
  let origin = $state({ left: 0, top: 0 });

  const data = $derived<P[]>(
    points.length === 0
      ? []
      : [{ ...points[0], n: 0, ...Object.fromEntries(series.map((s) => [s.key, 0])) } as P, ...points],
  );

  function niceStep(span: number, count: number): number {
    const raw = span / count;
    const mag = 10 ** Math.floor(Math.log10(raw));
    const f = raw / mag;
    return (f <= 1 ? 1 : f <= 2 ? 2 : f <= 2.5 ? 2.5 : f <= 5 ? 5 : 10) * mag;
  }

  const chart = $derived.by(() => {
    if (data.length === 0 || width === 0) return null;
    const h = height - M.top - M.bottom;
    let lo = 0;
    let hi = 0;
    for (const p of data) {
      for (const s of series) {
        lo = Math.min(lo, val(p, s.key));
        hi = Math.max(hi, val(p, s.key));
      }
    }
    if (hi - lo < 1) {
      hi += 1;
      lo -= 1;
    }
    const yStep = niceStep(hi - lo, Math.max(3, Math.round(h / 56)));
    lo = Math.floor(lo / yStep) * yStep;
    hi = Math.ceil(hi / yStep) * yStep;
    const longest = Math.max(tick(lo, yStep).length, tick(hi, yStep).length, xLabel.length);
    // The left margin grows with the longest tick label, so seven-digit values stay readable.
    const left = Math.max(M.left, Math.ceil(longest * CHAR) + 14);
    const w = width - left - M.right;
    const maxN = Math.max(data[data.length - 1].n, 1);
    // Hands and tournaments are counted in whole numbers.
    const xStep = Math.max(1, Math.ceil(niceStep(maxN, Math.max(2, Math.round(w / 110)))));

    const x = (n: number) => left + (n / maxN) * w;
    const y = (v: number) => M.top + h - ((v - lo) / (hi - lo)) * h;

    const yTicks: number[] = [];
    for (let v = lo; v <= hi + yStep / 2; v += yStep) yTicks.push(v);
    const xTicks: number[] = [];
    for (let n = 0; n <= maxN; n += xStep) xTicks.push(n);

    const lines = series.map((s) => ({
      ...s,
      d: data.map((p, i) => `${i ? "L" : "M"}${x(p.n).toFixed(1)},${y(val(p, s.key)).toFixed(1)}`).join(""),
    }));
    return { x, y, w, h, left, yTicks, xTicks, lines, yStep };
  });

  function move(e: PointerEvent) {
    if (!chart || data.length === 0) return;
    const rect = (e.currentTarget as SVGElement).getBoundingClientRect();
    origin = { left: rect.left, top: rect.top };
    const px = e.clientX - rect.left;
    if (px < chart.left - 8 || px > chart.left + chart.w + 8) {
      hover = null;
      return;
    }
    const maxN = data[data.length - 1].n || 1;
    const n = ((px - chart.left) / chart.w) * maxN;
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
    <div class="none">{empty}</div>
  {:else if chart}
    <svg
      {width}
      {height}
      role="img"
      aria-label="Ergebnis-Verlauf in {unit}"
      onpointermove={move}
      onpointerleave={() => (hover = null)}
    >
      {#each chart.yTicks as v (v)}
        <line class="grid" class:zero={v === 0} x1={chart.left} x2={chart.left + chart.w} y1={chart.y(v)} y2={chart.y(v)} />
        <text class="ylab" x={chart.left - 10} y={chart.y(v)} dominant-baseline="central" text-anchor="end">{tick(v, chart.yStep)}</text>
      {/each}
      {#each chart.xTicks as n (n)}
        <text
          class="xlab"
          x={chart.x(n)}
          y={M.top + chart.h + 18}
          text-anchor={n === 0 ? "start" : "middle"}>{int(n)}</text
        >
      {/each}
      <text class="axis" x={chart.left - 10} y={M.top + chart.h + 18} text-anchor="end">{xLabel}</text>
      <text class="axis" x={chart.left - 10} y={M.top - 14} dominant-baseline="central" text-anchor="end">{unit}</text>

      {#each [...chart.lines].reverse() as l (l.key)}
        <path d={l.d} stroke={l.color} stroke-width={l.width} stroke-dasharray={l.dash} fill="none" stroke-linejoin="round" />
      {/each}

      {#if data.length === 2}
        {#each chart.lines as l (l.key)}
          <circle cx={chart.x(data[1].n)} cy={chart.y(val(data[1], l.key))} r="3" fill={l.color} />
        {/each}
      {/if}

      {#if hovered}
        <line class="cross" x1={chart.x(hovered.n)} x2={chart.x(hovered.n)} y1={M.top} y2={M.top + chart.h} />
        {#each chart.lines as l (l.key)}
          <circle class="dot" cx={chart.x(hovered.n)} cy={chart.y(val(hovered, l.key))} r="3.5" fill={l.color} />
        {/each}
      {/if}
    </svg>

    {#if hovered}
      {@const x = origin.left + chart.x(hovered.n)}
      {@const y = origin.top + M.top + 6}
      <div class="tip" use:floating={{ anchor: { left: x - 6, right: x + 6, top: y, bottom: y }, side: "right" }}>
        <header>{pointLabel} <span class="num">{int(hovered.n)}</span> · {date(hovered.played_at)}</header>
        {#each series as s (s.key)}
          {@const v = val(hovered, s.key)}
          <div class="row">
            <span><i style="background:{s.color}"></i>{s.label}</span>
            <b class:pos={v > 0} class:neg={v < 0}>{format(v)}</b>
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
