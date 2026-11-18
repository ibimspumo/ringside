<script lang="ts" module>
  export interface SeatView {
    /** Face-up cards, "hidden" for two backs. */
    cards?: string[] | "hidden";
    /** Short note under the plate, e.g. the last action. */
    tag?: string;
    tone?: "allin" | "win" | "lose";
  }
</script>

<script lang="ts">
  import { REFERENCE_ASPECT } from "../hud/geometry";
  import PlayingCard from "../PlayingCard.svelte";
  import CardSlot from "../ui/CardSlot.svelte";
  import OvalFelt from "../table/OvalFelt.svelte";
  import { clear, layout, placeOf } from "../table/seats";
  import { bb } from "./actions";
  import { GLOSSARY } from "./explain";
  import { positionsFor } from "./scenario";
  import type { SolverPlayer } from "./types";

  /**
   * The table of a heads-up spot: every seat of the table, the two in the hand at their
   * positions, chips in front, pot and board. Seats and board slots can be made clickable.
   */
  let {
    players,
    positions,
    anchor = null,
    board,
    slotsClickable = false,
    nextSlot = -1,
    onSlot,
    pot,
    bets = [0, 0],
    stacks,
    acting = null,
    views = { oop: {}, ip: {} },
    seatsClickable = false,
    onSeat,
    pickCards = null,
    onCards,
    street = "",
  }: {
    players: number;
    positions: { oop: string; ip: string };
    /** Your position, drawn at the bottom in the middle (see `table/seats.ts`). */
    anchor?: string | null;
    board: string[];
    slotsClickable?: boolean;
    /** Slot waiting for the next card, highlighted. */
    nextSlot?: number;
    onSlot?: (i: number, el: HTMLElement) => void;
    pot: number;
    bets?: [number, number];
    stacks: [number, number];
    acting?: SolverPlayer | null;
    views?: { oop: SeatView; ip: SeatView };
    seatsClickable?: boolean;
    onSeat?: (position: string) => void;
    /** The player whose cards are picked at the table, shown as two slots while empty. */
    pickCards?: SolverPlayer | null;
    onCards?: (card: number, el: HTMLElement) => void;
    street?: string;
  } = $props();

  const order = $derived(positionsFor(players));
  const bottom = $derived(Math.max(0, order.indexOf(anchor ?? positions.oop)));

  const side = (pos: string): SolverPlayer | null => (pos === positions.oop ? "oop" : pos === positions.ip ? "ip" : null);
  const idx = (p: SolverPlayer) => (p === "oop" ? 0 : 1);

  let w = $state(860);
  let h = $state(600);
  const k = $derived(Math.min(1.1, Math.max(0.78, w / 880)));
  // Pods of players in the hand are larger than the empty seats; the layout leaves room for
  // both. Where chips would touch the board, everything on the table gets a little smaller.
  const inHand = (place: number) => {
    const pos = order[(place + bottom) % order.length];
    return pos === positions.oop || pos === positions.ip;
  };
  const sizesAt = (kk: number) => ({
    seat: { w: 112 * kk, h: 92 * kk },
    boxOf: (place: number) => (inHand(place) ? { w: 112 * kk, h: 92 * kk } : { w: 84 * kk, h: 46 * kk }),
    badge: { w: 1, h: 1 },
    bet: { w: 84 * kk, h: 24 * kk },
    dealer: 18 * kk,
    gap: 3 * kk,
  });
  const middleAt = (kk: number) => [{ box: { w: 240 * kk, h: 112 * kk }, dy: 0 }];
  const fitted = $derived.by(() => {
    const n = order.length;
    const dealer = placeOf(Math.max(0, order.indexOf("BTN")), bottom, n);
    for (let kk = k; kk > 0.55; kk -= 0.02) {
      const g = layout(n, w, h, sizesAt(kk));
      if (clear(g, sizesAt(kk), middleAt(kk), dealer)) return { geo: g, kk };
    }
    return { geo: layout(n, w, h, sizesAt(0.55)), kk: 0.55 };
  });
  const geo = $derived(fitted.geo);
  const kk = $derived(fitted.kk);
  /** Seat `i` of `order` in fractions of the stage, with its chips and button. */
  function place(i: number) {
    const p = geo.places[placeOf(i, bottom, order.length)];
    return { x: p.seat.x / w, y: p.seat.y / h, bx: p.bet.x / w, by: p.bet.y / h, dx: p.dealer.x / w, dy: p.dealer.y / h };
  }
  const CENTER = $derived({ x: geo.center.x / w, y: geo.center.y / h });

  const reduced = typeof matchMedia !== "undefined" && matchMedia("(prefers-reduced-motion: reduce)").matches;

  /** Chips slide in from the seat and out into the pot; positions are fractions of the stage. */
  function slide(_: Element, { from, to }: { from: { x: number; y: number }; to: { x: number; y: number } }) {
    const dx = (from.x - to.x) * w;
    const dy = (from.y - to.y) * h;
    return {
      duration: reduced ? 0 : 320,
      css: (t: number) => {
        const e = 1 - (1 - t) ** 3;
        return `transform: translate(-50%, -50%) translate(${dx * (1 - e)}px, ${dy * (1 - e)}px) scale(${kk}); opacity: ${Math.min(1, t * 3)}`;
      },
    };
  }

  const SLOT = ["Flop", "Flop", "Flop", "Turn", "River"];
  const middle = $derived(pot - bets[0] - bets[1]);
</script>

<div class="stage" bind:clientWidth={w} bind:clientHeight={h} style="--k:{kk}; --ar:{REFERENCE_ASPECT}; --cx:{CENTER.x * 100}%; --cy:{CENTER.y * 100}%">
  <div class="felt">
    <OvalFelt felt={geo.felt} />
    <div class="center" data-part="middle">
      <div class="board" role="group" aria-label="Board">
        {#each SLOT as name, i (i)}
          {@const card = board[i] ?? null}
          {@const pickable = slotsClickable || (i === nextSlot && !!onSlot)}
          <CardSlot
            {card}
            size={42}
            label={pickable ? name : ""}
            tone={i === nextSlot ? "next" : i >= 3 ? "optional" : "idle"}
            onpick={pickable ? (el) => onSlot?.(i, el) : undefined}
            ariaLabel={card ? `${name}: ${card} ändern` : `${name}-Karte wählen`}
          />
        {/each}
      </div>
      <div class="pot">
        <span>Pot</span>
        <b class="num">{bb(middle, 2)} BB</b>
        {#if bets[0] + bets[1] > 0}<span class="total num">gesamt {bb(pot, 2)}</span>{/if}
      </div>
      {#if street}<div class="street">{street}</div>{/if}
    </div>

    {#each order as pos, i (pos)}
      {@const p = place(i)}
      {@const s = side(pos)}
      {#if s && bets[idx(s)] > 0}
        {#key `${street}-${s}-${bets[idx(s)]}`}
          <div
            class="bet num"
            data-part="bet"
            data-seat={i}
            style="left:{p.bx * 100}%; top:{p.by * 100}%"
            in:slide={{ from: { x: p.x, y: p.y }, to: { x: p.bx, y: p.by } }}
            out:slide={{ from: CENTER, to: { x: p.bx, y: p.by } }}
          >
            <i></i>{bb(bets[idx(s)], 2)}
          </div>
        {/key}
      {/if}
    {/each}

    {#each order as pos, i (pos)}
      {@const p = place(i)}
      {@const s = side(pos)}
      {@const view = s ? views[s] : null}
      {#if pos === "BTN"}
        <span class="dealer" data-part="dealer" style="left:{p.dx * 100}%; top:{p.dy * 100}%" title="Dealer-Button">D</span>
      {/if}
      <div
        class="seat"
        data-part="seat"
        data-seat={i}
        class:in={!!s}
        class:out={!s}
        class:acting={!!s && acting === s}
        class:pickable={seatsClickable}
        style="left:{p.x * 100}%; top:{p.y * 100}%"
        role="group"
        aria-label={pos}
      >
        {#if s}
          {@const face = view?.cards && view.cards !== "hidden" ? view.cards : null}
          {#if pickCards === s}
            <div class="cards" role="group" aria-label="Deine Hand">
              {#each [0, 1] as j (j)}
                <CardSlot
                  card={face?.[j] ?? null}
                  size={30}
                  label="?"
                  onpick={(el) => onCards?.(j, el)}
                  ariaLabel={face ? `Deine Hand ${face.join(" ")} ändern` : "Deine Hand wählen (optional)"}
                />
              {/each}
            </div>
          {:else}
            <div class="cards">
              {#if face}
                {#each face as c (c)}<PlayingCard card={c} size={30} />{/each}
              {:else}
                <PlayingCard hidden size={30} /><PlayingCard hidden size={30} />
              {/if}
            </div>
          {/if}
        {/if}
        {#snippet plate()}
          {#if s && acting === s}<span class="turn">am Zug</span>{/if}
          <div class="who">
            <span class="posname">{pos}</span>
            {#if s}<span class="role" title={GLOSSARY[s]}>{s === "oop" ? "OOP" : "IP"}</span>{/if}
          </div>
          {#if s}
            <div class="num stack"><b>{bb(stacks[idx(s)], 2)}</b> <span>BB</span></div>
          {:else}
            <div class="fold">Fold</div>
          {/if}
          {#if view?.tag}
            <span class="tag" class:allin={view.tone === "allin"} class:win={view.tone === "win"} class:lose={view.tone === "lose"}>{view.tag}</span>
          {/if}
        {/snippet}
        {#if seatsClickable}
          <button class="seatbtn" onclick={() => onSeat?.(pos)} aria-pressed={!!s} aria-label="{pos}{s ? ` (${s === 'oop' ? 'OOP' : 'IP'}, in der Hand)` : ''}"><div class="pod">{@render plate()}</div></button>
        {:else}
          <div class="pod">{@render plate()}</div>
        {/if}
      </div>
    {/each}
  </div>
</div>

<style>
  .stage {
    position: relative;
    /* Short windows keep the actions below the table in view. */
    width: min(100%, calc(var(--stage-vh, 52vh) * var(--ar)));
    margin: 0 auto;
    aspect-ratio: var(--ar);
    overflow: hidden;
  }
  .felt {
    position: absolute;
    inset: 0;
    overflow: hidden;
    background: var(--bg);
  }
  .center {
    position: absolute;
    left: var(--cx);
    top: var(--cy);
    transform: translate(-50%, -50%) scale(var(--k));
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 7px;
    z-index: 1;
  }
  .board {
    display: flex;
    gap: 5px;
  }








  .pot {
    display: flex;
    align-items: baseline;
    gap: 7px;
    padding: 3px 12px;
    border-radius: 99px;
    background: color-mix(in srgb, var(--bg) 42%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--text) 6%, transparent);
    font-size: 12px;
    color: var(--text-2);
  }
  .pot b {
    font-size: var(--fs-lg);
    font-weight: 600;
    color: var(--text);
  }
  .total {
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .street {
    font-size: var(--fs-xs);
    font-weight: 600;
    color: color-mix(in srgb, var(--text) 40%, transparent);
  }

  .seat {
    position: absolute;
    z-index: 2;
    transform: translate(-50%, -50%) scale(var(--k));
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 0;
    border: 0;
    background: none;
    text-align: center;
  }
  .seatbtn {
    all: unset;
    display: block;
    cursor: pointer;
  }
  .seat.out {
    transform: translate(-50%, -50%) scale(calc(var(--k) * 0.82));
    opacity: 0.5;
  }
  .seat.out.pickable:has(.seatbtn:hover) {
    opacity: 0.9;
  }
  .cards {
    display: flex;
    gap: 3px;
    height: 42px;
    margin-bottom: -10px;
    z-index: 1;
  }





  .pod {
    position: relative;
    min-width: 104px;
    padding: 12px 12px 9px;
    border-radius: 8px;
    background: var(--plate);
    box-shadow:
      inset 0 0 0 1px var(--plate-edge),
      var(--shadow-2);
    transition: box-shadow var(--dur-fast) var(--ease-standard);
  }
  .out .pod {
    min-width: 76px;
    padding: 7px 10px 6px;
    background: color-mix(in srgb, var(--plate) 70%, transparent);
  }
  .seatbtn:hover .pod {
    box-shadow:
      inset 0 0 0 1.5px var(--text-2),
      var(--shadow-2);
  }
  .seatbtn:focus-visible .pod {
    outline: 2px solid var(--focus);
    outline-offset: 2px;
  }
  .acting .pod {
    box-shadow:
      inset 0 0 0 2px var(--text),
      0 0 0 4px color-mix(in srgb, var(--text) 8%, transparent),
      var(--shadow-2);
  }
  .turn {
    position: absolute;
    right: 0;
    top: 50%;
    transform: translate(70%, -50%);
    padding: 0 7px;
    border-radius: 99px;
    background: var(--text);
    color: var(--bg);
    font-size: var(--fs-xs);
    font-weight: 700;
    line-height: 16px;
    white-space: nowrap;
    z-index: 2;
  }
  .who {
    display: flex;
    align-items: baseline;
    justify-content: center;
    gap: 6px;
  }
  .posname {
    font-family: var(--font-num);
    font-size: 14px;
    font-weight: 700;
    letter-spacing: 0.02em;
    color: var(--text);
  }
  .out .posname {
    font-size: 12px;
    color: var(--text-2);
  }
  .role {
    font-size: var(--fs-xs);
    font-weight: 600;
    color: var(--text-3);
  }
  .stack {
    margin-top: 1px;
    font-size: 12px;
    color: var(--text-2);
    white-space: nowrap;
  }
  .stack b {
    font-size: var(--fs-lg);
    font-weight: 700;
    color: var(--text);
  }
  .fold {
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .tag {
    position: absolute;
    left: 50%;
    bottom: -10px;
    transform: translateX(-50%);
    padding: 0 8px;
    border-radius: 99px;
    background: var(--surface-3);
    border: 1px solid var(--line-2);
    color: var(--text);
    font-size: 11px;
    font-weight: 600;
    line-height: 17px;
    white-space: nowrap;
  }
  .tag.allin {
    background: var(--text);
    color: var(--bg);
    border-color: var(--text);
  }
  .tag.win {
    background: color-mix(in srgb, var(--pos) 14%, var(--surface));
    border-color: color-mix(in srgb, var(--pos) 55%, transparent);
    color: var(--pos);
  }
  .tag.lose {
    color: var(--text-2);
  }
  .dealer {
    position: absolute;
    z-index: 3;
    transform: translate(-50%, -50%) scale(var(--k));
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: var(--text);
    color: var(--bg);
    font-family: var(--font-num);
    font-size: var(--fs-xs);
    font-weight: 700;
    display: grid;
    place-items: center;
    box-shadow: 0 1px 3px color-mix(in srgb, var(--bg) 50%, transparent);
  }
  .bet {
    position: absolute;
    z-index: 1;
    left: 0;
    top: 0;
    transform: translate(-50%, -50%) scale(var(--k));
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 2px 9px 2px 3px;
    border-radius: 99px;
    background: color-mix(in srgb, var(--bg) 50%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--text) 6%, transparent);
    font-size: 13px;
    font-weight: 600;
    white-space: nowrap;
  }
  .bet i {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: var(--text);
    box-shadow:
      inset 0 0 0 2px var(--text-2),
      inset 0 0 0 4px var(--text);
  }
</style>
