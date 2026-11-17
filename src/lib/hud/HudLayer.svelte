<script lang="ts">
  import { api } from "../api";
  import type { Point, SeatPos, TableView } from "../types";
  import ClassicBox from "./ClassicBox.svelte";
  import DetailPopup from "./DetailPopup.svelte";
  import GraphicSeat from "./GraphicSeat.svelte";
  import Recap from "./Recap.svelte";
  import { plateSide, stageScale } from "./geometry";

  interface Props {
    view: TableView;
    width: number;
    height: number;
    showRecap?: boolean;
    /** Editor without a screenshot: draw stand-in avatars and name the classic boxes. */
    preview?: boolean;
    onDragging?: (dragging: boolean) => void;
    /** Called after a position was saved, with the new layout. */
    onLayoutChange?: (layout: SeatPos[]) => void;
  }

  let { view, width, height, showRecap = false, preview = false, onDragging, onLayoutChange }: Props = $props();

  type Part = "ring" | "card" | "plate";
  /** Positions in px while something is being dragged. */
  let draft = $state<Record<string, Point>>({});
  let openSeat = $state<number | null>(null);

  const s = $derived(view.hud.scale * stageScale(width, height));

  const seatPos = (visual: number): SeatPos => view.layout[visual] ?? { x: 0.5, y: 0.5 };
  const toPx = (p: Point): Point => ({ x: p.x * width, y: p.y * height });

  function ring(visual: number): Point {
    return draft[`${visual}:ring`] ?? toPx(seatPos(visual));
  }
  function part(visual: number, which: "card" | "plate"): Point | null {
    const saved = seatPos(visual)[which];
    return draft[`${visual}:${which}`] ?? (saved ? toPx(saved) : null);
  }
  /** Side of the client's name plate, from the seat's default (not dragged) position. */
  function side(visual: number) {
    const p = toPx(seatPos(visual));
    return plateSide(p.x, p.y, width, height);
  }

  function move(visual: number, which: Part, p: Point, done: boolean) {
    draft[`${visual}:${which}`] = p;
    onDragging?.(!done);
    if (!done) return;
    delete draft[`${visual}:${which}`];
    const n = { x: p.x / width, y: p.y / height };
    save(visual, which === "ring" ? { ...seatPos(visual), ...n } : { ...seatPos(visual), [which]: n });
  }

  function reset(visual: number, which: "card" | "plate") {
    save(visual, { ...seatPos(visual), [which]: null });
  }

  function save(visual: number, pos: SeatPos) {
    view.layout[visual] = pos;
    api.saveSeat(view.layout_key, visual, $state.snapshot(pos));
    onLayoutChange?.($state.snapshot(view.layout));
  }

  function tap(seat: number) {
    openSeat = openSeat === seat ? null : seat;
  }

  const openView = $derived(view.seats.find((x) => x.seat === openSeat));
</script>

{#if preview}
  {#each view.seats as seat (seat.seat)}
    {@const r = ring(seat.visual)}
    <div class="avatar" style="left:{r.x}px; top:{r.y}px; width:{66 * s}px; height:{66 * s}px; font-size:{22 * s}px">
      {seat.name.slice(0, 1)}
    </div>
  {/each}
{/if}

{#each view.seats as seat (seat.seat)}
  {@const r = ring(seat.visual)}
  {#if view.hud.mode === "graphic"}
    <GraphicSeat
      {seat}
      hud={view.hud}
      ring={r}
      plate={part(seat.visual, "plate")}
      scale={s}
      side={side(seat.visual)}
      game={view.game}
      currency={view.currency}
      bountyCurrency={view.bounty_currency}
      onRingMove={(p, done) => move(seat.visual, "ring", p, done)}
      onPlateMove={(p, done) => move(seat.visual, "plate", p, done)}
      onPlateReset={() => reset(seat.visual, "plate")}
      onTap={() => tap(seat.seat)}
    />
  {:else}
    <ClassicBox
      {seat}
      hud={view.hud}
      ring={r}
      box={part(seat.visual, "card")}
      showName={preview}
      scale={s}
      {width}
      {height}
      game={view.game}
      currency={view.currency}
      bountyCurrency={view.bounty_currency}
      onMove={(p, done) => move(seat.visual, "card", p, done)}
      onReset={() => reset(seat.visual, "card")}
      onTap={() => tap(seat.seat)}
    />
  {/if}
{/each}

{#if showRecap}
  <Recap {view} seatAt={ring} {width} {height} scale={s} />
{/if}

{#if openView}
  {@const r = ring(openView.visual)}
  <DetailPopup seat={openView} hud={view.hud} at={{ x: r.x + (r.x > width / 2 ? -40 * s : 40 * s), y: r.y + 30 * s }} flip={r.x > width / 2} />
{/if}

<style>
  .avatar {
    position: absolute;
    transform: translate(-50%, -50%);
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: radial-gradient(circle at 35% 30%, #3a404b, #1f2329);
    color: rgba(236, 235, 231, 0.4);
    font-family: var(--font-num);
    pointer-events: none;
  }
</style>
