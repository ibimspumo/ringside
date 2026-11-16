<script lang="ts">
  import { api } from "../api";
  import type { Point, SeatPos, TableView } from "../types";
  import SeatHud from "./SeatHud.svelte";

  interface Props {
    view: TableView;
    width: number;
    height: number;
    onDragging?: (dragging: boolean) => void;
  }

  let { view, width, height, onDragging }: Props = $props();

  /** Positions in px while something is being dragged. */
  let draftRing = $state<Record<number, Point>>({});
  let draftCard = $state<Record<number, Point>>({});
  let openSeat = $state<number | null>(null);

  const scale = $derived(view.hud.scale * Math.min(Math.max(width / 960, 0.5), 1.7));

  function seatPos(visual: number): SeatPos {
    return view.layout[visual] ?? { x: 0.5, y: 0.5 };
  }

  function ring(visual: number): Point {
    const p = seatPos(visual);
    return draftRing[visual] ?? { x: p.x * width, y: p.y * height };
  }

  function card(visual: number): Point | null {
    const c = seatPos(visual).card;
    return draftCard[visual] ?? (c ? { x: c.x * width, y: c.y * height } : null);
  }

  /**
   * The client draws the name plate beside the avatar, on the outer side for side seats and
   * on opposite sides for the two seats near the vertical centre line. The card goes on the
   * free side.
   */
  function cardLeft(x: number) {
    const rel = x / width;
    return rel > 0.5 !== Math.abs(rel - 0.5) < 0.1;
  }

  function save(visual: number, pos: SeatPos) {
    view.layout[visual] = pos;
    api.saveSeat(view.max_seats, visual, $state.snapshot(pos));
  }

  function dragRing(visual: number, p: Point, done: boolean) {
    draftRing[visual] = p;
    onDragging?.(!done);
    if (!done) return;
    delete draftRing[visual];
    save(visual, { ...seatPos(visual), x: p.x / width, y: p.y / height });
  }

  function dragCard(visual: number, p: Point, done: boolean) {
    draftCard[visual] = p;
    onDragging?.(!done);
    if (!done) return;
    delete draftCard[visual];
    save(visual, { ...seatPos(visual), card: { x: p.x / width, y: p.y / height } });
  }

  function resetCard(visual: number) {
    save(visual, { ...seatPos(visual), card: null });
  }
</script>

{#each view.seats as seat (seat.seat)}
  {@const p = ring(seat.visual)}
  <SeatHud
    {seat}
    hud={view.hud}
    x={p.x}
    y={p.y}
    card={card(seat.visual)}
    {scale}
    cardLeft={cardLeft(p.x)}
    game={view.game}
    currency={view.currency}
    open={openSeat === seat.seat}
    onToggle={() => (openSeat = openSeat === seat.seat ? null : seat.seat)}
    onRingDrag={(p, done) => dragRing(seat.visual, p, done)}
    onCardDrag={(p, done) => dragCard(seat.visual, p, done)}
    onCardReset={() => resetCard(seat.visual)}
  />
{/each}
