<script lang="ts" module>
  export type Series = { seat: number; name: string; points: { stack: number; pct: number }[] };
</script>

<script lang="ts">
  import { floating, type Anchor } from "../app/format";
  import Hint from "./Hint.svelte";
  import { TERM, pct } from "./words";

  /**
   * The stack depth as the page's main axis: how often each position opens over the depth,
   * and right under it, on the same log scale, the slider that picks the depth.
   */
  let {
    id,
    stacks,
    value,
    onPick,
    series,
    seat,
    onPickSeat,
    modelFrom = null,
    custom = null,
  }: {
    id: string;
    /** Depths that can be picked, ascending. */
    stacks: number[];
    value: number;
    onPick: (bb: number) => void;
    /** Open-raise share per position and depth; null while it loads. */
    series: Series[] | null;
    seat: number;
    onPickSeat: (seat: number) => void;
    /** First depth of the preflop model; below it the push/fold equilibrium. */
    modelFrom?: number | null;
    custom?: { digits: number; example: string; range: string | null } | null;
  } = $props();

  const PAD = { l: 38, r: 54, t: 22, b: 8 };
  const TICKS = [2, 3, 5, 7, 10, 15, 20, 30, 50, 75, 100];

  let width = $state(600);
  // Lower on narrow windows, so table and grid still come into view.
  const H = $derived(width < 760 ? 108 : 168);
  let hover = $state<{ stack: number; seat: number; anchor: Anchor } | null>(null);
  let open = $state(false);
  let typed = $state("");

  const plotW = $derived(Math.max(40, width - PAD.l - PAD.r));
  const lo = $derived(Math.log(stacks[0] ?? 1));
  const hi = $derived(Math.log(stacks[stacks.length - 1] ?? 100));
  const frac = (s: number) => (hi > lo ? (Math.log(s) - lo) / (hi - lo) : 0);
  const x = (s: number) => PAD.l + Math.min(1, Math.max(0, frac(s))) * plotW;
  const y = (p: number) => PAD.t + (1 - p / 100) * (H - PAD.t - PAD.b);
  const bb = (s: number) => s.toLocaleString("de-DE", { maximumFractionDigits: 2 });

  function nearest(f: number): number {
    const target = lo + f * (hi - lo);
    return stacks.reduce((a, b) => (Math.abs(Math.log(b) - target) < Math.abs(Math.log(a) - target) ? b : a));
  }

  /** Lines break at the model border: below it the share is all-in, above it raise. */
  function path(points: { stack: number; pct: number }[]): string {
    let d = "";
    let prev: number | null = null;
    for (const p of points) {
      const jump = prev === null || (modelFrom !== null && prev < modelFrom && p.stack >= modelFrom);
      d += `${jump ? "M" : "L"}${x(p.stack).toFixed(1)},${y(p.pct).toFixed(1)}`;
      prev = p.stack;
    }
    return d;
  }

  const at = (s: Series, stack: number) => s.points.find((p) => p.stack === stack)?.pct ?? null;
  const current = $derived(series?.find((s) => s.seat === seat) ?? null);
  const currentPct = $derived(current ? at(current, value) : null);

  /** End labels, nudged apart so none overlap. */
  const labels = $derived.by(() => {
    if (!series) return [];
    const ends = series
      .filter((s) => s.points.length)
      .map((s) => {
        const p = s.points[s.points.length - 1];
        return { seat: s.seat, name: s.name, x: x(p.stack) + 9, y: y(p.pct), at: y(p.pct) };
      })
      .sort((a, b) => a.y - b.y);
    for (let i = 1; i < ends.length; i++) if (ends[i].y - ends[i - 1].y < 13) ends[i].y = ends[i - 1].y + 13;
    return ends;
  });

  function move(e: PointerEvent) {
    if (!series?.length) return;
    const svg = e.currentTarget as SVGSVGElement;
    const r = svg.getBoundingClientRect();
    const px = e.clientX - r.left;
    const py = e.clientY - r.top;
    const stack = nearest((px - PAD.l) / plotW);
    const near = series
      .map((s) => ({ s, p: at(s, stack) }))
      .filter((v) => v.p !== null)
      .sort((a, b) => Math.abs(y(a.p as number) - py) - Math.abs(y(b.p as number) - py))[0];
    if (!near) return (hover = null);
    const { clientX: cx, clientY: cy } = e;
    hover = { stack, seat: near.s.seat, anchor: { left: cx - 10, right: cx + 14, top: cy + 14, bottom: cy + 14 } };
  }

  const rows = $derived(
    hover && series
      ? series
          .map((s) => ({ seat: s.seat, name: s.name, p: at(s, hover!.stack) }))
          .filter((r) => r.p !== null)
          .sort((a, b) => (b.p as number) - (a.p as number))
      : [],
  );

  const ticks = $derived(TICKS.filter((t) => t >= (stacks[0] ?? 0) && t <= (stacks[stacks.length - 1] ?? 0)));
  const onGrid = $derived(stacks.includes(value));
  const parsed = $derived.by(() => {
    const v = Number(typed.replace(",", "."));
    return typed && Number.isFinite(v) && v > 0 && custom ? Math.round(v * 10 ** custom.digits) / 10 ** custom.digits : null;
  });
</script>

<div class="axis" bind:clientWidth={width}>
  <div class="top">
    <label class="lbl" for={id}><Hint text={TERM.stack}>Stack</Hint> <span class="faint">· die wichtigste Wahl</span></label>
    {#if !onGrid}<span class="own">eigener Stack</span>{/if}
    <output class="val num" for={id}>{bb(value)} <small>BB</small></output>
  </div>

  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions (a mouse shortcut: keys reach depth and seat through the slider and the table) -->
  <svg
    width={width}
    height={H}
    role="img"
    aria-label="Wie oft jede Position eröffnet, über die Stacktiefe"
    onpointermove={move}
    onpointerleave={() => (hover = null)}
    onclick={() => hover && (onPick(hover.stack), onPickSeat(hover.seat))}
  >
    {#if modelFrom !== null && modelFrom > (stacks[0] ?? 0)}
      <rect class="band" x={PAD.l} y={PAD.t - 16} width={Math.max(0, x(modelFrom) - 4 - PAD.l)} height={H - PAD.t - PAD.b + 16} rx="4" />
      <text class="band-t" x={PAD.l + 6} y={PAD.t - 5}>Push/Fold · Gleichgewicht</text>
      <text class="band-t" x={x(modelFrom) + 4} y={PAD.t - 5}>Mit Raise · Modell</text>
    {/if}
    {#each [0, 25, 50, 75, 100] as g (g)}
      <line class="grid" x1={PAD.l} x2={PAD.l + plotW} y1={y(g)} y2={y(g)} />
      <text class="y" x={PAD.l - 6} y={y(g) + 3.5}>{g} %</text>
    {/each}
    <line class="now" x1={x(value)} x2={x(value)} y1={PAD.t - 2} y2={H - PAD.b} />
    {#if series}
      {#each series as s (s.seat)}
        <path class="line" class:on={s.seat === seat} class:hot={hover?.seat === s.seat} d={path(s.points)} />
      {/each}
      {#each labels as l (l.seat)}
        {#if Math.abs(l.y - l.at) > 2}<path class="lead" d="M{l.x - 8},{l.at} L{l.x - 2},{l.y}" />{/if}
        <text class="end" class:on={l.seat === seat} x={l.x} y={l.y + 3.5}>{l.name}</text>
      {/each}
      {#if hover}
        {@const hs = series.find((s) => s.seat === hover!.seat)}
        {@const hp = hs ? at(hs, hover.stack) : null}
        <line class="cross" x1={x(hover.stack)} x2={x(hover.stack)} y1={PAD.t - 2} y2={H - PAD.b} />
        {#if hp !== null}<circle class="dot hot" cx={x(hover.stack)} cy={y(hp)} r="4.5" />{/if}
      {/if}
      {#if currentPct !== null}<circle class="dot" cx={x(value)} cy={y(currentPct)} r="5" />{/if}
    {:else}
      <text class="wait" x={PAD.l + plotW / 2} y={H / 2}>Kurve wird geladen …</text>
    {/if}
  </svg>

  <input
    {id}
    type="range"
    min="0"
    max="1000"
    step="1"
    value={Math.round(frac(value) * 1000)}
    style="margin-left:{PAD.l - 8}px;width:{plotW + 16}px;--f:{frac(value)}"
    data-stacks={stacks.join(",")}
    aria-valuetext="{bb(value)} BB"
    oninput={(e) => onPick(nearest(Number(e.currentTarget.value) / 1000))}
  />
  <div class="ticks" aria-hidden="true">
    {#each ticks as t (t)}
      <button type="button" tabindex="-1" class:on={value === t} style="left:{x(t)}px" onclick={() => onPick(nearest(frac(t)))}>{t}</button>
    {/each}
  </div>

  <div class="foot">
  {#if custom}
    {#if open}
      <form
        class="custom"
        onsubmit={(e) => {
          e.preventDefault();
          if (parsed !== null) onPick(parsed);
        }}
      >
        <input class="field num" inputmode="decimal" placeholder="z. B. {custom.example}" aria-label="Eigener Stack in BB" bind:value={typed} />
        <span class="unit">BB</span>
        <button class="btn" type="submit" disabled={parsed === null}>Rechnen</button>
        <button class="btn ghost" type="button" onclick={() => ((open = false), (typed = ""))}>Abbrechen</button>
        <span class="help">{custom.range ?? ""} <Hint text={TERM.live}>Wird live gerechnet</Hint>, das dauert etwas.</span>
      </form>
    {:else}
      <button type="button" class="more" onclick={() => (open = true)}>Anderen Stack eingeben …</button>
    {/if}
  {/if}

  {#if series?.length}
    <details class="table-view">
      <summary>Werte als Tabelle</summary>
      <div class="scroll">
        <table>
          <thead>
            <tr><th>Stack</th>{#each series as s (s.seat)}<th>{s.name}</th>{/each}</tr>
          </thead>
          <tbody>
            {#each stacks.filter((st) => series.some((s) => at(s, st) !== null)) as st (st)}
              <tr class:on={st === value}>
                <td class="num">{bb(st)} BB</td>
                {#each series as s (s.seat)}<td class="num">{at(s, st) === null ? "–" : `${pct(at(s, st) as number, 0)} %`}</td>{/each}
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    </details>
  {/if}
  </div>
</div>

{#if hover}
  <div class="tip axis-tip" use:floating={{ anchor: hover.anchor, side: "right" }}>
    <header><b class="num">{bb(hover.stack)} BB</b> · so oft wird eröffnet</header>
    {#each rows as r (r.seat)}
      <div class="row" class:hot={r.seat === hover.seat}><span>{r.name}</span><b>{pct(r.p as number, 0)} %</b></div>
    {/each}
    <footer>Klick: zu {rows.find((r) => r.seat === hover!.seat)?.name} mit {bb(hover.stack)} BB</footer>
  </div>
{/if}

<style>
  .axis {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .top {
    display: flex;
    align-items: baseline;
    gap: 10px;
    margin-bottom: 2px;
  }
  .lbl {
    font-size: 12px;
    color: var(--text-2);
  }
  .val {
    margin-left: auto;
    font-size: 26px;
    font-weight: 600;
    line-height: 1;
  }
  .val small {
    font-size: 13px;
    font-weight: 500;
    color: var(--text-2);
  }
  .own {
    padding: 0 6px;
    border-radius: 4px;
    font-size: 11px;
    line-height: 18px;
    color: var(--amber);
    background: color-mix(in srgb, var(--amber) 12%, transparent);
  }
  svg {
    display: block;
    cursor: crosshair;
    overflow: visible;
  }
  .band {
    fill: color-mix(in srgb, var(--pos) 6%, transparent);
  }
  .band-t {
    font-size: 11px;
    fill: var(--text-2);
  }
  .grid {
    stroke: var(--line);
    stroke-width: 1;
  }
  .y {
    font-family: var(--font-num);
    font-size: 11px;
    fill: var(--text-2);
    text-anchor: end;
  }
  .now {
    stroke: var(--text-3);
    stroke-width: 1;
    stroke-dasharray: 3 3;
  }
  .cross {
    stroke: var(--line-2);
    stroke-width: 1;
  }
  .line {
    fill: none;
    stroke: color-mix(in srgb, var(--text-2) 70%, var(--text-3));
    stroke-width: 1.5;
    stroke-linejoin: round;
    stroke-linecap: round;
    opacity: 0.85;
    transition:
      stroke var(--dur-fast) var(--ease-out),
      opacity var(--dur-fast) var(--ease-out);
  }
  .line.hot {
    stroke: var(--text-2);
    opacity: 1;
  }
  .line.on {
    stroke: var(--text);
    stroke-width: 2.25;
    opacity: 1;
  }
  .lead {
    fill: none;
    stroke: var(--text-3);
    stroke-width: 1;
  }
  .end {
    font-family: var(--font-num);
    font-size: 11.5px;
    fill: var(--text-2);
  }
  .end.on {
    fill: var(--text);
    font-weight: 600;
  }
  .dot {
    fill: var(--text);
    stroke: var(--surface);
    stroke-width: 2;
    transition:
      cx var(--dur) var(--ease-out),
      cy var(--dur) var(--ease-out);
  }
  .dot.hot {
    fill: var(--text-2);
  }
  .wait {
    font-size: 12px;
    fill: var(--text-3);
    text-anchor: middle;
  }
  input[type="range"] {
    -webkit-appearance: none;
    appearance: none;
    height: 20px;
    margin: 4px 0 0;
    background: transparent;
    cursor: pointer;
  }
  input[type="range"]::-webkit-slider-runnable-track {
    height: 4px;
    border-radius: 2px;
    background: linear-gradient(to right, var(--text-2) calc(var(--f) * 100%), var(--surface-3) calc(var(--f) * 100%));
  }
  input[type="range"]::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 16px;
    height: 16px;
    margin-top: -6px;
    border-radius: 50%;
    background: var(--text);
    border: 3px solid var(--surface);
    box-shadow: 0 0 0 1px var(--line-2);
  }
  input[type="range"]:focus-visible {
    outline: none;
  }
  input[type="range"]:focus-visible::-webkit-slider-thumb {
    box-shadow: 0 0 0 2px var(--focus);
  }
  .ticks {
    position: relative;
    height: 18px;
  }
  .ticks button {
    position: absolute;
    transform: translateX(-50%);
    padding: 0 3px;
    border: 0;
    background: none;
    color: var(--text-2);
    font-family: var(--font-num);
    font-size: 11.5px;
    cursor: pointer;
  }
  .ticks button:hover,
  .ticks button.on {
    color: var(--text);
  }
  .foot {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 4px 18px;
    margin-top: 2px;
  }
  .foot details[open] {
    flex-basis: 100%;
  }
  .more {
    align-self: flex-start;
    padding: 0;
    border: 0;
    background: none;
    color: var(--text-2);
    font-size: 12.5px;
    cursor: pointer;
  }
  .more:hover {
    color: var(--text);
    text-decoration: underline;
  }
  .custom {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }
  .custom .field {
    width: 84px;
  }
  .unit {
    font-size: 12.5px;
    color: var(--text-2);
    margin-right: 4px;
  }
  .help {
    font-size: 12px;
    color: var(--text-3);
  }
  .table-view {
    margin: 0;
  }
  .table-view summary {
    font-size: 12.5px;
    color: var(--text-2);
    cursor: pointer;
  }
  .table-view summary:hover {
    color: var(--text);
  }
  .scroll {
    overflow-x: auto;
    margin-top: 6px;
  }
  table {
    border-collapse: collapse;
    font-size: 12px;
  }
  th,
  td {
    padding: 3px 10px 3px 0;
    text-align: right;
    white-space: nowrap;
  }
  th {
    font-weight: 500;
    color: var(--text-3);
  }
  th:first-child,
  td:first-child {
    text-align: left;
  }
  td {
    border-top: 1px solid var(--line);
    color: var(--text-2);
  }
  tr.on td {
    color: var(--text);
    font-weight: 600;
  }
  .axis-tip {
    width: 210px;
  }
  .axis-tip .row.hot {
    color: var(--text);
    font-weight: 600;
  }
  .axis-tip .row {
    color: var(--text-2);
  }
  .axis-tip footer {
    margin-top: 6px;
    padding-top: 5px;
    border-top: 1px solid var(--line-2);
    font-size: 11.5px;
    color: var(--text-3);
  }
</style>
