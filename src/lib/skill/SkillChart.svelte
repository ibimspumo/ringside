<script lang="ts">
  import { date } from "../app/format";
  import { floating } from "../ui/floating";
  import { delta, points, spread } from "./labels";
  import type { SkillEntry } from "./types";

  /**
   * The rating after every counted tournament: the line, its uncertainty as a band around it and
   * the levels as quiet lines behind. Hover names the tournament, a click opens it.
   */
  let {
    entries,
    tiers,
    start,
    names,
    height = 260,
    onPick,
  }: {
    entries: SkillEntry[];
    tiers: [number, string][];
    start: number;
    /** Tournament names by id. */
    names: Map<number, string>;
    height?: number;
    onPick?: (e: SkillEntry) => void;
  } = $props();

  const M = { top: 16, right: 112, bottom: 28, left: 48 };
  let width = $state(0);
  let hover = $state<number | null>(null);
  let origin = $state({ left: 0, top: 0 });

  /** The start as point 0, then every counted tournament. */
  const data = $derived([
    { n: 0, rating: start, sigma: 300, entry: null as SkillEntry | null },
    ...entries.filter((e) => e.performance !== null).map((e, i) => ({ n: i + 1, rating: e.rating, sigma: e.sigma, entry: e as SkillEntry | null })),
  ]);

  const chart = $derived.by(() => {
    if (width === 0 || data.length < 2) return null;
    const h = height - M.top - M.bottom;
    const w = width - M.left - M.right;
    // The scale holds the whole band, so the funnel of the first tournaments shows.
    let lo = Math.min(...data.map((d) => d.rating - d.sigma));
    let hi = Math.max(...data.map((d) => d.rating + d.sigma));
    lo = Math.floor((lo - 40) / 100) * 100;
    hi = Math.ceil((hi + 40) / 100) * 100;
    const maxN = data[data.length - 1].n;
    const x = (n: number) => M.left + (n / maxN) * w;
    const y = (v: number) => M.top + h - ((Math.min(hi, Math.max(lo, v)) - lo) / (hi - lo)) * h;
    const step = hi - lo > 500 ? 200 : 100;
    const ticks: number[] = [];
    for (let v = lo; v <= hi; v += step) ticks.push(v);
    const line = data.map((d, i) => `${i ? "L" : "M"}${x(d.n).toFixed(1)},${y(d.rating).toFixed(1)}`).join("");
    const upper = data.map((d) => `${x(d.n).toFixed(1)},${y(d.rating + d.sigma).toFixed(1)}`);
    const lower = data.map((d) => `${x(d.n).toFixed(1)},${y(d.rating - d.sigma).toFixed(1)}`).reverse();
    const band = `M${upper.join("L")}L${lower.join("L")}Z`;
    const levels = tiers.filter(([from]) => from > lo && from < hi).map(([from, name]) => ({ from, name, y: y(from) }));
    const xStep = Math.max(1, Math.ceil(maxN / Math.max(2, Math.floor(w / 90))));
    const xTicks: number[] = [];
    for (let n = 0; n <= maxN; n += xStep) xTicks.push(n);
    return { x, y, h, w, ticks, line, band, levels, xTicks };
  });

  function move(e: PointerEvent) {
    if (!chart) return;
    const rect = (e.currentTarget as SVGElement).getBoundingClientRect();
    origin = { left: rect.left, top: rect.top };
    const n = Math.round(((e.clientX - rect.left - M.left) / chart.w) * data[data.length - 1].n);
    hover = n >= 1 && n < data.length ? n : null;
  }

  const hovered = $derived(hover !== null ? data[hover] : null);
  const label = (e: SkillEntry) => (e.tournament_id !== null ? (names.get(e.tournament_id) ?? "Turnier") : "Cash");
  function key(e: KeyboardEvent) {
    if (!data.length) return;
    if (e.key === "ArrowRight" || e.key === "ArrowLeft") {
      e.preventDefault();
      const last = data.length - 1;
      hover = Math.min(last, Math.max(1, (hover ?? last) + (e.key === "ArrowRight" ? 1 : -1)));
    } else if (e.key === "Enter" && hovered?.entry) onPick?.(hovered.entry);
  }
</script>

<div class="sk-chart" bind:clientWidth={width} style:height="{height}px">
  {#if chart}
    <svg
      {width}
      {height}
      role="slider"
      aria-valuemin={1}
      aria-valuemax={data.length - 1}
      aria-valuenow={hover ?? data.length - 1}
      tabindex="0"
      aria-label="Verlauf deiner Wertung über {data.length - 1} Turniere"
      onpointermove={move}
      onpointerleave={() => (hover = null)}
      onblur={() => (hover = null)}
      onkeydown={key}
      onclick={() => hovered?.entry && onPick?.(hovered.entry)}
      class:pick={!!hovered && !!onPick}
    >
      {#each chart.ticks as v (v)}
        <line class="grid" x1={M.left} x2={M.left + chart.w} y1={chart.y(v)} y2={chart.y(v)} />
        <text class="ylab" x={M.left - 10} y={chart.y(v)} dominant-baseline="central" text-anchor="end">{v}</text>
      {/each}
      {#each chart.levels as l (l.from)}
        <line class="level" x1={M.left} x2={M.left + chart.w} y1={l.y} y2={l.y} />
        <text class="lvl" x={M.left + chart.w + 10} y={l.y} dominant-baseline="central">{l.name}</text>
      {/each}
      {#each chart.xTicks as n (n)}
        <text class="xlab" x={chart.x(n)} y={M.top + chart.h + 18} text-anchor={n === 0 ? "start" : "middle"}>{n === 0 ? "Start" : n}</text>
      {/each}
      <path class="band" d={chart.band} />
      <path class="line" d={chart.line} />
      {#each data as d (d.n)}
        {#if d.n > 0}
          <circle class="pt" class:up={(d.entry?.delta ?? 0) > 0} class:down={(d.entry?.delta ?? 0) < 0} cx={chart.x(d.n)} cy={chart.y(d.rating)} r={d.n === data.length - 1 ? 4.5 : 2.5} />
        {/if}
      {/each}
      {#if hovered}
        <line class="cross" x1={chart.x(hovered.n)} x2={chart.x(hovered.n)} y1={M.top} y2={M.top + chart.h} />
        <circle class="dot" cx={chart.x(hovered.n)} cy={chart.y(hovered.rating)} r="5" />
      {/if}
    </svg>
    {#if hovered?.entry}
      {@const e = hovered.entry}
      {@const px = origin.left + chart.x(hovered.n)}
      {@const py = origin.top + chart.y(hovered.rating)}
      <div class="tip sk-tip" use:floating={{ anchor: { left: px - 8, right: px + 8, top: py, bottom: py }, side: "right" }}>
        <header>Turnier {hovered.n} · {date(e.started_at)}</header>
        <p class="sk-tip-name">{label(e)}</p>
        <div class="row"><span>Wertung</span><b>{points(e.rating)} <small>{spread(e.sigma)}</small></b></div>
        <div class="row"><span>Änderung</span><b class:up={e.delta > 0} class:down={e.delta < 0}>{delta(e.delta)}</b></div>
        <div class="row"><span>Entscheidungen</span><b>{e.decisions}</b></div>
        <div class="row"><span>Fehler · grobe</span><b>{e.grades.mistake} · {e.grades.blunder}</b></div>
        {#if onPick && e.tournament_id !== null}<footer>Klick öffnet den Turnier-Bericht</footer>{/if}
      </div>
    {/if}
  {/if}
</div>

<style>
  .sk-chart {
    position: relative;
    width: 100%;
  }
  svg {
    display: block;
    outline: none;
    border-radius: var(--r-sm);
  }
  svg:focus-visible {
    box-shadow: 0 0 0 2px var(--focus);
  }
  svg.pick {
    cursor: pointer;
  }
  .grid {
    stroke: var(--line);
    shape-rendering: crispEdges;
  }
  .level {
    stroke: var(--line-2);
    stroke-dasharray: 2 4;
    shape-rendering: crispEdges;
  }
  text {
    fill: var(--text-2);
    font-family: var(--font-num);
    font-size: var(--fs-sm);
    font-variant-numeric: tabular-nums;
  }
  .lvl {
    fill: var(--text-3);
    font-family: var(--font-ui);
    font-size: var(--fs-xs);
  }
  .band {
    fill: var(--sd);
    opacity: 0.12;
  }
  .line {
    fill: none;
    stroke: var(--sd);
    stroke-width: 2;
    stroke-linejoin: round;
  }
  .pt {
    fill: var(--text-3);
    stroke: var(--surface);
    stroke-width: 1.5;
  }
  .pt.up {
    fill: var(--pos);
  }
  .pt.down {
    fill: var(--neg);
  }
  .cross {
    stroke: var(--text-3);
    stroke-dasharray: 3 3;
    shape-rendering: crispEdges;
  }
  .dot {
    fill: var(--sd);
    stroke: var(--surface);
    stroke-width: 2;
  }
  .sk-tip {
    min-width: 200px;
  }
  .sk-tip-name {
    margin: 0 0 var(--sp-1);
    max-width: 260px;
    overflow: hidden;
    font-weight: var(--fw-semibold);
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sk-tip small {
    color: var(--text-2);
  }
  .sk-tip footer {
    margin-top: var(--sp-1);
    color: var(--text-3);
    font-size: var(--fs-xs);
  }
  .up {
    color: var(--pos);
  }
  .down {
    color: var(--neg);
  }
</style>
