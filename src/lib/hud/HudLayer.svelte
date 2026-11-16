<script lang="ts">
  import { api } from "../api";
  import type { TableView } from "../types";
  import SeatHud from "./SeatHud.svelte";

  interface Props {
    view: TableView;
    width: number;
    height: number;
    onDragging?: (dragging: boolean) => void;
  }

  let { view, width, height, onDragging }: Props = $props();

  let draft = $state<Record<number, { x: number; y: number }>>({});
  let openSeat = $state<number | null>(null);

  const scale = $derived(view.hud.scale * Math.min(Math.max(width / 960, 0.5), 1.7));

  function pos(visual: number) {
    const d = draft[visual];
    if (d) return d;
    const p = view.layout[visual] ?? { x: 0.5, y: 0.5 };
    return { x: p.x * width, y: p.y * height };
  }

  function drag(visual: number, x: number, y: number, done: boolean) {
    draft[visual] = { x, y };
    onDragging?.(!done);
    if (done) {
      const normalised = { x: x / width, y: y / height };
      view.layout[visual] = normalised;
      delete draft[visual];
      api.saveSeat(view.max_seats, visual, normalised);
    }
  }
</script>

{#each view.seats as seat (seat.seat)}
  {@const p = pos(seat.visual)}
  <SeatHud
    {seat}
    hud={view.hud}
    x={p.x}
    y={p.y}
    {scale}
    cardLeft={p.x > width * 0.62}
    game={view.game}
    currency={view.currency}
    open={openSeat === seat.seat}
    onToggle={() => (openSeat = openSeat === seat.seat ? null : seat.seat)}
    onDrag={(x, y, done) => drag(seat.visual, x, y, done)}
  />
{/each}
