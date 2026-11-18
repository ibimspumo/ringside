<script lang="ts" module>
  import { minHeight, type Box, type Sizes } from "../table/seats";
  import type { ActionKind, Grade, SizeStep } from "./poker";

  /**
   * Below this width the full plates leave no room round the board, so the table turns compact:
   * smaller plates at the same type sizes, action tags on the plate's edge, a taller stage.
   */
  export const COMPACT_BELOW = 760;

  const FULL: Sizes = {
    seat: { w: 112, h: 74 },
    me: { w: 124, h: 88 },
    badge: { w: 92, h: 22 },
    bet: { w: 64, h: 22 },
    dealer: 18,
    gap: 6,
    margin: 8,
  };
  const COMPACT: Sizes = {
    seat: { w: 84, h: 52 },
    me: { w: 100, h: 74 },
    badge: { w: 64, h: 20 },
    bet: { w: 52, h: 20 },
    dealer: 16,
    gap: 4,
    margin: 6,
    onEdge: true,
  };
  /** Pot, board and street name, stacked in the middle. */
  const MIDDLE = { full: { w: 186, h: 92 }, compact: { w: 140, h: 70 } };

  /** One size step from about 900 px on, at most a third larger. */
  export const scaleAt = (w: number) => (w < COMPACT_BELOW ? 1 : Math.min(1.3, Math.max(1, w / 900)));

  export function sizesAt(w: number): Sizes {
    if (w < COMPACT_BELOW) return COMPACT;
    const k = scaleAt(w);
    const grow = (b: Box) => ({ w: b.w * k, h: b.h * k });
    return { ...FULL, seat: grow(FULL.seat), me: grow(FULL.me ?? FULL.seat), badge: grow(FULL.badge), bet: grow(FULL.bet), dealer: 18 * k, gap: 6 * k };
  }

  /** The table's height at a width; callers fitting it into a box use it to find the width. */
  export function tableHeight(w: number): number {
    const compact = w < COMPACT_BELOW;
    const k = scaleAt(w);
    const mid = compact ? MIDDLE.compact : { w: MIDDLE.full.w * k, h: MIDDLE.full.h * k };
    return Math.max(minHeight(sizesAt(w), [{ box: mid, dy: 0 }]), Math.round(w * (compact ? 0.85 : 0.66)));
  }

  export type TableSeat = {
    name?: string;
    /** "BTN", "SB", … */
    position?: string;
    /** In big blinds. */
    stack?: number;
    /** Face-up cards like ["Qs", "8s"], or "hidden" for backs. */
    cards?: string[] | "hidden";
    /** Last action of the current street; shown on the line towards the middle. */
    action?: { kind: ActionKind; amount?: number; step?: SizeStep };
    /** Chips in front of the seat this street, in big blinds. */
    bet?: number;
    folded?: boolean;
    toAct?: boolean;
    dealer?: boolean;
    empty?: boolean;
    /** Won at the end of the hand, in big blinds: the plate turns green, the amount sits on the line. */
    won?: number;
    /** Grade of the action on the line, e.g. your rated decision while a hand is replayed. */
    grade?: Grade;
  };
</script>

<script lang="ts">
  import { layout, placeOf, type Point } from "../table/seats";
  import ActionLabel from "./ActionLabel.svelte";
  import { bb, num } from "./format";
  import GradeIcon from "./GradeIcon.svelte";
  import { GRADE } from "./poker";
  import PlayingCard from "./PlayingCard.svelte";

  /**
   * Table for every view with seats: "you" (`hero`) bottom centre, the others clockwise. The
   * geometry comes from table/seats.ts: action tag and chips sit on the line from each seat to
   * the middle, at the same pixel distance for all seats. Plates keep their size on small
   * tables and grow a step on wide ones, so names and stacks stay readable everywhere.
   * `format` writes amounts (given in big blinds), e.g. as money; `note` sits under the board.
   */
  let {
    seats,
    hero = 0,
    board = [],
    pot,
    note,
    format,
    label = "Pokertisch",
  }: {
    seats: TableSeat[];
    hero?: number;
    board?: string[];
    pot?: number;
    note?: string;
    format?: (bb: number) => string;
    label?: string;
  } = $props();

  let w = $state(640);
  const k = $derived(scaleAt(w));
  const SIZES = $derived(sizesAt(w));
  const n = $derived(seats.length);
  const h = $derived(tableHeight(w));
  const table = $derived(layout(n, w, h, SIZES));
  const amount = (v: number) => (format ? format(v) : num(v, v % 1 ? 1 : 0));
  const place = (i: number) => table.places[placeOf(i, hero, n)];
  const at = (p: Point) => `left:${p.x}px; top:${p.y}px`;
  /** A chip or tag starts its glide at the seat. */
  const from = (i: number, p: Point) => {
    const s = place(i).seat;
    return `--dx:${(s.x - p.x).toFixed(1)}px; --dy:${(s.y - p.y).toFixed(1)}px`;
  };
</script>

<div class="ui-table" role="group" aria-label={label} bind:clientWidth={w} style:height="{h}px" style:--k={k} class:compact={w < COMPACT_BELOW}>
  <div
    class="rail"
    style:left="{table.felt.cx - table.felt.rx}px"
    style:top="{table.felt.cy - table.felt.ry}px"
    style:width="{table.felt.rx * 2}px"
    style:height="{table.felt.ry * 2}px"
  >
    <div class="cloth"></div>
  </div>

  <div class="middle" style={at(table.center)}>
    {#if pot !== undefined}<span class="pot num" class:hidden={pot <= 0}>Pot {format ? format(pot) : bb(pot)}</span>{/if}
    <div class="board" aria-label="Board">
      {#each board as c, j (c)}<PlayingCard card={c} size={w < COMPACT_BELOW ? 25 : Math.round(32 * k)} reveal delay={j * 70} />{/each}
    </div>
    {#if note}<span class="note">{note}</span>{/if}
  </div>

  {#each seats as s, i (i)}
    {#if !s.empty}
      {@const p = place(i)}
      {#if s.bet}
        <span class="bet num" style="{at(p.bet)}; {from(i, p.bet)}"><i aria-hidden="true"></i>{amount(s.bet)}</span>
      {/if}
      {#if s.won !== undefined}
        <span class="tag won num" style="{at(p.badge)}; {from(i, p.badge)}">+{amount(s.won)}</span>
      {:else if s.action}
        <span class="tag" class:graded={!!s.grade} style="{at(p.badge)}; {from(i, p.badge)}" title={s.grade ? GRADE[s.grade].label : undefined}
          ><ActionLabel {...s.action} amount={undefined} size="sm" />{#if s.grade}<GradeIcon grade={s.grade} size={15} />{/if}</span
        >
      {/if}
      {#if s.dealer}<span class="dealer" style={at(p.dealer)} title="Dealer-Button">D</span>{/if}
    {/if}
  {/each}

  {#each seats as s, i (i)}
    <div
      class="seat"
      class:hero={i === hero}
      class:folded={s.folded}
      class:act={s.toAct}
      class:empty={s.empty}
      class:winner={s.won !== undefined}
      style={at(place(i).seat)}
      aria-label={s.empty ? "freier Platz" : `${i === hero ? "Du" : (s.name ?? s.position ?? `Platz ${i + 1}`)}${s.toAct ? ", am Zug" : ""}${s.folded ? ", ausgestiegen" : ""}${s.won !== undefined ? ", gewinnt" : ""}`}
    >
      {#if s.cards && !s.folded}
        <div class="cards">
          {#each s.cards === "hidden" ? [undefined, undefined] : s.cards as c, j (j)}<PlayingCard
              card={c}
              size={(i === hero || s.cards !== "hidden" ? 30 : 22) - (w < COMPACT_BELOW ? 5 : 0)}
              hidden={!c}
              reveal={i !== hero && s.cards !== "hidden"}
              delay={j * 60}
            />{/each}
        </div>
      {/if}
      {#if !s.empty}
        <div class="plate">
          <span class="who">
            {#if s.position}<b>{s.position}</b>{/if}
            <span class="name" title={i === hero ? undefined : s.name}>{i === hero ? "Du" : (s.name ?? "")}</span>
          </span>
          {#if s.stack !== undefined}<span class="stack num">{format ? format(s.stack) : bb(s.stack)}</span>{/if}
        </div>
      {:else}
        <div class="plate free">frei</div>
      {/if}
    </div>
  {/each}
</div>

<style>
  .ui-table {
    position: relative;
    width: 100%;
  }
  .rail {
    position: absolute;
    padding: 7px;
    border-radius: var(--r-pill);
    background: var(--rail);
    box-shadow:
      inset 0 0 0 1px var(--line-2),
      var(--shadow-2);
  }
  .cloth {
    width: 100%;
    height: 100%;
    border-radius: var(--r-pill);
    background: radial-gradient(ellipse at 50% 42%, var(--felt) 0%, var(--felt-edge) 100%);
    box-shadow: inset 0 0 0 1px var(--line);
  }
  .middle {
    position: absolute;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    transform: translate(-50%, -50%);
  }
  .pot.hidden {
    visibility: hidden;
  }
  .note {
    color: var(--text-3);
    font-size: var(--fs-xs);
    font-weight: var(--fw-semibold);
    scale: var(--k);
  }
  .pot {
    padding: 2px var(--sp-2);
    border-radius: var(--r-pill);
    background: var(--grid-plate);
    color: var(--text);
    font-size: var(--fs-sm);
    font-weight: var(--fw-semibold);
  }
  .board {
    display: flex;
    gap: 4px;
  }
  .seat,
  .bet,
  .tag,
  .dealer {
    position: absolute;
    transform: translate(-50%, -50%);
    scale: var(--k);
  }
  .pot {
    scale: var(--k);
  }
  .seat {
    display: flex;
    flex-direction: column;
    align-items: center;
    z-index: 2;
  }
  .cards {
    display: flex;
    gap: 2px;
    margin-bottom: -6px;
    z-index: 1;
  }
  .plate {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    min-width: 88px;
    padding: 5px 10px;
    border-radius: var(--r-md);
    border: 1px solid var(--line-2);
    background: var(--surface-2);
    box-shadow: var(--shadow-1);
  }
  .who {
    display: flex;
    gap: 5px;
    align-items: baseline;
    max-width: 110px;
    font-size: var(--fs-sm);
    line-height: 1.1;
  }
  .who b {
    color: var(--text-2);
    font-size: var(--fs-xs);
    font-weight: var(--fw-semibold);
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .stack {
    color: var(--text);
    font-size: var(--fs-md);
    font-weight: var(--fw-semibold);
    line-height: 1.1;
  }
  .compact .plate {
    min-width: 76px;
    padding: 3px var(--sp-2);
  }
  .compact .who {
    max-width: 76px;
  }
  .compact .middle {
    gap: 4px;
  }
  .hero .plate {
    border-color: var(--text);
    box-shadow:
      0 0 0 1px var(--text),
      var(--shadow-1);
  }
  .act .plate {
    border-color: var(--amber);
    box-shadow:
      0 0 0 2px color-mix(in srgb, var(--amber) 45%, transparent),
      var(--shadow-1);
  }
  .winner .plate {
    border-color: var(--pos);
    box-shadow:
      0 0 0 2px color-mix(in srgb, var(--pos) 40%, transparent),
      var(--shadow-1);
  }
  .folded {
    opacity: 0.42;
    transition: opacity var(--dur) var(--ease-standard);
  }
  .free {
    min-width: 64px;
    border-style: dashed;
    background: transparent;
    color: var(--text-3);
    font-size: var(--fs-xs);
  }
  .dealer {
    display: grid;
    place-items: center;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: var(--text);
    color: var(--bg);
    font-size: var(--fs-xs);
    font-weight: var(--fw-bold);
    box-shadow: var(--shadow-1);
  }
  .bet {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 2px 7px 2px 3px;
    border-radius: var(--r-pill);
    background: var(--grid-plate);
    color: var(--text);
    font-size: var(--fs-sm);
    font-weight: var(--fw-semibold);
    white-space: nowrap;
    z-index: 1;
  }
  .bet i {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--chip);
    box-shadow:
      inset 0 0 0 2px var(--act-bet-3),
      inset 0 0 0 3.5px var(--chip);
  }
  .tag {
    z-index: 1;
  }
  .graded {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    padding-right: 3px;
    border-radius: var(--r-pill);
    background: var(--grid-plate);
  }
  .won {
    padding: 1px var(--sp-2);
    border-radius: var(--r-pill);
    border: 1px solid color-mix(in srgb, var(--pos) 55%, transparent);
    background: color-mix(in srgb, var(--pos) 16%, var(--bg));
    color: var(--pos);
    font-size: var(--fs-sm);
    font-weight: var(--fw-semibold);
    white-space: nowrap;
  }
  .bet,
  .tag {
    animation: glide var(--dur-slow) var(--ease-enter) both;
  }
  .bet {
    animation-delay: 60ms;
  }
  @keyframes glide {
    from {
      opacity: 0;
      transform: translate(calc(-50% + var(--dx)), calc(-50% + var(--dy))) scale(0.8);
    }
  }
</style>
