<script lang="ts">
  import { STAT_META, fmt, moneyShort, stat, toneColor } from "../stats";
  import type { GameKind, HudConfig, Point, SeatView } from "../types";
  import { draggable } from "./drag";
  import { AVATAR_RADIUS, PLATE, RING, defaultPlate, type Side } from "./geometry";

  interface Props {
    seat: SeatView;
    hud: HudConfig;
    ring: Point;
    /** Plate top-left once moved on its own. */
    plate: Point | null;
    scale: number;
    /** Where the client's own name plate is. */
    side: Side;
    game: GameKind;
    currency: string | null;
    bountyCurrency: string | null;
    onRingMove: (p: Point, done: boolean) => void;
    onPlateMove: (p: Point, done: boolean) => void;
    onPlateReset: () => void;
    onTap: () => void;
  }

  let { seat, hud, ring, plate, scale: s, side, game, currency, bountyCurrency, onRingMove, onPlateMove, onPlateReset, onTap }: Props =
    $props();

  const cover = $derived(hud.graphic.cover_plate);
  const auto = $derived(defaultPlate(ring.x, ring.y, s, side, cover));
  const at = $derived(plate ?? auto);
  /** The ring overlaps the covering plate; keep text clear of it on that side. */
  const ringPad = $derived(cover && !plate ? (PLATE.behindAvatar + RING.size / 2 + 2) * s : 9 * s);
  /** Punch the avatar out of the plate so the client's picture stays visible inside the ring. */
  const mask = $derived.by(() => {
    const r = AVATAR_RADIUS * s;
    const g = `radial-gradient(circle ${r}px at ${ring.x - at.x}px ${ring.y - at.y}px, transparent ${r}px, #000 ${r + 0.5}px)`;
    return `-webkit-mask-image:${g}; mask-image:${g};`;
  });

  const hands = $derived(stat(seat.stats, "hands"));
  const plateStats = $derived(hud.graphic.plate_stats.slice(0, 3).map((id) => ({ id, s: stat(seat.stats, id) })));

  // Four arcs, clockwise from the top-left, with small gaps between them.
  const R = 50 - RING.stroke / 2;
  const GAP = 9;
  function arc(i: number) {
    const a0 = ((-180 + i * 90 + GAP / 2) * Math.PI) / 180;
    const a1 = ((-90 + i * 90 - GAP / 2) * Math.PI) / 180;
    const p = (a: number) => `${50 + R * Math.cos(a)} ${50 + R * Math.sin(a)}`;
    return `M ${p(a0)} A ${R} ${R} 0 0 1 ${p(a1)}`;
  }
  const segments = $derived(hud.graphic.segments.slice(0, 4).map((id, i) => ({ id, d: arc(i), color: toneColor(hud, stat(seat.stats, id), "#4a4f57") })));

  const stackText = $derived(game === "tournament" ? Math.round(seat.stack / 100).toLocaleString("de-DE") : moneyShort(seat.stack, currency));
  const segTitle = $derived(hud.graphic.segments.map((id) => `${STAT_META[id].name}: ${fmt(stat(seat.stats, id))}`).join(" · "));
</script>

<div class="seat" class:hero={seat.is_hero} style="--s:{s}; opacity:{hud.opacity}">
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="plate hit"
    class:cover
    class:compact={!cover}
    style="left:{at.x}px; top:{at.y}px; width:{auto.width}px; height:{auto.height}px; --pad-l:{side === 'right' && cover && !plate
      ? ringPad
      : 9 * s}px; --pad-r:{side === 'left' && cover && !plate ? ringPad : 9 * s}px; {mask}"
    title="Ziehen verschiebt die Platte · Doppelklick legt sie zurück · Klick zeigt alle Werte"
    use:draggable={{ origin: () => ({ x: at.x, y: at.y }), onMove: onPlateMove, onTap, onDouble: onPlateReset }}
  >
    {#if cover}
      <div class="row name">
        <span class="who">{seat.name}</span>
        {#if hud.show_bounty && seat.bounty !== null}
          <span class="ko num" title="Bounty auf diesen Spieler">{moneyShort(seat.bounty, bountyCurrency ?? "USD")}</span>
        {/if}
      </div>
    {/if}
    {#if cover && hud.show_stack}
      <div class="row stack num">
        <b>{stackText}</b>
        <span class="bb">{Math.round(seat.stack_bb)}</span><span class="unit">BB</span>
      </div>
    {/if}
    <div class="cols">
      {#each plateStats as p, i (i)}
        <div class="col">
          <b class="num" style="color:{toneColor(hud, p.s, 'var(--text-3)')}">{fmt(p.s)}</b>
          <i>{STAT_META[p.id].short}</i>
        </div>
      {/each}
    </div>
  </div>

  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="ring hit"
    style="left:{ring.x}px; top:{ring.y}px"
    title={segTitle}
    use:draggable={{ origin: () => ring, onMove: onRingMove, onTap }}
  >
    <svg viewBox="0 0 100 100" aria-hidden="true">
      <defs>
        <clipPath id="inner-{seat.seat}"><circle cx="50" cy="50" r={50 - RING.stroke - 1} /></clipPath>
      </defs>
      <circle class="track" cx="50" cy="50" r={R} stroke-width={RING.stroke + 3} />
      {#each segments as g, i (i)}
        <path d={g.d} stroke={g.color} stroke-width={RING.stroke} fill="none" stroke-linecap="butt" />
      {/each}
      <g clip-path="url(#inner-{seat.seat})">
        <rect x="0" y="70" width="100" height="30" class="band" />
      </g>
      <text x="50" y="83" class="count">{fmt(hands)}</text>
    </svg>
  </div>
</div>

<style>
  .seat {
    position: absolute;
    inset: 0;
    pointer-events: none;
  }
  .hit {
    pointer-events: auto;
    cursor: grab;
  }
  .hit:active {
    cursor: grabbing;
  }

  .plate {
    position: absolute;
    display: flex;
    flex-direction: column;
    justify-content: center;
    padding: 0 var(--pad-r) 0 var(--pad-l);
    border-radius: calc(9px * var(--s));
    background: var(--plate);
    box-shadow:
      0 0 0 1px var(--plate-edge) inset,
      0 calc(3px * var(--s)) calc(10px * var(--s)) rgba(0, 0, 0, 0.45);
    font-family: var(--font-num);
    line-height: 1.1;
    white-space: nowrap;
  }
  .hero .plate {
    box-shadow:
      0 0 0 1px rgba(236, 235, 231, 0.18) inset,
      0 calc(3px * var(--s)) calc(10px * var(--s)) rgba(0, 0, 0, 0.45);
  }
  .row {
    display: flex;
    align-items: baseline;
    gap: calc(4px * var(--s));
    min-width: 0;
  }
  .name {
    justify-content: space-between;
    font-family: var(--font-ui);
    font-size: calc(12px * var(--s));
    color: #d3d4d7;
  }
  .who {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .ko {
    flex-shrink: 0;
    padding: 0 calc(4px * var(--s));
    border-radius: calc(3px * var(--s));
    background: #3a3f47;
    color: #ecebe7;
    font-size: calc(10.5px * var(--s));
    line-height: 1.35;
  }
  .stack {
    margin-top: calc(2px * var(--s));
    font-size: calc(17px * var(--s));
  }
  .stack b {
    font-weight: 600;
    color: #f4f3f0;
  }
  .bb {
    margin-left: calc(3px * var(--s));
    font-weight: 600;
    color: var(--pos);
  }
  .unit {
    color: #8d9097;
    font-size: 0.85em;
  }
  .cols {
    display: flex;
    margin-top: calc(5px * var(--s));
  }
  .col {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    min-width: 0;
  }
  .col + .col {
    border-left: 1px solid rgba(255, 255, 255, 0.09);
  }
  .col b {
    font-size: calc(19px * var(--s));
    font-weight: 700;
    line-height: 1;
  }
  .col i {
    margin-top: calc(2px * var(--s));
    font-style: normal;
    font-size: calc(10px * var(--s));
    font-weight: 600;
    letter-spacing: 0.03em;
    color: #a4a7ad;
  }
  .compact .cols {
    margin-top: 0;
  }
  .compact .col b {
    font-size: calc(16px * var(--s));
  }
  .compact .col i {
    font-size: calc(9px * var(--s));
  }

  .ring {
    position: absolute;
    width: calc(74px * var(--s));
    height: calc(74px * var(--s));
    transform: translate(-50%, -50%);
    border-radius: 50%;
  }
  .ring svg {
    display: block;
    width: 100%;
    height: 100%;
    overflow: visible;
  }
  .track {
    fill: none;
    stroke: rgba(14, 15, 18, 0.85);
  }
  .band {
    fill: #101216;
  }
  .count {
    fill: #ffffff;
    font-family: var(--font-num);
    font-size: 15px;
    font-weight: 700;
    text-anchor: middle;
    dominant-baseline: middle;
  }
</style>
