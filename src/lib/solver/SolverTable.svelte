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
  import Felt from "../Felt.svelte";
  import { REFERENCE_ASPECT } from "../hud/geometry";
  import PlayingCard from "../PlayingCard.svelte";
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
    street = "",
  }: {
    players: number;
    positions: { oop: string; ip: string };
    /** Position drawn at the bottom; the button by default. */
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
    street?: string;
  } = $props();

  const order = $derived(positionsFor(players));
  const bottom = $derived(Math.max(0, order.indexOf(anchor ?? "BTN")));
  const CENTER = { x: 0.5, y: 0.44 };

  function place(i: number) {
    const n = order.length;
    const visual = (((i - bottom) % n) + n) % n;
    const a = Math.PI / 2 + (visual * 2 * Math.PI) / n;
    const x = CENTER.x + 0.41 * Math.cos(a);
    const y = CENTER.y + 0.335 * Math.sin(a);
    // The button sits a little round the table from the seat, clear of its chips.
    const d = a + 0.4;
    return { x, y, bx: x + (CENTER.x - x) * 0.46, by: y + (CENTER.y - y) * 0.5, dx: CENTER.x + 0.27 * Math.cos(d), dy: CENTER.y + 0.22 * Math.sin(d) };
  }

  const side = (pos: string): SolverPlayer | null => (pos === positions.oop ? "oop" : pos === positions.ip ? "ip" : null);
  const idx = (p: SolverPlayer) => (p === "oop" ? 0 : 1);

  let w = $state(860);
  let h = $state(600);
  const k = $derived(Math.min(1.1, Math.max(0.78, w / 880)));

  const reduced = typeof matchMedia !== "undefined" && matchMedia("(prefers-reduced-motion: reduce)").matches;

  /** Chips slide in from the seat and out into the pot; positions are fractions of the stage. */
  function slide(_: Element, { from, to }: { from: { x: number; y: number }; to: { x: number; y: number } }) {
    const dx = (from.x - to.x) * w;
    const dy = (from.y - to.y) * h;
    return {
      duration: reduced ? 0 : 320,
      css: (t: number) => {
        const e = 1 - (1 - t) ** 3;
        return `transform: translate(-50%, -50%) translate(${dx * (1 - e)}px, ${dy * (1 - e)}px) scale(${k}); opacity: ${Math.min(1, t * 3)}`;
      },
    };
  }

  const SLOT = ["Flop", "Flop", "Flop", "Turn", "River"];
  const middle = $derived(pot - bets[0] - bets[1]);
</script>

<div class="stage" bind:clientWidth={w} bind:clientHeight={h} style="--k:{k}; --ar:{REFERENCE_ASPECT}">
  <Felt>
    <div class="center">
      <div class="board" role="group" aria-label="Board">
        {#each SLOT as name, i (i)}
          {@const card = board[i]}
          {#if slotsClickable || (i === nextSlot && onSlot)}
            <button
              class="slot"
              class:filled={!!card}
              class:optional={i >= 3 && !card}
              class:next={i === nextSlot}
              onclick={(e) => onSlot?.(i, e.currentTarget)}
              aria-label={card ? `${name}: ${card} ändern` : `${name}-Karte wählen`}
            >
              {#if card}<PlayingCard {card} size={42} />{:else}<span>{name}</span>{/if}
            </button>
          {:else if card}
            <span class="slot filled"><PlayingCard {card} size={42} /></span>
          {:else}
            <span class="slot ghost" class:next={i === nextSlot}></span>
          {/if}
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
        <span class="dealer" style="left:{p.dx * 100}%; top:{p.dy * 100}%" title="Dealer-Button">D</span>
      {/if}
      <svelte:element
        this={seatsClickable ? "button" : "div"}
        class="seat"
        class:in={!!s}
        class:out={!s}
        class:acting={!!s && acting === s}
        class:pickable={seatsClickable}
        style="left:{p.x * 100}%; top:{p.y * 100}%"
        onclick={seatsClickable ? () => onSeat?.(pos) : undefined}
        aria-pressed={seatsClickable ? !!s : undefined}
        aria-label={seatsClickable ? `${pos}${s ? ` (${s === "oop" ? "OOP" : "IP"}, in der Hand)` : ""}` : undefined}
        role={seatsClickable ? undefined : "group"}
      >
        {#if s}
          <div class="cards">
            {#if view?.cards === "hidden" || !view?.cards}
              <PlayingCard hidden size={30} /><PlayingCard hidden size={30} />
            {:else}
              {#each view.cards as c (c)}<PlayingCard card={c} size={30} />{/each}
            {/if}
          </div>
        {/if}
        <div class="pod">
          {#if s && acting === s}<span class="turn">am Zug</span>{/if}
          <div class="who">
            <span class="pos">{pos}</span>
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
        </div>
      </svelte:element>
    {/each}
  </Felt>
</div>

<style>
  .stage {
    position: relative;
    /* Short windows keep the actions below the table in view. */
    width: min(100%, calc(52vh * var(--ar)));
    margin: 0 auto;
    aspect-ratio: var(--ar);
    border-radius: var(--r-lg);
    overflow: hidden;
    border: 1px solid var(--line);
  }
  .center {
    position: absolute;
    left: 50%;
    top: 44%;
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
  .slot {
    width: 42px;
    height: 59px;
    padding: 0;
    border-radius: 6px;
    display: grid;
    place-items: center;
    border: 0;
    background: none;
    line-height: 0;
  }
  button.slot {
    cursor: pointer;
  }
  .slot:not(.filled) {
    border: 1.5px dashed rgba(236, 235, 231, 0.28);
    background: rgba(0, 0, 0, 0.18);
    color: rgba(236, 235, 231, 0.55);
    font-size: 10.5px;
    font-weight: 600;
    line-height: 1;
  }
  .slot.optional {
    border-color: rgba(236, 235, 231, 0.14);
    color: rgba(236, 235, 231, 0.32);
  }
  .slot.ghost {
    border-color: rgba(236, 235, 231, 0.08);
    background: rgba(0, 0, 0, 0.1);
  }
  button.slot:not(.filled):hover {
    border-color: var(--text);
    color: var(--text);
  }
  button.slot.filled:hover {
    transform: translateY(-2px);
  }
  .slot.next {
    border: 1.5px solid var(--text);
    color: var(--text);
    background: rgba(236, 235, 231, 0.08);
  }
  .pot {
    display: flex;
    align-items: baseline;
    gap: 7px;
    padding: 3px 12px;
    border-radius: 99px;
    background: rgba(0, 0, 0, 0.42);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.06);
    font-size: 12px;
    color: var(--text-2);
  }
  .pot b {
    font-size: 15px;
    font-weight: 600;
    color: var(--text);
  }
  .total {
    font-size: 11.5px;
    color: var(--text-3);
  }
  .street {
    font-size: 11.5px;
    font-weight: 600;
    color: rgba(236, 235, 231, 0.4);
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
  .seat.pickable {
    cursor: pointer;
  }
  .seat.out {
    transform: translate(-50%, -50%) scale(calc(var(--k) * 0.82));
    opacity: 0.5;
  }
  .seat.out.pickable:hover {
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
      0 6px 16px rgba(0, 0, 0, 0.45);
    transition: box-shadow 0.15s;
  }
  .out .pod {
    min-width: 76px;
    padding: 7px 10px 6px;
    background: rgba(40, 43, 49, 0.7);
  }
  .seat.pickable:hover .pod {
    box-shadow:
      inset 0 0 0 1.5px var(--text-2),
      0 6px 16px rgba(0, 0, 0, 0.45);
  }
  .seat.pickable:focus-visible {
    outline: none;
  }
  .seat.pickable:focus-visible .pod {
    outline: 2px solid var(--focus);
    outline-offset: 2px;
  }
  .acting .pod {
    box-shadow:
      inset 0 0 0 2px var(--text),
      0 0 0 4px rgba(236, 235, 231, 0.08),
      0 6px 16px rgba(0, 0, 0, 0.45);
  }
  .turn {
    position: absolute;
    right: -10px;
    top: -9px;
    padding: 0 7px;
    border-radius: 99px;
    background: var(--text);
    color: var(--bg);
    font-size: 10px;
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
  .pos {
    font-family: var(--font-num);
    font-size: 14px;
    font-weight: 700;
    letter-spacing: 0.02em;
    color: var(--text);
  }
  .out .pos {
    font-size: 12px;
    color: var(--text-2);
  }
  .role {
    font-size: 10px;
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
    font-size: 15px;
    font-weight: 700;
    color: #fff;
  }
  .fold {
    font-size: 10.5px;
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
    background: #1d2a1a;
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
    font-size: 10.5px;
    font-weight: 700;
    display: grid;
    place-items: center;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.5);
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
    background: rgba(0, 0, 0, 0.5);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.06);
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
      inset 0 0 0 2px #8f9198,
      inset 0 0 0 4px var(--text);
  }
</style>
