<script lang="ts">
  import { STAT_META, classicLines, fmt, moneyShort, stat, toneColor, unit } from "../stats";
  import type { GameKind, HudConfig, Point, SeatView } from "../types";
  import { draggable } from "./drag";
  import { classicAnchor } from "./geometry";

  interface Props {
    seat: SeatView;
    hud: HudConfig;
    /** Avatar centre; the box sits beside it until moved. */
    ring: Point;
    box: Point | null;
    scale: number;
    /** Size of the table area, for the default placement. */
    width: number;
    height: number;
    game: GameKind;
    currency: string | null;
    bountyCurrency: string | null;
    onMove: (p: Point, done: boolean) => void;
    onReset: () => void;
    onTap: () => void;
    /** Show the player's name (editor), since the client's name plate is not there. */
    showName?: boolean;
  }

  let { seat, hud, ring, box, scale: s, width, height, game, currency, bountyCurrency, onMove, onReset, onTap, showName = false }: Props =
    $props();

  let el: HTMLDivElement | undefined = $state();
  const layout = $derived(classicLines(hud));
  const hands = $derived(stat(seat.stats, "hands"));
  const color = (id: Parameters<typeof stat>[1]) => toneColor(hud, stat(seat.stats, id), "var(--text-3)");
  const stackText = $derived(
    game === "tournament" ? `${Math.round(seat.stack_bb)} BB` : `${moneyShort(seat.stack, currency)} · ${Math.round(seat.stack_bb)} BB`,
  );

  /** Until moved, the box hugs the avatar on the free side, vertically centred. */
  function origin(): Point {
    if (box) return box;
    const r = el!.getBoundingClientRect();
    const parent = el!.offsetParent!.getBoundingClientRect();
    return { x: r.left - parent.left, y: r.top - parent.top };
  }
  const anchor = $derived(classicAnchor(ring.x, ring.y, s, width, height));
  const style = $derived(
    box ? `left:${box.x}px; top:${box.y}px` : `left:${anchor.x}px; top:${anchor.y}px; transform:${anchor.transform}`,
  );
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  bind:this={el}
  class="box hit"
  class:hero={seat.is_hero}
  class:labelled={seat.note?.label}
  class:words={layout.words}
  style="{style}; --s:{s}; opacity:{hud.opacity}; --label:{seat.note?.label?.color ?? 'transparent'}"
  title="Ziehen verschiebt die Box · Doppelklick legt sie zurück · Klick zeigt alle Werte"
  use:draggable={{ origin, onMove, onTap, onDouble: onReset }}
>
  <div class="head">
    {#if showName}<span class="pname">{seat.name}</span>{/if}
    <span class="hands num">{fmt(hands)} <i>Hände</i></span>
    {#if hud.show_stack}<span class="meta num">{stackText}</span>{/if}
    {#if hud.show_bounty && seat.bounty !== null}<span class="ko num">{moneyShort(seat.bounty, bountyCurrency ?? "USD")}</span>{/if}
  </div>
  {#each layout.lines as line, i (i)}
    {#if layout.words}
      <div class="sentence">
        {#each line as id, j (j)}
          {#if j > 0}<span class="dot">·</span>{/if}
          <span class="phrase" title={STAT_META[id].help}>
            <span class="word">{STAT_META[id].plain}</span>
            <b class="num" style="color:{color(id)}">{fmt(stat(seat.stats, id))}<small>{unit(stat(seat.stats, id))}</small></b>
            {#if STAT_META[id].after}<span class="word">{STAT_META[id].after}</span>{/if}
          </span>
        {/each}
      </div>
    {:else}
      <div class="cols" class:wide={line.length > 4}>
        {#each line as id, j (j)}
          <span class="col" title={STAT_META[id].help}>
            <b class="num" style="color:{id === 'hands' ? 'var(--text)' : color(id)}">{fmt(stat(seat.stats, id))}</b>
            <i>{STAT_META[id].short}</i>
          </span>
        {/each}
      </div>
    {/if}
  {/each}
</div>

<style>
  .box {
    position: absolute;
    pointer-events: auto;
    cursor: grab;
    padding: calc(4px * var(--s)) calc(8px * var(--s)) calc(5px * var(--s));
    border-radius: calc(7px * var(--s));
    background: rgba(26, 29, 34, 0.95);
    box-shadow:
      0 0 0 1px rgba(255, 255, 255, 0.08) inset,
      0 calc(3px * var(--s)) calc(10px * var(--s)) rgba(0, 0, 0, 0.45);
    white-space: nowrap;
    line-height: 1.15;
  }
  .box:active {
    cursor: grabbing;
  }
  .labelled {
    box-shadow:
      0 0 0 calc(2px * var(--s)) var(--label) inset,
      0 calc(3px * var(--s)) calc(10px * var(--s)) rgba(0, 0, 0, 0.45);
  }
  .hero {
    box-shadow:
      0 0 0 1px rgba(236, 235, 231, 0.2) inset,
      0 calc(3px * var(--s)) calc(10px * var(--s)) rgba(0, 0, 0, 0.45);
  }
  .head {
    display: flex;
    align-items: baseline;
    gap: calc(7px * var(--s));
    margin-bottom: calc(2px * var(--s));
    font-size: calc(10.5px * var(--s));
    color: #8d9097;
  }
  .hands {
    color: #c6c8cc;
  }
  .pname {
    font-family: var(--font-ui);
    font-weight: 600;
    color: var(--text);
  }
  .hands i {
    font-style: normal;
    color: #8d9097;
  }
  .meta {
    margin-left: auto;
  }
  .ko {
    padding: 0 calc(4px * var(--s));
    border-radius: calc(3px * var(--s));
    background: #3a3f47;
    color: #ecebe7;
  }
  .sentence {
    font-family: var(--font-ui);
    font-size: calc(11.5px * var(--s));
    color: #b9bbc0;
  }
  .sentence b {
    margin-left: calc(3px * var(--s));
    font-weight: 600;
    font-size: 1.08em;
  }
  .sentence small {
    margin-left: 1px;
    font-size: 0.8em;
    font-weight: 500;
    color: #a4a7ad;
  }
  .cols.wide .col b {
    font-size: calc(14px * var(--s));
  }
  .cols.wide {
    gap: calc(7px * var(--s));
  }
  .dot {
    margin: 0 calc(5px * var(--s));
    color: #5d6168;
  }
  .cols {
    display: flex;
    gap: calc(9px * var(--s));
  }
  .col {
    display: flex;
    flex-direction: column;
    align-items: center;
    min-width: calc(20px * var(--s));
  }
  .col b {
    font-size: calc(16px * var(--s));
    font-weight: 600;
    line-height: 1.05;
  }
  .col i {
    font-style: normal;
    font-family: var(--font-num);
    font-size: calc(9.5px * var(--s));
    font-weight: 600;
    letter-spacing: 0.03em;
    color: #a4a7ad;
  }
</style>
