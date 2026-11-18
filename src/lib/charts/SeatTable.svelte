<script lang="ts">
  import { untrack, type Snippet } from "svelte";
  import { preview } from "../learn/preview";
  import Tag from "../ui/Tag.svelte";
  import Tooltip from "../ui/Tooltip.svelte";
  import { posHint, type SeatMark } from "./words";

  /**
   * Small table to pick a seat. Your seat turns to the bottom, as in the poker client; players who
   * already acted in the chosen situation carry their action, those who folded fade.
   */
  let {
    positions,
    seat,
    onPick,
    marks = {},
    folded = [],
    middle,
  }: {
    positions: { seat: number; name: string }[];
    seat: number;
    onPick: (seat: number) => void;
    marks?: Record<number, SeatMark>;
    folded?: number[];
    /** Controls in the middle of the felt, e.g. the table size. */
    middle?: Snippet;
  } = $props();

  const n = $derived(positions.length);

  // The table turns along its rim so your seat ends at the bottom. `turn` is the seat shown
  // at the bottom, between two seats while it turns.
  let turn = $state(untrack(() => seat));
  /** Browser preview: stop the turn at this share, for screenshots of the motion. */
  const freeze = Number(preview("seatanim") ?? NaN);
  const ease = (k: number) => (k < 0.5 ? 4 * k * k * k : 1 - Math.pow(-2 * k + 2, 3) / 2);
  $effect(() => {
    const target = seat;
    const count = n;
    const from = untrack(() => turn);
    // The shortest way round.
    let d = (((target - from) % count) + count * 1.5) % count - count / 2;
    if (d === 0 || matchMedia("(prefers-reduced-motion: reduce)").matches) {
      turn = target;
      return;
    }
    const start = performance.now();
    let raf = 0;
    const step = (t: number) => {
      let k = Math.min(1, (t - start) / 520);
      if (Number.isFinite(freeze)) k = Math.min(k, freeze);
      turn = from + d * ease(k);
      if (k < 1 && !(Number.isFinite(freeze) && k >= freeze)) raf = requestAnimationFrame(step);
      else if (k >= 1) turn = target;
    };
    raf = requestAnimationFrame(step);
    return () => cancelAnimationFrame(raf);
  });

  /** Seats sit on an ellipse; `k` < 1 moves a point along the line from the seat to the middle. */
  function at(s: number, k = 1, offset = 0): string {
    const a = Math.PI / 2 + ((s - turn) * 2 * Math.PI) / n + offset;
    return `left:${(50 + 46 * k * Math.cos(a)).toFixed(2)}%;top:${(50 + 42 * k * Math.sin(a)).toFixed(2)}%`;
  }
  /** Actions lie between seat and middle, the same distance for every seat. */
  const MARK = 0.55;

  const btn = $derived(positions.find((p) => p.name === "BTN") ?? (n === 2 ? positions[0] : null));
  const mine = $derived(positions.find((p) => p.seat === seat)?.name ?? "");
  const about = $derived.by(() => {
    const t = posHint(mine).replace(/^[^:]*:\s*/, "");
    return t.charAt(0).toUpperCase() + t.slice(1);
  });
</script>

<div class="seat-table">
  <div class="felt" role="group" aria-label="Deine Position am Tisch">
    <div class="rail"></div>
    {#if middle}<div class="center">{@render middle()}</div>{/if}
    {#if btn}<span class="dealer" style={at(btn.seat, 0.78, -0.42)} aria-hidden="true">D</span>{/if}
    {#each positions as p (p.seat)}
      {@const m = marks[p.seat]}
      {#if m}<span class="mark" style="{at(p.seat, MARK)};--c:{m.color}">{m.text}</span>{/if}
      <div class="seat" class:folded={folded.includes(p.seat) && p.seat !== seat} style={at(p.seat)}>
        <Tooltip text={posHint(p.name)}>
          <Tag selected={p.seat === seat} onclick={() => onPick(p.seat)}><span class="seat-name">{p.name}</span></Tag>
        </Tooltip>
      </div>
    {/each}
  </div>
  <p class="cap"><b>Du sitzt unten: {mine}.</b> {about}</p>
</div>

<style>
  .seat-table {
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-width: 0;
  }
  /* Seat pills reach past the ellipse; the side margin keeps them inside the column. */
  .felt {
    position: relative;
    width: calc(100% - 36px);
    aspect-ratio: 1.7 / 1;
    margin: 2px auto;
  }
  .rail {
    position: absolute;
    inset: 8% 4%;
    border-radius: 999px;
    background:
      radial-gradient(ellipse at 50% 40%, color-mix(in srgb, var(--pos) 9%, var(--surface-2)), var(--surface-2) 70%);
    box-shadow:
      inset 0 0 0 1px var(--line-2),
      0 0 0 5px var(--bg),
      0 0 0 6px var(--line);
  }
  .center {
    position: absolute;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
  }
  .dealer {
    position: absolute;
    transform: translate(-50%, -50%);
    width: 16px;
    height: 16px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: var(--text);
    color: var(--bg);
    font-size: var(--fs-xs);
    font-weight: 700;
  }
  .seat {
    position: absolute;
    transform: translate(-50%, -50%);
    transition: opacity var(--dur) var(--ease-standard);
  }
  .seat.folded {
    opacity: 0.6;
  }
  .seat-name {
    font-family: var(--font-num);
    font-weight: 600;
  }
  .mark {
    position: absolute;
    transform: translate(-50%, -50%);
    padding: 0 6px;
    border-radius: 4px;
    background: var(--c);
    color: var(--text);
    font-size: var(--fs-xs);
    font-weight: 600;
    line-height: 16px;
    max-width: 48px;
    white-space: nowrap;
  }
  .cap {
    margin: 0;
    font-size: var(--fs-sm);
    line-height: 1.45;
    color: var(--text-2);
  }
  .cap b {
    color: var(--text);
    font-weight: 600;
  }
</style>
