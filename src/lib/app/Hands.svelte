<script lang="ts">
  import { api } from "../api";
  import PlayingCard from "../PlayingCard.svelte";
  import { moneyShort } from "../stats";
  import type { Filter, Hand, HandQuery, HandSummary } from "../types";
  import { date, dateTime, sign, signed, time } from "./format";
  import Replayer from "./Replayer.svelte";

  let {
    filter,
    refresh,
    player,
    openId,
    onClearPlayer,
  }: {
    filter: Filter;
    refresh: number;
    player: string | null;
    openId: number | null;
    onClearPlayer: () => void;
  } = $props();

  const PAGE = 100;
  let showdownOnly = $state(false);
  let result = $state<"won" | "lost" | null>(null);
  let minPot = $state<number | null>(null);

  let list = $state<HandSummary[]>([]);
  let loaded = $state(false);
  let more = $state(false);
  let selected = $state<Hand | null>(null);

  const query = $derived<HandQuery>({
    ...filter,
    player,
    showdown_only: showdownOnly,
    result,
    min_pot_bb: minPot && minPot > 0 ? minPot : null,
    limit: PAGE,
    offset: 0,
  });

  $effect(() => {
    void refresh;
    const q = { ...query };
    let live = true;
    api.hands(q).then((r) => {
      if (!live) return;
      list = r;
      loaded = true;
      more = r.length === PAGE;
      if (!selected && openId === null && r[0]) open(r[0].id);
    });
    return () => (live = false);
  });

  $effect(() => {
    if (openId !== null) open(openId);
  });

  async function loadMore() {
    const r = await api.hands({ ...query, offset: list.length });
    list = [...list, ...r];
    more = r.length === PAGE;
  }

  async function open(id: number) {
    selected = await api.hand(id);
  }

  const cards = (s: string) => s.split(" ").filter(Boolean);
  const netBb = (h: HandSummary) => (h.hero_net !== null && h.big_blind > 0 ? h.hero_net / h.big_blind : null);

  function stakes(h: { game: string; small_blind: number; big_blind: number; currency: string | null }) {
    if (h.game === "tournament") return `Turnier · ${Math.round(h.small_blind / 100)}/${Math.round(h.big_blind / 100)}`;
    return `${moneyShort(h.small_blind, h.currency)}/${moneyShort(h.big_blind, h.currency)}`;
  }
</script>

<div class="hands">
  <div class="bar">
    <label class="toggle">
      <input type="checkbox" bind:checked={showdownOnly} />
      Nur Showdown
    </label>
    <div class="seg" role="group" aria-label="Ergebnis">
      <button class:on={result === null} onclick={() => (result = null)}>Alle</button>
      <button class:on={result === "won"} onclick={() => (result = "won")}>Gewonnen</button>
      <button class:on={result === "lost"} onclick={() => (result = "lost")}>Verloren</button>
    </div>
    <label class="pot">
      Pot ab
      <input class="field num" type="number" min="0" step="5" placeholder="0" bind:value={minPot} />
      BB
    </label>
    {#if player}
      <span class="chip">
        mit {player}
        <button onclick={onClearPlayer} aria-label="Spielerfilter entfernen" title="Spielerfilter entfernen">
          <svg viewBox="0 0 24 24" width="12" height="12"><path d="M6 6l12 12M18 6 6 18" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" /></svg>
        </button>
      </span>
    {/if}
  </div>

  <div class="body">
    <div class="list" role="listbox" aria-label="Hände">
      {#each list as h (h.id)}
        {@const net = netBb(h)}
        <button class="row" class:on={selected?.id === h.id} role="option" aria-selected={selected?.id === h.id} onclick={() => open(h.id)}>
          <span class="hole">
            {#each cards(h.hero_cards) as c (c)}<PlayingCard card={c} size={22} />{:else}<span class="faint">–</span>{/each}
          </span>
          <span class="mid">
            <span class="top">
              <b>{h.hero_position ?? "–"}</b>
              {#if h.hero_showdown}<span class="sd" title="Showdown">SD</span>{/if}
            </span>
            <span class="board">
              {#each cards(h.board) as c (c)}<PlayingCard card={c} size={16} />{:else}<span class="faint">kein Flop</span>{/each}
            </span>
          </span>
          <span class="end">
            <span class="net num {sign(net)}">{net === null ? "–" : signed(net)}<span class="unit">BB</span></span>
            <small class="num" title={date(h.played_at)}>{time(h.played_at)} · {date(h.played_at).slice(0, 6)}</small>
          </span>
        </button>
      {:else}
        {#if loaded}
          <div class="empty">Keine Hände für diese Filter. Lockere die Filter oder wähle oben einen längeren Zeitraum.</div>
        {/if}
      {/each}
      {#if more}
        <button class="btn more" onclick={loadMore}>Mehr laden</button>
      {/if}
    </div>

    <div class="stage">
      {#if selected}
        <div class="info">
          <h2>{selected.table_name}</h2>
          <span>{stakes(selected)}</span>
          <span>{dateTime(selected.played_at)}</span>
          <span class="faint num">#{selected.id}</span>
        </div>
        <Replayer hand={selected} />
      {:else if loaded && list.length}
        <div class="empty">Wähle links eine Hand zum Nachspielen.</div>
      {/if}
    </div>
  </div>
</div>

<style>
  .hands {
    display: flex;
    flex-direction: column;
    height: 100%;
  }
  .bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px 18px;
    padding: 10px 24px;
    border-bottom: 1px solid var(--line);
    font-size: 13px;
    color: var(--text-2);
  }
  .toggle {
    display: flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
    color: var(--text);
  }
  .toggle input {
    margin: 0;
    accent-color: var(--text);
  }
  .pot {
    display: flex;
    align-items: center;
    gap: 7px;
  }
  .pot input {
    width: 64px;
    height: 30px;
    text-align: right;
    color: var(--text);
  }
  .body {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: 340px minmax(0, 1fr);
  }
  .list {
    overflow: auto;
    border-right: 1px solid var(--line);
    padding: 6px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 7px 10px;
    border: 0;
    border-radius: var(--r-sm);
    background: transparent;
    text-align: left;
    cursor: pointer;
  }
  .row:hover {
    background: var(--surface);
  }
  .row.on {
    background: var(--surface-2);
  }
  .hole {
    display: flex;
    gap: 2px;
    width: 48px;
    flex-shrink: 0;
  }
  .mid {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .top {
    display: flex;
    align-items: baseline;
    gap: 8px;
    font-size: 12.5px;
  }
  .top b {
    font-family: var(--font-num);
    font-weight: 600;
    font-size: 13px;
    min-width: 30px;
  }
  .sd {
    padding: 0 4px;
    border-radius: 3px;
    font-family: var(--font-num);
    font-size: 10.5px;
    color: var(--sd);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--sd) 45%, transparent);
  }
  .board {
    display: flex;
    gap: 2px;
    min-height: 20px;
    font-size: 11.5px;
    align-items: center;
  }
  .end {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 3px;
    white-space: nowrap;
  }
  .net {
    font-size: 15px;
  }
  .end small {
    font-size: 11.5px;
    color: var(--text-3);
  }
  .more {
    width: 100%;
    margin-top: 6px;
  }
  .stage {
    display: flex;
    flex-direction: column;
    min-height: 0;
    overflow: hidden;
    padding: 16px 24px 18px;
  }
  .info {
    display: flex;
    align-items: baseline;
    flex-wrap: wrap;
    gap: 4px 14px;
    flex-shrink: 0;
    margin-bottom: 12px;
    color: var(--text-2);
    font-size: 13px;
  }
  .info h2 {
    margin: 0;
    font-family: var(--font-num);
    font-size: 17px;
    font-weight: 600;
    color: var(--text);
  }

  @container (max-width: 980px) {
    .body {
      grid-template-columns: 290px minmax(0, 1fr);
    }
    .stage {
      padding: 14px 16px 16px;
    }
  }
</style>
