<script lang="ts">
  import PlayingCard from "../PlayingCard.svelte";
  import { money, moneyShort } from "../stats";
  import { RING, other, plateSide } from "./geometry";
  import type { Point, TableView } from "../types";
  import { draggable } from "./drag";

  interface Props {
    view: TableView;
    /** Avatar centre per visual seat, in px. */
    seatAt: (visual: number) => Point;
    /** Saved top-left of a seat's cards, if the user moved them. */
    cardsAt: (visual: number) => Point | null;
    /** Saved top-left of the board panel, if moved. */
    board: Point | null;
    width: number;
    height: number;
    scale: number;
    /** Editor: cards and board can be dragged. */
    editable?: boolean;
    onCardsMove?: (visual: number, p: Point, done: boolean) => void;
    onBoardMove?: (p: Point, done: boolean) => void;
  }

  let { view, seatAt, cardsAt, board, width, height, scale: s, editable = false, onCardsMove, onBoardMove }: Props = $props();
  const dockAt = $derived(board ?? { x: 24 * s, y: height * 0.8 });

  const recap = $derived(view.recap);
  const winners = $derived(new Map(recap.winners));
  const bounties = $derived(new Map(recap.bounties));
  const nameOf = (visual: number) => view.seats.find((x) => x.visual === visual)?.name ?? "";
  const amount = (a: number) => (view.game === "tournament" ? money(a, null, "tournament") : money(a, view.currency));
  const winnerText = $derived(recap.winners.map(([v, a]) => `${nameOf(v)} gewinnt ${amount(a)}`).join(" · "));
  let holes: Record<number, HTMLElement> = {};
  /** Current top-left of a seat's cards, measured, so a drag starts where they are. */
  function topLeft(visual: number, p: Point): Point {
    const saved = cardsAt(visual);
    if (saved) return saved;
    const el = holes[visual];
    if (!el) return p;
    const r = el.getBoundingClientRect();
    const parent = el.offsetParent!.getBoundingClientRect();
    return { x: r.left - parent.left, y: r.top - parent.top };
  }

  /** Cards go beside the ring on the side away from the client's name plate, unless moved. */
  function holeStyle(visual: number, p: { x: number; y: number }) {
    const saved = cardsAt(visual);
    if (saved) return `left:${saved.x}px; top:${saved.y}px`;
    const free = other(plateSide(p.x, p.y, width, height));
    const gap = (RING.size / 2 + 3) * s;
    return free === "right"
      ? `left:${p.x + gap}px; top:${p.y}px; transform:translate(0,-50%)`
      : `left:${p.x - gap}px; top:${p.y}px; transform:translate(-100%,-50%)`;
  }
</script>

<div class="recap" style="--s:{s}" aria-live="polite">
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="dock"
    class:edit={editable}
    style="left:{dockAt.x}px; top:{dockAt.y}px"
    use:draggable={{ origin: () => dockAt, onMove: (p, done) => onBoardMove?.(p, done) }}
  >
    <span class="label">Letzte Hand</span>
    {#if recap.board.length}
      <div class="board">
        {#each recap.board as c (c)}<PlayingCard card={c} size={Math.round(30 * s)} />{/each}
      </div>
    {:else}
      <div class="noboard">vor dem Flop entschieden</div>
    {/if}
    {#if winnerText}<div class="result">{winnerText}</div>{/if}
  </div>

  {#each recap.shown as [visual, cards] (visual)}
    {@const p = seatAt(visual)}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="hole"
      class:edit={editable}
      class:won={winners.has(visual)}
      bind:this={holes[visual]}
      style={holeStyle(visual, p)}
      use:draggable={{ origin: () => topLeft(visual, p), onMove: (q, done) => onCardsMove?.(visual, q, done) }}
    >
      {#each cards as c (c)}<PlayingCard card={c} size={Math.round(26 * s)} />{/each}
      {#if bounties.has(visual)}
        <span class="ko num">KO +{moneyShort(bounties.get(visual) ?? 0, view.bounty_currency ?? "USD")}</span>
      {/if}
    </div>
  {/each}
</div>

<style>
  .edit {
    pointer-events: auto;
    cursor: grab;
    outline: 1px dashed rgba(236, 235, 231, 0.35);
    outline-offset: 2px;
  }
  .recap {
    position: absolute;
    inset: 0;
    pointer-events: none;
    animation: show 0.18s ease-out;
  }
  @keyframes show {
    from {
      opacity: 0;
      transform: translateY(calc(4px * var(--s)));
    }
  }
  .dock {
    position: absolute;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: calc(5px * var(--s));
    padding: calc(7px * var(--s)) calc(10px * var(--s)) calc(8px * var(--s));
    border-radius: calc(10px * var(--s));
    background: #14161a;
    box-shadow:
      0 0 0 1px rgba(255, 255, 255, 0.09) inset,
      0 calc(8px * var(--s)) calc(24px * var(--s)) rgba(0, 0, 0, 0.5);
  }
  .label {
    font-family: var(--font-num);
    font-size: calc(9.5px * var(--s));
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: #8d9097;
  }
  .board {
    display: flex;
    gap: calc(4px * var(--s));
  }
  .noboard {
    font-size: calc(12px * var(--s));
    color: #b9bbc0;
  }
  .result {
    font-size: calc(13px * var(--s));
    font-weight: 600;
    color: var(--pos);
  }
  .hole {
    position: absolute;
    display: flex;
    align-items: center;
    gap: calc(3px * var(--s));
    padding: calc(4px * var(--s));
    border-radius: calc(7px * var(--s));
    background: #14161a;
    box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.09) inset;
  }
  .hole.won {
    box-shadow:
      0 0 0 calc(2px * var(--s)) var(--pos) inset,
      0 0 calc(14px * var(--s)) rgba(120, 193, 66, 0.35);
  }
  .ko {
    margin-left: calc(3px * var(--s));
    padding: 0 calc(4px * var(--s));
    border-radius: calc(3px * var(--s));
    background: #3a3f47;
    color: #ecebe7;
    font-size: calc(11px * var(--s));
    white-space: nowrap;
  }
</style>
