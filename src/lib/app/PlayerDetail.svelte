<script lang="ts">
  import { api } from "../api";
  import PlayingCard from "../PlayingCard.svelte";
  import { RATED_STATS, STAT_META, compact, fmt, stat, toneColor } from "../stats";
  import type { HudConfig, PlayerView, ShownHand } from "../types";
  import { dateTime, int, sign, signed } from "./format";
  import Wheel from "./Wheel.svelte";

  let {
    player,
    hud,
    onClose,
    onHands,
  }: {
    player: PlayerView;
    hud: HudConfig;
    onClose: () => void;
    onHands: (name: string, handId?: number) => void;
  } = $props();

  let shown = $state<ShownHand[] | null>(null);
  let panel = $state<HTMLElement>();

  $effect(() => {
    const name = player.name;
    let live = true;
    shown = null;
    api.shownHands(name).then((r) => live && (shown = r));
    panel?.focus();
    return () => (live = false);
  });

  const hands = $derived(stat(player.stats, "hands")?.value ?? 0);
  const cards = (s: string) => s.split(" ").filter(Boolean);

  function key(e: KeyboardEvent) {
    if (e.key === "Escape") onClose();
  }
</script>

<svelte:window onkeydown={key} />

<div class="backdrop" onclick={onClose} aria-hidden="true"></div>
<aside class="drawer" bind:this={panel} tabindex="-1" aria-label="Spieler {player.name}">
  <header>
    <Wheel stats={player.stats} {hud} size={48} hands={compact(hands)} />
    <div class="who">
      <h2>{player.name}</h2>
      <span class="dim"><span class="num">{int(hands)}</span> gemeinsame Hände</span>
    </div>
    <button class="icon-btn" onclick={onClose} aria-label="Schließen" title="Schließen (Esc)">
      <svg viewBox="0 0 24 24" width="18" height="18"><path d="M6 6l12 12M18 6 6 18" /></svg>
    </button>
  </header>

  <div class="body">
    <button class="btn all" onclick={() => onHands(player.name)}>Alle Hände mit {player.name} anzeigen</button>

    <h3 class="panel-title">Werte</h3>
    <dl class="stats">
      {#each RATED_STATS as id (id)}
        {@const s = stat(player.stats, id)}
        <div class="stat">
          <dt>
            <span>{STAT_META[id].name}</span>
            <small>{STAT_META[id].help}</small>
          </dt>
          <dd>
            <b class="num" style="color:{toneColor(hud, s, 'var(--text-3)')}"
              >{fmt(s)}{#if s && s.value !== null && !STAT_META[id].ratio}<span class="unit">%</span>{/if}</b
            >
            <small class="num" title="Treffer / Gelegenheiten">{int(s?.made ?? 0)}/{int(s?.opp ?? 0)}</small>
          </dd>
        </div>
      {/each}
    </dl>

    <h3 class="panel-title">Gezeigte Karten</h3>
    {#if shown === null}
      <p class="faint">Lädt …</p>
    {:else if shown.length === 0}
      <p class="faint none">{player.name} hat dir noch keine Karten gezeigt.</p>
    {:else}
      <ul class="shown">
        {#each shown as h (h.hand_id)}
          {@const net = h.big_blind ? h.net / h.big_blind : 0}
          <li>
            <span class="hole">{#each cards(h.cards) as c (c)}<PlayingCard card={c} size={24} />{/each}</span>
            <span class="board">
              {#each cards(h.board) as c (c)}<PlayingCard card={c} size={18} />{/each}
              {#if !h.board}<span class="faint">kein Board</span>{/if}
            </span>
            <span class="meta">
              <b class="num {sign(net)}">{signed(net)}<span class="unit">BB</span></b>
              <small>{h.position ?? "–"} · {dateTime(h.played_at)}</small>
            </span>
            <button class="btn" onclick={() => onHands(player.name, h.hand_id)}>Abspielen</button>
          </li>
        {/each}
      </ul>
    {/if}
  </div>
</aside>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 30;
    background: rgba(8, 9, 11, 0.5);
  }
  .drawer {
    position: fixed;
    z-index: 31;
    top: 0;
    right: 0;
    bottom: 0;
    width: min(460px, calc(100vw - 220px));
    display: flex;
    flex-direction: column;
    background: var(--surface);
    border-left: 1px solid var(--line-2);
    box-shadow: -16px 0 40px rgba(0, 0, 0, 0.4);
    outline: none;
  }
  header {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 18px 16px 16px 20px;
    border-bottom: 1px solid var(--line);
  }
  .who {
    flex: 1;
    min-width: 0;
  }
  h2 {
    margin: 0;
    font-family: var(--font-num);
    font-size: 20px;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .who .dim {
    font-size: 13px;
  }
  header .icon-btn {
    align-self: flex-start;
  }
  .body {
    flex: 1;
    overflow: auto;
    padding: 16px 20px 28px;
  }
  .all {
    width: 100%;
  }
  h3 {
    margin: 22px 0 8px;
  }
  .stats {
    margin: 0;
  }
  .stat {
    display: flex;
    gap: 16px;
    padding: 9px 0;
    border-bottom: 1px solid var(--line);
  }
  .stat:last-child {
    border-bottom: 0;
  }
  dt {
    flex: 1;
    min-width: 0;
  }
  dt span {
    display: block;
    font-size: 13.5px;
  }
  dt small {
    display: block;
    margin-top: 1px;
    font-size: 12px;
    line-height: 1.4;
    color: var(--text-3);
  }
  dd {
    margin: 0;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    min-width: 64px;
  }
  dd b {
    font-size: 19px;
    font-weight: 600;
    line-height: 1.2;
  }
  dd small {
    font-size: 11.5px;
    color: var(--text-3);
  }
  .none {
    font-size: 13px;
  }
  .shown {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .shown li {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto auto;
    align-items: center;
    gap: 12px;
    padding: 10px 0;
    border-bottom: 1px solid var(--line);
  }
  .hole,
  .board {
    display: flex;
    gap: 3px;
  }
  .board {
    gap: 2px;
    font-size: 12px;
  }
  .meta {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
  }
  .meta b {
    font-size: 15px;
    font-weight: 600;
  }
  .meta small {
    font-size: 11.5px;
    color: var(--text-3);
    white-space: nowrap;
  }
  .shown .btn {
    height: 28px;
    padding: 0 10px;
    font-size: 12.5px;
  }
</style>
