<script lang="ts" module>
  import { STAGE } from "../table/seats";
  import type { ActionKind, Grade, SizeStep } from "./poker";

  /** Height over width: the table keeps this shape at every size. */
  export const TABLE_RATIO = STAGE.h / STAGE.w;
  /** Narrowest table whose names and stacks are still comfortable to read. */
  export const CLEAN_FROM = 600;

  /** The table's height at a width; callers fitting it into a box use it to find the width. */
  export const tableHeight = (w: number) => Math.round(w * TABLE_RATIO);

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
  import { layout, placeOf, STAGE_CARDS, STAGE_SIZES, type Point } from "../table/seats";
  import ActionLabel from "./ActionLabel.svelte";
  import { bb, num } from "./format";
  import GradeIcon from "./GradeIcon.svelte";
  import { GRADE } from "./poker";
  import PlayingCard from "./PlayingCard.svelte";

  /**
   * Table for every view with seats: "you" (`hero`) bottom centre, the others clockwise. It is
   * laid out once on the fixed stage of table/seats.ts and scaled as a whole to its width, so
   * plates, cards and chips keep their places and proportions at every size. Action, win and
   * dealer button have a row of their own on the name plate; chips sit on the line from each
   * seat to the middle, clear of every seat block.
   * `format` writes amounts (given in big blinds), e.g. as money; `note` sits under the board.
   */
  let {
    seats,
    hero = 0,
    board = [],
    pot,
    note,
    noteWin = false,
    noteAside,
    format,
    label = "Pokertisch",
  }: {
    seats: TableSeat[];
    hero?: number;
    board?: string[];
    pot?: number;
    note?: string;
    /** The note says who won: it stands out instead of whispering the street. */
    noteWin?: boolean;
    /** A quieter second part of the note, e.g. your own hand beside the winner's. */
    noteAside?: string;
    format?: (bb: number) => string;
    label?: string;
  } = $props();

  let w = $state(STAGE.w);
  const n = $derived(seats.length);
  const table = $derived(layout(n, STAGE.w, STAGE.h, STAGE_SIZES));
  const amount = (v: number) => (format ? format(v) : num(v, v % 1 ? 1 : 0));
  const place = (i: number) => table.places[placeOf(i, hero, n)];
  const at = (p: Point) => `left:${p.x}px; top:${p.y}px`;
  /** A chip starts its glide at the seat. */
  const from = (i: number, p: Point) => {
    const s = place(i).seat;
    return `--dx:${(s.x - p.x).toFixed(1)}px; --dy:${(s.y - p.y).toFixed(1)}px`;
  };
</script>

<div class="ui-table" role="group" aria-label={label} bind:clientWidth={w} style:aspect-ratio="{STAGE.w} / {STAGE.h}">
<div class="stage" style:width="{STAGE.w}px" style:height="{STAGE.h}px" style:transform="scale({w / STAGE.w})">
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
      {#each board as c, j (c)}<PlayingCard card={c} size={STAGE_CARDS.board} reveal delay={j * 70} />{/each}
    </div>
    {#if note}<span class="note" class:win={noteWin}>{note}{#if noteAside}<span class="aside">· {noteAside}</span>{/if}</span>{/if}
  </div>

  {#each seats as s, i (i)}
    {#if !s.empty}
      {@const p = place(i)}
      {#if s.bet}
        <span class="bet num" style="{at(p.bet)}; {from(i, p.bet)}"><i aria-hidden="true"></i>{amount(s.bet)}</span>
      {/if}
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
      style="{at(place(i).seat)}; width:{place(i).box.w}px"
      aria-label={s.empty ? "freier Platz" : `${i === hero ? "Du" : (s.name ?? s.position ?? `Platz ${i + 1}`)}${s.toAct ? ", am Zug" : ""}${s.folded ? ", ausgestiegen" : ""}${s.won !== undefined ? ", gewinnt" : ""}`}
    >
      {#if s.cards && !s.folded}
        <div class="cards">
          {#each s.cards === "hidden" ? [undefined, undefined] : s.cards as c, j (j)}<PlayingCard
              card={c}
              size={i === hero ? STAGE_CARDS.hero : s.cards === "hidden" ? STAGE_CARDS.hidden : STAGE_CARDS.shown}
              hidden={!c}
              reveal={i !== hero && s.cards !== "hidden"}
              delay={j * 60}
            />{/each}
        </div>
      {/if}
      {#if !s.empty}
        <div class="plate">
          <span class="name" title={i === hero ? undefined : s.name}>{i === hero ? "Du" : (s.name ?? "")}</span>
          <span class="line">
            {#if s.position}<b>{s.position}</b>{/if}
            {#if s.stack !== undefined}<span class="stack num">{format ? format(s.stack) : bb(s.stack)}</span>{/if}
          </span>
          <span class="act">
            {#if s.won !== undefined}
              <span class="won num" title="Was dieser Spieler aus dem Pot bekommt">+{amount(s.won)}</span>
            {:else if s.action}
              <span class="tag" class:graded={!!s.grade} title={s.grade ? GRADE[s.grade].label : undefined}
                ><ActionLabel {...s.action} amount={undefined} size="sm" />{#if s.grade}<GradeIcon grade={s.grade} size={13} />{/if}</span
              >
            {/if}
            {#if s.dealer}<span class="dealer" title="Dealer-Button">D</span>{/if}
          </span>
        </div>
      {:else}
        <div class="plate free">frei</div>
      {/if}
    </div>
  {/each}
</div>
</div>

<style>
  .ui-table {
    position: relative;
    width: 100%;
  }
  .stage {
    position: absolute;
    left: 0;
    top: 0;
    transform-origin: 0 0;
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
    color: var(--text-2);
    font-size: var(--fs-xs);
    font-weight: var(--fw-semibold);
    white-space: nowrap;
  }
  .note .aside {
    margin-left: 0.4em;
    color: var(--text-2);
  }
  .note.win {
    padding: 2px var(--sp-2);
    border-radius: var(--r-pill);
    background: var(--grid-plate);
    color: var(--pos);
    font-size: var(--fs-sm);
    animation: rise var(--dur-slow) var(--ease-enter) both;
  }
  @keyframes rise {
    from {
      opacity: 0;
      translate: 0 4px;
    }
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
  .bet {
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
    width: 100%;
    padding: 4px;
    border-radius: var(--r-md);
    border: 1px solid var(--line-2);
    background: var(--surface-2);
    box-shadow: var(--shadow-1);
  }
  /* The name has the plate's full width; the position goes beside the stack. */
  .name {
    max-width: 100%;
    overflow: hidden;
    font-size: var(--fs-md);
    line-height: 1.1;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .line {
    display: flex;
    gap: 4px;
    align-items: baseline;
    line-height: 1.1;
    white-space: nowrap;
  }
  .line b {
    color: var(--text);
    font-size: var(--fs-sm);
    font-weight: var(--fw-semibold);
  }
  .stack {
    color: var(--text);
    font-size: var(--fs-md);
    font-weight: var(--fw-semibold);
    line-height: 1.1;
  }
  /* Always there, so a plate keeps its height whether or not the seat acted. */
  .act {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    align-self: stretch;
    height: 18px;
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
    opacity: 0.72;
    transition: opacity var(--dur) var(--ease-standard);
  }
  .free {
    min-width: 64px;
    border-style: dashed;
    background: transparent;
    color: var(--text-2);
    font-size: var(--fs-sm);
  }
  .dealer {
    position: absolute;
    right: 0;
    display: grid;
    place-items: center;
    width: 16px;
    height: 16px;
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
    z-index: 3;
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
    position: relative;
    display: inline-flex;
  }
  /* The grade sits on the tag's corner, so a graded tag is as wide as everyone else's. */
  .graded > :global(.ui-grade-icon) {
    position: absolute;
    top: -7px;
    right: -7px;
    border-radius: 50%;
    background: var(--bg);
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
  .bet {
    animation: glide var(--dur-slow) var(--ease-enter) 60ms both;
  }
  .tag,
  .won {
    animation: rise var(--dur-slow) var(--ease-enter) both;
  }
  @keyframes glide {
    from {
      opacity: 0;
      transform: translate(calc(-50% + var(--dx)), calc(-50% + var(--dy))) scale(0.8);
    }
  }
</style>
