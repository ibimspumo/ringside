<script lang="ts">
  import { onMount } from "svelte";
  import { api, on } from "../api";
  import { moneyShort } from "../stats";
  import type { LiveTable, PlayerView, SeatView, TableView } from "../types";
  import Badge from "../ui/Badge.svelte";
  import EmptyState from "../ui/EmptyState.svelte";
  import SegmentedControl from "../ui/SegmentedControl.svelte";
  import { pokerClient, watchPokerClient } from "./client.svelte";
  import { int } from "./format";
  import MeterLegend from "./MeterLegend.svelte";
  import PlayerDetail from "./PlayerDetail.svelte";
  import PlayerTable, { type ExtraColumn, type PlayerRow } from "./PlayerTable.svelte";
  import { preview } from "../learn/preview";

  let { onHands }: { onHands: (name: string, handId?: number) => void } = $props();

  let tables = $state<LiveTable[]>([]);
  let loaded = $state(false);
  // Browser demo: `cols=all` all values, `player=<name>` opens that player at the first table.
  let columns = $state<"core" | "all">(preview("cols") === "all" ? "all" : "core");
  let selected = $state<{ key: string; player: PlayerView; table: TableView; hero: boolean } | null>(null);

  function load() {
    api.liveTables().then((t) => {
      tables = t;
      const want = loaded ? null : preview("player");
      const first = t.find((x) => x.table)?.table;
      const seat = first?.seats.find((x) => x.name === want);
      if (first && seat) pick(first, rowKey(first, seat));
      loaded = true;
    });
  }

  onMount(() => {
    watchPokerClient();
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
  const rowKey = (t: TableView, s: SeatView) => `${t.table_key}:${s.seat}`;

  function rowsOf(t: TableView): PlayerRow[] {
    return seatsOf(t).map((s) => ({ key: rowKey(t, s), name: s.name, stats: s.stats, label: s.note?.label ?? null, hero: s.is_hero }));
  }

  function extraOf(t: TableView): ExtraColumn[] {
    const seat = (r: PlayerRow) => t.seats.find((s) => rowKey(t, s) === r.key);
    const cols: ExtraColumn[] = [{ key: "stack", head: "Stack", help: "Stack in Big Blinds nach der letzten Hand", at: "before", unit: "BB", text: (r) => int(seat(r)?.stack_bb ?? 0) }];
    if (t.seats.some((s) => s.bounty !== null))
      cols.push({ key: "bounty", head: "Bounty", help: "Kopfgeld auf diesen Spieler", at: "before", text: (r) => { const b = seat(r)?.bounty ?? null; return b === null ? "–" : moneyShort(b, t.bounty_currency); } });
    if (t.seats.some((s) => s.note?.text || s.note?.label))
      cols.push({ key: "note", head: "Deine Notiz", at: "after", wide: true, text: (r) => { const n = seat(r)?.note; return n ? [n.label?.name, n.text].filter(Boolean).join(" – ") : ""; } });
    return cols;
  }

  function stakes(t: TableView) {
    if (t.game === "tournament") return `Turnier · BB ${int(t.big_blind / 100)}`;
    return `Cash · ${moneyShort(t.big_blind, t.currency)} BB`;
  }

  function pick(t: TableView, key: string) {
    const s = t.seats.find((x) => rowKey(t, x) === key);
    if (s) selected = { key, player: { name: s.name, net: 0, stats: s.stats }, table: t, hero: s.is_hero };
  }
</script>

<div class="page roomy">
  {#if tables.some((t) => t.table)}
    <div class="tools">
      <MeterLegend />
      <SegmentedControl
        label="Spalten"
        size="sm"
        value={columns}
        onchange={(v) => (columns = v)}
        options={[
          { value: "core", label: "Kernwerte", hint: "VPIP, PFR, 3-Bet und Aggression" },
          { value: "all", label: "Alle 12 Werte", hint: "Dazu Steal, C-Bet, Showdown und die Fold-Werte" },
        ]}
      />
    </div>
  {/if}

  {#each tables as t (t.table_key)}
    <section class="table-card" aria-label={t.title}>
      <header>
        <h2 title={t.title}>{t.title}</h2>
        {#if t.table}
          <span class="facts">
            <Badge size="sm" tone={t.table.game === "tournament" ? "info" : "neutral"}>{stakes(t.table)}</Badge>
            <span><span class="num">{t.table.seats.length}</span> Spieler</span>
            <span class="faint num">Hand #{t.table.hand_id}</span>
          </span>
        {/if}
      </header>
      {#if t.table}
        {@const table = t.table}
        <div class="scroll">
          <PlayerTable
            label={`Spieler an ${t.title}`}
            rows={rowsOf(table)}
            hud={table.hud}
            {columns}
            extra={extraOf(table)}
            selected={selected?.key ?? null}
            onPick={(k) => pick(table, k)}
          />
        </div>
      {:else}
        <div class="wait"><EmptyState inline tone="loading" title="Warte auf die erste Hand" text="Die Werte erscheinen, sobald Ringside die erste Hand von diesem Tisch gelesen hat." /></div>
      {/if}
    </section>
  {:else}
    {#if loaded}
      <EmptyState title="Kein Tisch offen" text="Sobald du in PokerStars einen Tisch öffnest, stehen hier deine Gegner mit ihren Werten aus dem HUD." />
    {:else}
      <EmptyState tone="loading" title="Suche offene Tische" />
    {/if}
  {/each}
</div>

{#if selected}
  <PlayerDetail
    player={selected.player}
    hud={selected.table.hud}
    advice={pokerClient.running === false}
    hero={selected.hero}
    onClose={() => (selected = null)}
    onHands={(name, id) => onHands(name, id)}
  />
{/if}

<style>
  .roomy {
    container: player-list / inline-size;
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    max-width: 1640px;
  }
  .tools {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-4);
  }
  .table-card {
    border: 1px solid var(--line);
    border-radius: var(--r-lg);
    background: var(--surface);
    overflow: hidden;
  }
  header {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: var(--sp-1) var(--sp-4);
    padding: var(--sp-3) var(--sp-4);
    border-bottom: 1px solid var(--line);
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
    font-size: var(--fs-lg);
    font-weight: var(--fw-semibold);
  }
  .facts {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-3);
    color: var(--text-2);
    font-size: var(--fs-md);
    white-space: nowrap;
  }
  .scroll {
    overflow-x: auto;
  }
  .wait {
    padding: var(--sp-3) var(--sp-4);
  }
</style>
