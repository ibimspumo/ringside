<script lang="ts">
  import { STAT_META, fill, fmt, moneyShort, stat, toneColor, unit } from "../stats";
  import type { GameKind, HudConfig, Point, SeatView, StatId } from "../types";
  import { draggable } from "./drag";
  import { AVATAR_RADIUS, PLATE, RING, defaultPlate, type Side } from "./geometry";

  interface Props {
    seat: SeatView;
    hud: HudConfig;
    /** Avatar centre; ring and plate always move together. */
    ring: Point;
    scale: number;
    /** Where the client's own name plate is. */
    side: Side;
    game: GameKind;
    currency: string | null;
    bountyCurrency: string | null;
    onRingMove: (p: Point, done: boolean) => void;
    /** Click on the ring: all numbers of this player. */
    onTap: () => void;
    /** Which page of `plate_pages` the table shows; a click on a plate asks for the next. */
    page: number;
    onNextPage: () => void;
  }

  let { seat, hud, ring, scale: s, side, game, currency, bountyCurrency, onRingMove, onTap, page, onNextPage }: Props =
    $props();

  const cover = $derived(hud.graphic.cover_plate);
  const auto = $derived(defaultPlate(ring.x, ring.y, s, side, cover, hud.graphic.plate_width ?? 1));
  const at = $derived(auto);
  /** The ring overlaps the covering plate; keep text clear of it on that side. */
  const ringPad = $derived(cover ? (PLATE.behindAvatar + RING.size / 2 + 2) * s : 9 * s);
  /** Round the ring side with the ring's radius, so the plate ends in a half circle around it. */
  const corners = $derived.by(() => {
    const r = `${(RING.size / 2) * s}px`;
    const c = `${9 * s}px`;
    if (!cover) return c;
    return side === "left" ? `${c} ${r} ${r} ${c}` : `${r} ${c} ${c} ${r}`;
  });
  /** Punch the avatar out of the plate so the client's picture stays visible inside the ring. */
  const mask = $derived.by(() => {
    const r = AVATAR_RADIUS * s;
    const g = `radial-gradient(circle ${r}px at ${ring.x - at.x}px ${ring.y - at.y}px, transparent ${r}px, #000 ${r + 0.5}px)`;
    return `-webkit-mask-image:${g}; mask-image:${g};`;
  });

  const hands = $derived(stat(seat.stats, "hands"));
  const pages = $derived<StatId[][]>(hud.graphic.plate_pages?.length ? hud.graphic.plate_pages : [["vpip", "pfr", "three_bet"]]);
  const plateStats = $derived(pages[page % pages.length].slice(0, 3).map((id) => ({ id, s: stat(seat.stats, id) })));

  // Four quarter tracks, clockwise from the top-left, with small gaps. Each fills with its
  // stat's value (percent of 100, AF of 5) in the stat's colour.
  const R = 50 - RING.stroke / 2;
  const GAP = 9;
  function arc(i: number, part = 1) {
    const a0 = -180 + i * 90 + GAP / 2;
    const a1 = a0 + (90 - GAP) * part;
    const p = (deg: number) => `${50 + R * Math.cos((deg * Math.PI) / 180)} ${50 + R * Math.sin((deg * Math.PI) / 180)}`;
    return `M ${p(a0)} A ${R} ${R} 0 0 1 ${p(a1)}`;
  }
  const segments = $derived(
    hud.graphic.segments.slice(0, 4).map((id, i) => {
      const s = stat(seat.stats, id);
      const f = fill(s);
      return { id, track: arc(i), d: f > 0.005 ? arc(i, f) : null, color: toneColor(hud, s, "#4a4f57") };
    }),
  );

  const stackText = $derived(game === "tournament" ? Math.round(seat.stack / 100).toLocaleString("de-DE") : moneyShort(seat.stack, currency));
  const segTitle = $derived(hud.graphic.segments.map((id) => `${STAT_META[id].name}: ${fmt(stat(seat.stats, id))}`).join(" · "));
</script>

<div class="seat" class:hero={seat.is_hero} class:labelled={seat.note?.label} style="--s:{s}; --label:{seat.note?.label?.color ?? 'transparent'}">
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="plate hit"
    class:cover
    class:compact={!cover}
    style="left:{at.x}px; top:{at.y}px; width:{auto.width}px; height:{auto.height}px; border-radius:{corners}; --pad-l:{side === 'right' && cover
      ? ringPad
      : 9 * s}px; --pad-r:{side === 'left' && cover ? ringPad : 9 * s}px; {mask}"
    title="Klick wechselt die drei Werte an allen Plätzen · Ziehen verschiebt Ring und Platte"
    use:draggable={{ origin: () => ring, onMove: onRingMove, onTap: onNextPage }}
  >
    {#if cover}
      <div class="row name">
        <span class="who">{seat.name}</span>
        {#if pages.length > 1}
          <span class="dots" aria-hidden="true">
            {#each pages as _, i (i)}<i class:on={i === page % pages.length}></i>{/each}
          </span>
        {/if}
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
        <div class="col" title={STAT_META[p.id].help}>
          <b class="num" style="color:{toneColor(hud, p.s, 'var(--text-3)')}">{fmt(p.s)}<small>{unit(p.s)}</small></b>
          <i>{STAT_META[p.id].short}</i>
        </div>
      {/each}
    </div>

  </div>

  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="ring hit"
    style="left:{ring.x}px; top:{ring.y}px"
    title="{segTitle} · {fmt(hands)} gemeinsame Hände · Klick zeigt alle Werte"
    use:draggable={{ origin: () => ring, onMove: onRingMove, onTap }}
  >
    <svg viewBox="0 0 100 100" aria-hidden="true">
      <defs>
        <clipPath id="inner-{seat.seat}"><circle cx="50" cy="50" r={50 - RING.stroke - 1} /></clipPath>
      </defs>
      <circle class="track" cx="50" cy="50" r={R} stroke-width={RING.stroke + 3} />
      {#each segments as g, i (i)}
        <path d={g.track} class="quarter" stroke-width={RING.stroke} fill="none" />
        {#if g.d}<path d={g.d} stroke={g.color} stroke-width={RING.stroke} fill="none" stroke-linecap="butt" />{/if}
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
  /* The colour the user gave this player in the client; the plate covers the client's own ring. */
  .labelled .plate {
    box-shadow:
      0 0 0 calc(2.5px * var(--s)) var(--label) inset,
      0 calc(3px * var(--s)) calc(10px * var(--s)) rgba(0, 0, 0, 0.45);
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
    margin-top: calc(1px * var(--s));
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
    margin-top: calc(3px * var(--s));
    padding-bottom: calc(2px * var(--s));
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
  .col b small {
    margin-left: calc(1px * var(--s));
    font-size: 0.5em;
    font-weight: 600;
    opacity: 0.75;
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
    stroke: rgba(14, 15, 18, 0.9);
  }
  .quarter {
    stroke: #33373e;
  }
  .plate {
    position: absolute;
  }
  .dots {
    display: flex;
    align-self: center;
    gap: calc(3px * var(--s));
    margin-left: auto;
    margin-right: calc(4px * var(--s));
  }
  .dots i {
    width: calc(3.5px * var(--s));
    height: calc(3.5px * var(--s));
    border-radius: 50%;
    background: #4a4f57;
  }
  .dots i.on {
    background: #c6c8cc;
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
