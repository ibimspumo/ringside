<script lang="ts">
  import { STAT_META, fill, fmt, stat, tone } from "../stats";
  import type { GameKind, HudConfig, SeatView } from "../types";
  import { money } from "../stats";

  interface Props {
    seat: SeatView;
    hud: HudConfig;
    /** Ring centre in px inside the parent. */
    x: number;
    y: number;
    scale: number;
    /** Card sits on the side facing the table centre. */
    cardLeft: boolean;
    game: GameKind;
    currency: string | null;
    open: boolean;
    onToggle: () => void;
    onDrag: (x: number, y: number, done: boolean) => void;
  }

  let { seat, hud, x, y, scale, cardLeft, game, currency, open, onToggle, onDrag }: Props = $props();

  const outer = $derived(stat(seat.stats, hud.ring_outer));
  const inner = $derived(stat(seat.stats, hud.ring_inner));
  const hands = $derived(stat(seat.stats, "hands"));
  const rows = $derived(hud.rows.map((id) => stat(seat.stats, id)));
  const details = $derived(seat.stats.filter((s) => s.id !== "hands"));

  let drag: { px: number; py: number; ox: number; oy: number; moved: boolean } | null = null;

  function down(e: PointerEvent) {
    if (e.button !== 0) return;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    drag = { px: e.clientX, py: e.clientY, ox: x, oy: y, moved: false };
  }

  function move(e: PointerEvent) {
    if (!drag) return;
    const dx = e.clientX - drag.px;
    const dy = e.clientY - drag.py;
    if (!drag.moved && Math.hypot(dx, dy) < 4) return;
    drag.moved = true;
    onDrag(drag.ox + dx, drag.oy + dy, false);
  }

  function up(e: PointerEvent) {
    if (!drag) return;
    const d = drag;
    drag = null;
    if (d.moved) onDrag(d.ox + e.clientX - d.px, d.oy + e.clientY - d.py, true);
    else onToggle();
  }
</script>

<div
  class="seat"
  class:left={cardLeft}
  class:hero={seat.is_hero}
  style="left:{x}px; top:{y}px; --s:{scale}; opacity:{hud.opacity}"
>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="grab hit" onpointerdown={down} onpointermove={move} onpointerup={up}>
    <svg class="ring" viewBox="0 0 100 100" aria-hidden="true">
      <circle class="track" cx="50" cy="50" r="45" stroke-width="7" />
      <circle class="track" cx="50" cy="50" r="35.5" stroke-width="5" />
      <circle
        class="arc tone-{tone(hud, outer)}"
        cx="50"
        cy="50"
        r="45"
        stroke-width="7"
        pathLength="100"
        stroke-dasharray="{fill(outer) * 100} 100"
      />
      <circle
        class="arc tone-{tone(hud, inner)}"
        cx="50"
        cy="50"
        r="35.5"
        stroke-width="5"
        pathLength="100"
        stroke-dasharray="{fill(inner) * 100} 100"
      />
    </svg>
    <div class="badge num">{fmt(hands)}</div>

    <div class="card">
      <div class="main num">
        <span class="tone-{tone(hud, outer)}">{fmt(outer)}</span><span class="sep">/</span><span
          class="tone-{tone(hud, inner)}">{fmt(inner)}</span
        >
        {#if hud.show_stack}
          <span class="stack">
            {#if game === "tournament"}
              {Math.round(seat.stack_bb)}<small>BB</small>
            {:else}
              {money(seat.stack, currency, game)}
            {/if}
          </span>
        {/if}
      </div>
      {#if rows.length}
        <div class="rows num">
          {#each rows as s, i (hud.rows[i])}
            <span class="cell"><b class="tone-{tone(hud, s)}">{fmt(s)}</b><i>{STAT_META[hud.rows[i]].short}</i></span>
          {/each}
        </div>
      {/if}
    </div>
  </div>

  {#if open}
    <div class="pop hit">
      <header>
        <strong>{seat.name}</strong>
        <span class="num">{fmt(hands)} Hände</span>
      </header>
      <div class="grid">
        {#each details as s (s.id)}
          <div class="line" title={STAT_META[s.id].help}>
            <span class="label">{STAT_META[s.id].name}</span>
            <span class="num val tone-{tone(hud, s)}">{fmt(s)}{STAT_META[s.id].ratio || s.value === null ? "" : "%"}</span>
            <span class="num sample">{s.made}/{s.opp}</span>
          </div>
        {/each}
      </div>
    </div>
  {/if}
</div>

<style>
  .seat {
    position: absolute;
    width: 0;
    height: 0;
    pointer-events: none;
  }

  .grab {
    position: absolute;
    left: calc(-38px * var(--s));
    top: calc(-38px * var(--s));
    width: calc(76px * var(--s));
    height: calc(76px * var(--s));
    pointer-events: auto;
    cursor: grab;
    touch-action: none;
  }
  .grab:active {
    cursor: grabbing;
  }

  .ring {
    width: 100%;
    height: 100%;
    overflow: visible;
    filter: drop-shadow(0 0 calc(5px * var(--s)) rgba(0, 0, 0, 0.6));
  }
  .ring circle {
    fill: none;
    transform: rotate(-90deg);
    transform-origin: 50% 50%;
  }
  .track {
    stroke: rgba(10, 12, 18, 0.72);
  }
  .arc {
    stroke: currentColor;
    stroke-linecap: round;
    filter: drop-shadow(0 0 2.5px currentColor);
    transition: stroke-dasharray 0.6s cubic-bezier(0.2, 0.7, 0.2, 1);
  }
  .arc.tone-none {
    stroke: transparent;
    filter: none;
  }

  .badge {
    position: absolute;
    left: 50%;
    bottom: calc(-7px * var(--s));
    transform: translateX(-50%);
    min-width: calc(26px * var(--s));
    padding: calc(1px * var(--s)) calc(6px * var(--s));
    border-radius: 99px;
    background: #0d1017;
    border: 1px solid rgba(255, 255, 255, 0.16);
    font-size: calc(10.5px * var(--s));
    line-height: 1.3;
    text-align: center;
    color: var(--text);
  }

  .card {
    position: absolute;
    top: 50%;
    left: calc(100% + 4px * var(--s));
    transform: translateY(-50%);
    padding: calc(4px * var(--s)) calc(8px * var(--s)) calc(5px * var(--s));
    border-radius: calc(8px * var(--s));
    background: linear-gradient(180deg, rgba(22, 26, 36, 0.94), rgba(11, 13, 19, 0.94));
    border: 1px solid rgba(255, 255, 255, 0.1);
    box-shadow:
      0 calc(4px * var(--s)) calc(14px * var(--s)) rgba(0, 0, 0, 0.45),
      0 1px 0 rgba(255, 255, 255, 0.05) inset;
    white-space: nowrap;
  }
  .left .card {
    left: auto;
    right: calc(100% + 4px * var(--s));
  }
  .hero .card {
    border-color: rgba(255, 138, 61, 0.35);
  }

  .main {
    display: flex;
    align-items: baseline;
    gap: calc(2px * var(--s));
    font-size: calc(17px * var(--s));
    font-weight: 600;
    line-height: 1.1;
  }
  .sep {
    color: var(--text-faint);
    font-weight: 400;
  }
  .stack {
    margin-left: calc(8px * var(--s));
    font-size: calc(11.5px * var(--s));
    font-weight: 500;
    color: var(--text-dim);
  }
  .stack small {
    font-size: 0.8em;
    margin-left: 1px;
  }

  .rows {
    display: flex;
    gap: calc(8px * var(--s));
    margin-top: calc(2px * var(--s));
    font-size: calc(12.5px * var(--s));
    line-height: 1.15;
  }
  .cell b {
    font-weight: 600;
  }
  .cell i {
    font-style: normal;
    margin-left: calc(2px * var(--s));
    font-size: 0.72em;
    color: var(--text-faint);
    letter-spacing: 0.04em;
  }

  .pop {
    position: absolute;
    top: calc(44px * var(--s));
    left: calc(-38px * var(--s));
    z-index: 10;
    width: max(230px, calc(250px * var(--s)));
    padding: 10px 12px 8px;
    border-radius: 12px;
    background: rgba(13, 16, 23, 0.97);
    border: 1px solid rgba(255, 255, 255, 0.12);
    box-shadow: 0 18px 40px rgba(0, 0, 0, 0.55);
    pointer-events: auto;
    font-size: 12.5px;
    opacity: 1;
  }
  .left .pop {
    left: auto;
    right: calc(-38px * var(--s));
  }
  .pop header {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 8px;
    padding-bottom: 6px;
    margin-bottom: 4px;
    border-bottom: 1px solid var(--line);
  }
  .pop header span {
    color: var(--text-dim);
  }
  .line {
    display: grid;
    grid-template-columns: 1fr auto 52px;
    gap: 8px;
    padding: 2.5px 0;
  }
  .label {
    color: var(--text-dim);
  }
  .val {
    font-size: 14px;
    font-weight: 600;
  }
  .sample {
    text-align: right;
    color: var(--text-faint);
  }
</style>
