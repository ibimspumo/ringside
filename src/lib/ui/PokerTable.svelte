<script lang="ts" module>
  import type { ActionKind, SizeStep } from "./poker";

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
  };
</script>

<script lang="ts">
  import { layout, minHeight, placeOf, type Point, type Sizes } from "../table/seats";
  import ActionLabel from "./ActionLabel.svelte";
  import { bb, num } from "./format";
  import PlayingCard from "./PlayingCard.svelte";

  /**
   * Table for every view with seats: "you" (`hero`) bottom centre, the others clockwise. The
   * geometry comes from table/seats.ts: action tag and chips sit on the line from each seat to
   * the middle, at the same pixel distance for all seats.
   */
  let {
    seats,
    hero = 0,
    board = [],
    pot,
    label = "Pokertisch",
  }: { seats: TableSeat[]; hero?: number; board?: string[]; pot?: number; label?: string } = $props();

  const SIZES: Sizes = {
    seat: { w: 112, h: 74 },
    me: { w: 124, h: 88 },
    badge: { w: 70, h: 22 },
    bet: { w: 52, h: 22 },
    dealer: 18,
    gap: 6,
    margin: 8,
  };
  /** Pot above the board, both centred on the middle. */
  const MIDDLE = [
    { box: { w: 110, h: 20 }, dy: -28 },
    { box: { w: 170, h: 42 }, dy: 6 },
  ];

  let w = $state(640);
  const n = $derived(seats.length);
  const h = $derived(Math.max(minHeight(SIZES, MIDDLE), Math.round(w * 0.66)));
  const table = $derived(layout(n, w, h, SIZES));
  const place = (i: number) => table.places[placeOf(i, hero, n)];
  const at = (p: Point) => `left:${p.x}px; top:${p.y}px`;
  /** A chip or tag starts its glide at the seat. */
  const from = (i: number, p: Point) => {
    const s = place(i).seat;
    return `--dx:${(s.x - p.x).toFixed(1)}px; --dy:${(s.y - p.y).toFixed(1)}px`;
  };
</script>

<div class="ui-table" role="group" aria-label={label} bind:clientWidth={w} style:height="{h}px">
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
    {#if pot !== undefined}<span class="pot num">Pot {bb(pot)}</span>{/if}
    <div class="board" aria-label="Board">
      {#each board as c, k (c)}<PlayingCard card={c} size={30} reveal delay={k * 70} />{/each}
    </div>
  </div>

  {#each seats as s, i (i)}
    {#if !s.empty}
      {@const p = place(i)}
      {#if s.bet}
        <span class="bet num" style="{at(p.bet)}; {from(i, p.bet)}"><i aria-hidden="true"></i>{num(s.bet, s.bet % 1 ? 1 : 0)}</span>
      {/if}
      {#if s.action}
        <span class="tag" style="{at(p.badge)}; {from(i, p.badge)}"><ActionLabel {...s.action} amount={undefined} size="sm" /></span>
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
      style={at(place(i).seat)}
      aria-label={s.empty ? "freier Platz" : `${i === hero ? "Du" : (s.name ?? s.position ?? `Platz ${i + 1}`)}${s.toAct ? ", am Zug" : ""}${s.folded ? ", ausgestiegen" : ""}`}
    >
      {#if s.cards && !s.folded}
        <div class="cards">
          {#each s.cards === "hidden" ? [undefined, undefined] : s.cards as c, k (k)}<PlayingCard card={c} size={i === hero ? 30 : 22} hidden={!c} />{/each}
        </div>
      {/if}
      {#if !s.empty}
        <div class="plate">
          <span class="who">
            {#if s.position}<b>{s.position}</b>{/if}
            <span class="name">{i === hero ? "Du" : (s.name ?? "")}</span>
          </span>
          {#if s.stack !== undefined}<span class="stack num">{bb(s.stack)}</span>{/if}
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
  .folded {
    opacity: 0.42;
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
