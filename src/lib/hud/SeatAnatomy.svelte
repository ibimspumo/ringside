<script lang="ts" module>
  export type Slot = { kind: "ring" | "plate"; i: number };
</script>

<script lang="ts">
  import { STAT_META } from "../stats";
  import type { GameKind, HudConfig, SeatView } from "../types";
  import GraphicSeat from "./GraphicSeat.svelte";
  import { PLATE, RING } from "./geometry";

  interface Props {
    seat: SeatView;
    hud: HudConfig;
    game: GameKind;
    currency: string | null;
    bountyCurrency: string | null;
    page: number;
    selected: Slot | null;
    onSelect: (slot: Slot) => void;
  }

  let { seat, hud, game, currency, bountyCurrency, page, selected, onSelect }: Props = $props();

  let w = $state(0);
  let box: HTMLDivElement | undefined = $state();
  /** Plate columns as rendered, relative to the box. */
  let cols = $state<{ x: number; y: number; w: number; h: number }[]>([]);

  const LABEL = 30;
  const cover = $derived(hud.graphic.cover_plate);
  /** Width of plate + ring in table pixels; the seat is scaled to fill the box. */
  const units = $derived(cover ? PLATE.width * (hud.graphic.plate_width ?? 1) : PLATE.width / 2 + PLATE.compact.width / 2 + RING.size / 2);
  /** Room right of the ring for the right-hand segment tags. */
  const RIGHT = 26;
  const s = $derived(w > 0 ? Math.min(1.45, (w - 8 - RIGHT) / units) : 1);
  const r = $derived((RING.size / 2) * s);
  const h = $derived(Math.round(RING.size * s + 2 * LABEL + 12 + (cover ? 0 : 30 * s)));
  const ring = $derived({ x: w - RIGHT - r, y: LABEL + 6 + r });

  // Quarter i runs clockwise from the left: top-left, top-right, bottom-right, bottom-left.
  const quarters = $derived(
    // Above and below the ring, so the tags never cover the plate beside it.
    [
      [-1, -1],
      [1, -1],
      [1, 1],
      [-1, 1],
    ].map(([sx, sy], i) => ({ i, x: ring.x + sx * r * 0.45, y: ring.y + sy * (r + 15) })),
  );
  const R = 50 - RING.stroke / 2;
  function arc(i: number) {
    const a0 = -180 + i * 90 + 4.5;
    const a1 = a0 + 81;
    const p = (deg: number) => `${50 + R * Math.cos((deg * Math.PI) / 180)} ${50 + R * Math.sin((deg * Math.PI) / 180)}`;
    return `M ${p(a0)} A ${R} ${R} 0 0 1 ${p(a1)}`;
  }

  $effect(() => {
    // Re-measure whenever the plate's look changes.
    void [hud.graphic.plate_pages, hud.graphic.cover_plate, hud.graphic.plate_width, hud.show_stack, page, w];
    if (!box) return;
    const el = box;
    const id = requestAnimationFrame(() => {
      const o = el.getBoundingClientRect();
      cols = [...el.querySelectorAll(".plate .col")].map((c) => {
        const b = c.getBoundingClientRect();
        return { x: b.left - o.left, y: b.top - o.top, w: b.width, h: b.height };
      });
    });
    return () => cancelAnimationFrame(id);
  });

  const pageStats = $derived(hud.graphic.plate_pages[page] ?? []);
  const on = (k: Slot["kind"], i: number) => selected?.kind === k && selected.i === i;
</script>

<div class="anatomy" bind:this={box} bind:clientWidth={w} style="height:{h}px">
  {#if w > 0}
    <div class="seat-preview" aria-hidden="true">
      <GraphicSeat
        {seat}
        {hud}
        {ring}
        scale={s}
        side="left"
        {game}
        {currency}
        {bountyCurrency}
        {page}
        onRingMove={() => {}}
        onNextPage={() => {}}
        onTap={() => {}}
      />
    </div>

    <svg class="arcs" style="left:{ring.x - r}px; top:{ring.y - r}px; width:{2 * r}px; height:{2 * r}px" viewBox="0 0 100 100" aria-hidden="true">
      {#each [0, 1, 2, 3] as i (i)}
        <path d={arc(i)} class:on={on("ring", i)} stroke-width={RING.stroke + 6} fill="none" />
      {/each}
    </svg>

    {#each quarters as q (q.i)}
      {@const id = hud.graphic.segments[q.i]}
      <button
        class="tag"
        class:on={on("ring", q.i)}
        class:left={q.x < ring.x}
        class:up={q.y < ring.y}
        style="left:{q.x}px; top:{q.y}px"
        title="Ring-Segment {q.i + 1}: {STAT_META[id].name}"
        onclick={() => onSelect({ kind: "ring", i: q.i })}
      >
        <span class="num">{STAT_META[id].short}</span>
      </button>
    {/each}

    {#each cols as c, i (i)}
      <button
        class="col-hit"
        class:on={on("plate", i)}
        style="left:{c.x - 3}px; top:{c.y - 3}px; width:{c.w + 6}px; height:{c.h + 6}px"
        title="Platte, Wert {i + 1}: {pageStats[i] ? STAT_META[pageStats[i]].name : ''}"
        aria-label="Platte, Wert {i + 1}"
        onclick={() => onSelect({ kind: "plate", i })}
      ></button>
    {/each}
  {/if}
</div>

<style>
  .anatomy {
    position: relative;
    width: 100%;
  }
  .seat-preview {
    position: absolute;
    inset: 0;
    pointer-events: none;
  }
  .arcs {
    position: absolute;
    overflow: visible;
    pointer-events: none;
  }
  .arcs path {
    stroke: transparent;
    transition: stroke 0.12s;
  }
  .arcs path.on {
    stroke: rgba(236, 235, 231, 0.9);
    stroke-width: 3;
    transform: scale(1.18);
    transform-origin: 50px 50px;
  }
  .tag {
    position: absolute;
    transform: translate(0, -50%);
    height: 22px;
    min-width: 34px;
    padding: 0 7px;
    border-radius: 99px;
    border: 1px solid var(--line-2);
    background: var(--surface-2);
    color: var(--text-2);
    font-size: 11.5px;
    font-weight: 600;
    letter-spacing: 0.02em;
    cursor: pointer;
    transition:
      border-color 0.12s,
      color 0.12s,
      background 0.12s;
  }
  .tag.left {
    transform: translate(-100%, -50%);
  }
  .tag:hover {
    color: var(--text);
    border-color: var(--text-3);
  }
  .tag.on {
    background: var(--text);
    border-color: var(--text);
    color: var(--bg);
  }
  .col-hit {
    position: absolute;
    border: 1.5px dashed transparent;
    border-radius: 6px;
    background: transparent;
    cursor: pointer;
    transition: border-color 0.12s;
  }
  .col-hit:hover {
    border-color: rgba(236, 235, 231, 0.35);
  }
  .col-hit.on {
    border: 2px solid rgba(236, 235, 231, 0.9);
  }
</style>
