<script lang="ts">
  import type { SeatMark } from "../articles";
  import { POSITIONS, type Seats } from "../seats";

  let {
    seats,
    marks = [],
    order = null,
    selected = null,
    onpick,
  }: {
    seats: Seats;
    marks?: SeatMark[];
    /** Action order per position, shown as small numbers. */
    order?: Record<string, number> | null;
    selected?: string | null;
    onpick?: (pos: string) => void;
  } = $props();

  const uid = $props.id();
  const W = 640;
  const H = 360;
  const CX = W / 2;
  const CY = H / 2;

  const layout = $derived.by(() => {
    const names = POSITIONS[seats];
    const btn = names.indexOf("BTN");
    return names.map((pos, i) => {
      // Button at the bottom, action runs clockwise.
      const a = Math.PI / 2 + ((i - btn) * 2 * Math.PI) / names.length;
      const x = CX + 262 * Math.cos(a);
      const y = CY + 140 * Math.sin(a);
      return { pos, x, y, mark: marks.find((m) => m.pos === pos) };
    });
  });

  const towards = (x: number, y: number, k: number) => ({ x: CX + (x - CX) * k, y: CY + (y - CY) * k });
</script>

{#snippet seat(x: number, y: number, pos: string)}
  <rect x={x - 42} y={y - 18} width="84" height="36" rx="18" />
  <text x={x} y={y + 5}>{pos}</text>
  {#if order?.[pos]}
    <circle class="ord" cx={x + 40} cy={y - 16} r="11" />
    <text class="ord-t" x={x + 40} y={y - 12}>{order[pos]}</text>
  {/if}
{/snippet}

<svg viewBox="0 0 {W} {H}" class="table" role="img" aria-label="Pokertisch mit {seats} Plätzen">
  <defs>
    <radialGradient id="cloth-{uid}" cx="50%" cy="42%" r="60%">
      <stop offset="0%" stop-color="#23312d" />
      <stop offset="62%" stop-color="#1b2623" />
      <stop offset="100%" stop-color="#16201d" />
    </radialGradient>
  </defs>
  <ellipse cx={CX} cy={CY} rx="214" ry="112" fill="#1c1f24" stroke="rgba(236,235,231,0.06)" />
  <ellipse cx={CX} cy={CY} rx="202" ry="100" fill="url(#cloth-{uid})" stroke="rgba(0,0,0,0.4)" />

  {#each layout as s (s.pos)}
    {@const m = s.mark}
    {#if s.pos === "BTN"}
      {@const d = { x: s.x + 62, y: s.y + 2 }}
      <g class="dealer">
        <circle cx={d.x} cy={d.y} r="11" />
        <text x={d.x} y={d.y + 4}>D</text>
      </g>
    {/if}
    {#if m?.chip}
      {@const c = towards(s.x, s.y, 0.58)}
      <g class="chip" class:allin={m.state === "allin"}>
        <circle cx={c.x - 22} cy={c.y} r="8" />
        <text x={c.x - 9} y={c.y + 5}>{m.chip}</text>
      </g>
    {/if}
    {#if onpick}
      <g
        class="seat pickable {m?.state ?? ''}"
        class:selected={selected === s.pos}
        role="button"
        tabindex="0"
        aria-label="Sitz {s.pos}"
        onclick={() => onpick(s.pos)}
        onkeydown={(e) => (e.key === "Enter" || e.key === " ") && (e.preventDefault(), onpick(s.pos))}
      >
        {@render seat(s.x, s.y, s.pos)}
      </g>
    {:else}
      <g class="seat {m?.state ?? ''}" class:selected={selected === s.pos}>{@render seat(s.x, s.y, s.pos)}</g>
    {/if}
    {#if m?.note}
      <text class="note" x={s.x} y={s.y < CY ? s.y - 26 : s.y + 34}>{m.note}</text>
    {/if}
  {/each}
</svg>

<style>
  .table {
    display: block;
    width: 100%;
    height: auto;
    max-height: 340px;
    font-family: var(--font-num);
  }
  .seat rect {
    fill: var(--surface-3);
    stroke: var(--line-2);
    stroke-width: 1;
    transition:
      fill 0.15s,
      stroke 0.15s;
  }
  .seat text {
    fill: var(--text);
    font-size: 15px;
    font-weight: 600;
    text-anchor: middle;
    letter-spacing: 0.02em;
  }
  .seat.fold rect {
    fill: var(--surface);
    stroke: var(--line);
  }
  .seat.fold text {
    fill: var(--text-3);
  }
  .seat.active rect {
    stroke: var(--text-2);
    stroke-width: 1.5;
  }
  .seat.hero rect,
  .seat.selected rect {
    fill: var(--text);
    stroke: var(--text);
  }
  .seat.hero text,
  .seat.selected text {
    fill: var(--bg);
  }
  .seat.allin rect {
    stroke: var(--amber);
    stroke-width: 2;
  }
  .pickable {
    cursor: pointer;
  }
  .pickable:hover rect {
    stroke: var(--text);
  }
  .pickable:focus {
    outline: none;
  }
  .pickable:focus-visible rect {
    stroke: var(--focus);
    stroke-width: 2;
  }
  .ord {
    fill: var(--bg);
    stroke: var(--text-2);
  }
  .seat text.ord-t {
    fill: var(--text);
    font-size: 12px;
  }
  .seat.selected text.ord-t {
    fill: var(--text);
  }
  .dealer circle {
    fill: #f3f2ee;
  }
  .dealer text {
    fill: #1d2026;
    font-size: 12px;
    font-weight: 700;
    text-anchor: middle;
  }
  .chip circle {
    fill: var(--text-2);
    stroke: var(--bg);
    stroke-width: 2;
    stroke-dasharray: 3 2;
  }
  .chip.allin circle {
    fill: var(--amber);
  }
  .chip text {
    fill: var(--text);
    font-size: 14px;
    font-weight: 600;
  }
  .note {
    fill: var(--text-2);
    font-family: var(--font-ui);
    font-size: 13px;
    text-anchor: middle;
  }
</style>
