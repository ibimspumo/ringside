<script lang="ts">
  import { api } from "../api";
  import PlayingCard from "../PlayingCard.svelte";
  import { money } from "../stats";
  import type { GameKind, Hand, HandSummary } from "../types";
  import GameFilter from "./GameFilter.svelte";
  import Replayer from "./Replayer.svelte";

  let {
    refresh,
    player,
    onClearPlayer,
  }: { refresh: number; player: string | null; onClearPlayer: () => void } = $props();

  const PAGE = 100;
  let game = $state<GameKind | null>(null);
  let list = $state<HandSummary[]>([]);
  let more = $state(false);
  let selected = $state<Hand | null>(null);

  $effect(() => {
    void refresh;
    const g = game;
    const p = player;
    api.hands(g, p, PAGE, 0).then((r) => {
      list = r;
      more = r.length === PAGE;
      if (!selected && r[0]) open(r[0].id);
    });
  });

  async function loadMore() {
    const r = await api.hands(game, player, PAGE, list.length);
    list = [...list, ...r];
    more = r.length === PAGE;
  }

  async function open(id: number) {
    selected = await api.hand(id);
  }

  function stakes(h: HandSummary) {
    if (h.game === "tournament") return "Turnier";
    return `${money(h.small_blind, h.currency)}/${money(h.big_blind, h.currency)}`;
  }
</script>

<div class="page wide">
  <div class="page-head">
    <div>
      <h1>Hände & Replay</h1>
      <p>Jede Hand Schritt für Schritt nachspielen. Pfeiltasten springen, Leertaste spielt ab.</p>
    </div>
    <div class="tools">
      {#if player}
        <button class="chip" onclick={onClearPlayer}>mit {player} ✕</button>
      {/if}
      <GameFilter bind:value={game} />
    </div>
  </div>

  <div class="layout">
    <div class="panel list">
      {#each list as h (h.id)}
        <button class="row" class:on={selected?.id === h.id} onclick={() => open(h.id)}>
          <span class="cards">
            {#each h.hero_cards.split(" ").filter(Boolean) as c (c)}<PlayingCard card={c} size={20} />{/each}
          </span>
          <span class="meta">
            <b>{h.table_name}</b>
            <small>{stakes(h)} · {h.played_at.slice(5, 16)}</small>
          </span>
          {#if h.hero_net !== null}
            <span class="num net" class:pos={h.hero_net > 0} class:neg={h.hero_net < 0}>
              {h.hero_net > 0 ? "+" : ""}{(h.hero_net / h.big_blind).toFixed(1)}<small>BB</small>
            </span>
          {/if}
        </button>
      {:else}
        <div class="empty">Noch keine Hände.</div>
      {/each}
      {#if more}
        <button class="btn more" onclick={loadMore}>Mehr laden</button>
      {/if}
    </div>

    <div>
      {#if selected}
        <div class="info">
          <span class="num">#{selected.id}</span>
          <span>{selected.table_name}</span>
          <span>{selected.variant}</span>
          <span>{selected.played_at}</span>
        </div>
        <Replayer hand={selected} />
      {/if}
    </div>
  </div>
</div>

<style>
  .wide {
    max-width: 1500px;
  }
  .tools {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .chip {
    border: 1px solid var(--line-strong);
    background: var(--panel-2);
    border-radius: 99px;
    padding: 6px 12px;
    cursor: pointer;
  }
  .layout {
    display: grid;
    grid-template-columns: 330px 1fr;
    gap: 16px;
    align-items: start;
  }
  .list {
    max-height: calc(100vh - 150px);
    overflow: auto;
    padding: 6px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 8px 10px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    text-align: left;
    cursor: pointer;
  }
  .row:hover {
    background: rgba(255, 255, 255, 0.03);
  }
  .row.on {
    background: var(--panel-2);
  }
  .cards {
    display: flex;
    gap: 2px;
    width: 44px;
  }
  .meta {
    flex: 1;
    min-width: 0;
  }
  .meta b {
    display: block;
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .meta small {
    color: var(--text-faint);
    font-size: 12px;
  }
  .net {
    font-size: 15px;
  }
  .net small {
    font-size: 10px;
    margin-left: 2px;
    color: var(--text-faint);
  }
  .more {
    width: 100%;
    justify-content: center;
    margin-top: 6px;
  }
  .info {
    display: flex;
    gap: 14px;
    margin-bottom: 10px;
    color: var(--text-dim);
    font-size: 13px;
  }
</style>
