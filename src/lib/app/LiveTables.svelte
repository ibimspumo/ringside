<script lang="ts">
  import { onMount } from "svelte";
  import { api, on } from "../api";
  import { STAT_META, fmt, moneyShort, stat, toneColor } from "../stats";
  import type { LiveTable, PlayerView, SeatView, StatId, TableView } from "../types";
  import { int } from "./format";
  import PlayerDetail from "./PlayerDetail.svelte";
  import Wheel from "./Wheel.svelte";

  let { onHands }: { onHands: (name: string, handId?: number) => void } = $props();

  const COLS: StatId[] = ["hands", "vpip", "pfr", "three_bet", "fold_to_three_bet", "steal", "fold_to_steal", "cbet", "af"];
  /** Below this many chances a value is still noise; it stays visible but recedes. */
  const SMALL_SAMPLE = 20;

  let tables = $state<LiveTable[]>([]);
  let loaded = $state(false);
  let selected = $state<{ player: PlayerView; table: TableView } | null>(null);

  function load() {
    api.liveTables().then((t) => {
      tables = t;
      loaded = true;
    });
  }

  onMount(() => {
    load();
    const subs = [on("table-updated", load), on("tables-tracked", load), on("settings-changed", load), on("notes-changed", load)];
    // Titles change when a tournament moves you to another table; the tracker reports no event for that.
    const poll = setInterval(load, 5000);
    return () => {
      clearInterval(poll);
      subs.forEach((s) => s.then((f) => f()));
    };
  });

  const seatsOf = (t: TableView) => [...t.seats].sort((a, b) => a.visual - b.visual);
  const hasBounty = (t: TableView) => t.seats.some((s) => s.bounty !== null);
  const hasNotes = (t: TableView) => t.seats.some((s) => s.note);

  function stakes(t: TableView) {
    if (t.game === "tournament") return `Turnier · BB ${int(t.big_blind / 100)}`;
    return `Cash · ${moneyShort(t.big_blind, t.currency)} BB`;
  }

  function open(seat: SeatView, table: TableView) {
    selected = { player: { name: seat.name, net: 0, stats: seat.stats }, table };
  }
</script>

<div class="page">
  {#each tables as t (t.table_key)}
    <section class="panel">
      <header class="head">
        <h2 title={t.title}>{t.title}</h2>
        {#if t.table}
          <span class="dim">{stakes(t.table)}</span>
          <span class="dim"><span class="num">{t.table.seats.length}</span> Spieler</span>
          <span class="faint num">Hand #{t.table.hand_id}</span>
        {/if}
      </header>
      {#if t.table}
        {@const table = t.table}
        <div class="scroll">
          <table class="table">
            <thead>
              <tr>
                <th class="name">Spieler</th>
                <th title="Stack in Big Blinds nach der letzten Hand">Stack</th>
                {#if hasBounty(table)}<th title="Kopfgeld auf diesen Spieler">Bounty</th>{/if}
                {#each COLS as id (id)}
                  <th title={`${STAT_META[id].name}: ${STAT_META[id].help}`} aria-label={STAT_META[id].name}>{STAT_META[id].short}</th>
                {/each}
                {#if hasNotes(table)}<th class="note" title="Deine Notizen und Farb-Labels aus PokerStars">Notiz</th>{/if}
                <th class="act"><span class="sr">Details</span></th>
              </tr>
            </thead>
            <tbody>
              {#each seatsOf(table) as seat (seat.seat)}
                <tr class:hero={seat.is_hero} onclick={() => open(seat, table)}>
                  <td class="name">
                    <span class="who">
                      <Wheel stats={seat.stats} hud={table.hud} size={16} />
                      <span class="n" title={seat.name}>{seat.name}</span>
                      {#if seat.note?.label}
                        <i class="dot" style="--c:{seat.note.label.color}" title={seat.note.label.name || "Label"}></i>
                      {/if}
                      {#if seat.is_hero}<span class="you">du</span>{/if}
                    </span>
                  </td>
                  <td>{int(seat.stack_bb)}<span class="unit">BB</span></td>
                  {#if hasBounty(table)}
                    <td>{seat.bounty === null ? "–" : moneyShort(seat.bounty, table.bounty_currency)}</td>
                  {/if}
                  {#each COLS as id (id)}
                    {@const s = stat(seat.stats, id)}
                    <td
                      class:thin={id !== "hands" && (s?.opp ?? 0) < SMALL_SAMPLE}
                      style={id === "hands" ? "" : `color:${toneColor(table.hud, s, "var(--text-3)")}`}
                      title={id === "hands" || !s ? undefined : `${STAT_META[id].name}: ${s.made} von ${s.opp}`}>{fmt(s)}</td
                    >
                  {/each}
                  {#if hasNotes(table)}
                    {@const note = seat.note}
                    <td class="note" title={note ? [note.label?.name, note.text].filter(Boolean).join(" – ") : undefined}>
                      {note?.text || note?.label?.name || ""}
                    </td>
                  {/if}
                  <td class="act">
                    <button
                      class="btn ghost"
                      onclick={(e) => {
                        e.stopPropagation();
                        open(seat, table);
                      }}>Details</button
                    >
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {:else}
        <p class="wait">Noch keine Hand von diesem Tisch importiert. Die Werte erscheinen nach der ersten Hand.</p>
      {/if}
    </section>
  {:else}
    {#if loaded}
      <section class="panel empty">
        <strong>Kein Tisch offen</strong>
        Sobald du in PokerStars einen Tisch öffnest, stehen hier deine Gegner mit ihren Werten aus dem HUD.
      </section>
    {/if}
  {/each}
</div>

{#if selected}
  <PlayerDetail
    player={selected.player}
    hud={selected.table.hud}
    onClose={() => (selected = null)}
    onHands={(name, id) => onHands(name, id)}
  />
{/if}

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding-top: 20px;
  }
  .panel {
    overflow: hidden;
    max-width: 1080px;
  }
  .head {
    display: flex;
    align-items: baseline;
    flex-wrap: wrap;
    gap: 4px 14px;
    padding: 14px 18px 10px;
    font-size: 13px;
  }
  h2 {
    flex: 0 1 auto;
    min-width: 0;
    max-width: 100%;
    margin: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-num);
    font-size: 16px;
    font-weight: 600;
  }
  .scroll {
    overflow-x: auto;
  }
  .table th {
    padding: 8px 8px;
  }
  .table td {
    padding: 0 8px;
    height: 32px;
    font-size: 13.5px;
  }
  .table td.thin {
    opacity: 0.45;
  }
  th.name {
    width: 1%;
  }
  td.name {
    min-width: 170px;
    max-width: 240px;
  }
  .who {
    display: flex;
    align-items: center;
    gap: 10px;
    font-family: var(--font-ui);
    font-size: 13px;
    min-width: 0;
  }
  .who .n {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .dot {
    flex-shrink: 0;
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: var(--c);
  }
  .table .note {
    text-align: left;
  }
  td.note {
    max-width: 220px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-ui);
    font-size: 12.5px;
    color: var(--text-2);
  }
  .you {
    flex-shrink: 0;
    padding: 0 5px;
    border-radius: 3px;
    font-size: 11px;
    color: var(--text-2);
    box-shadow: inset 0 0 0 1px var(--line-2);
  }
  tbody tr {
    cursor: pointer;
  }
  tr.hero td {
    background: rgba(236, 235, 231, 0.02);
  }
  .act {
    width: 1%;
  }
  td.act {
    padding-right: 10px !important;
  }
  .act .btn {
    height: 26px;
    padding: 0 9px;
    font-family: var(--font-ui);
    font-size: 12.5px;
  }
  .sr {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
  }
  .wait {
    margin: 0;
    padding: 4px 18px 16px;
    color: var(--text-2);
    font-size: 13px;
  }
  .empty strong {
    display: block;
    margin-bottom: 4px;
    color: var(--text);
    font-weight: 600;
  }
</style>
