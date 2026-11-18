<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "../api";
  import { stat } from "../stats";
  import type { Filter, HudConfig, PlayerView, StatId } from "../types";
  import Button from "../ui/Button.svelte";
  import EmptyState from "../ui/EmptyState.svelte";
  import SearchField from "../ui/SearchField.svelte";
  import SegmentedControl from "../ui/SegmentedControl.svelte";
  import SourceNote from "../ui/SourceNote.svelte";
  import { pokerClient, watchPokerClient } from "./client.svelte";
  import { int } from "./format";
  import MeterLegend from "./MeterLegend.svelte";
  import MoreColumns from "./MoreColumns.svelte";
  import PlayerDetail from "./PlayerDetail.svelte";
  import PlayerTable from "./PlayerTable.svelte";
  import { preview } from "../learn/preview";

  let {
    filter,
    hud,
    refresh,
    onHands,
  }: {
    filter: Filter;
    hud: HudConfig | null;
    refresh: number;
    onHands: (name: string, handId?: number) => void;
  } = $props();

  const LIMIT = 500;
  /** From this width the detail stands next to the list instead of sliding over it. */
  const DOCK_FROM = 1080;

  // Browser demo: `player=<name>` opens a player, `cols=all` all values, `q=<text>` a search.
  let search = $state(preview("q") ?? "");
  let rows = $state<PlayerView[]>([]);
  let loaded = $state(false);
  let sortBy = $state<StatId | "name">("hands");
  let asc = $state(false);
  let picked = $state<string | null>(preview("player"));
  let columns = $state<"core" | "all">(preview("cols") === "all" ? "all" : "core");
  let width = $state(0);
  let more = $state(false);
  /** Your own names: your row gets "du" and no player type. */
  let heroes = $state<string[]>([]);

  onMount(watchPokerClient);
  /** Type and counter only while PokerStars is closed: advice during a session breaks its rules. */
  const advice = $derived(pokerClient.running === false);
  /** All twelve values need the full width; the detail then slides in. */
  const docked = $derived(width >= DOCK_FROM && columns === "core");

  $effect(() => {
    void refresh;
    const f = { ...filter };
    const q = search.trim();
    let live = true;
    const t = setTimeout(
      () =>
        api.players(f, q, LIMIT).then((r) => {
          if (!live) return;
          rows = r;
          loaded = true;
        }),
      q ? 160 : 0,
    );
    return () => {
      live = false;
      clearTimeout(t);
    };
  });

  $effect(() => {
    void refresh;
    let live = true;
    api.dashboard({ ...filter }).then((d) => live && (heroes = d.heroes));
    return () => (live = false);
  });

  const sorted = $derived.by(() => {
    const dir = asc ? 1 : -1;
    if (sortBy === "name") return [...rows].sort((a, b) => dir * a.name.localeCompare(b.name, "de"));
    const id = sortBy;
    return [...rows].sort((a, b) => {
      const x = stat(a.stats, id)?.value ?? null;
      const y = stat(b.stats, id)?.value ?? null;
      if (x === null && y === null) return 0;
      if (x === null) return 1;
      if (y === null) return -1;
      return dir * (x - y) || (stat(b.stats, "hands")?.value ?? 0) - (stat(a.stats, "hands")?.value ?? 0);
    });
  });
  /** Docked, the detail always shows someone: the picked player, else the first of the list. */
  const selected = $derived(rows.find((p) => p.name === picked) ?? (docked ? (sorted[0] ?? null) : null));

  function sortOn(id: StatId | "name") {
    if (sortBy === id) asc = !asc;
    else {
      sortBy = id;
      asc = id === "name";
    }
  }
</script>

<div class="page roomy" bind:clientWidth={width}>
  <div class="split" class:docked>
    <section class="list" aria-label="Spieler">
      <div class="bar">
        <SearchField bind:value={search} label="Spieler suchen" />
        <span class="count">
          {#if loaded}{rows.length >= LIMIT ? `Die ${int(LIMIT)} mit den meisten Händen` : `${int(rows.length)} Spieler`}{/if}
        </span>
        {#if more}<MoreColumns />{/if}
        <SegmentedControl
          label="Spalten"
          size="sm"
          value={columns}
          onchange={(v) => (columns = v)}
          options={[
            { value: "core", label: "Kernwerte", hint: "VPIP, PFR, 3-Bet und Aggression: daran erkennst du den Spielertyp" },
            { value: "all", label: "Alle 12 Werte", hint: "Dazu Steal, C-Bet, Showdown und die Fold-Werte" },
          ]}
        />
      </div>
      {#if pokerClient.running}
        <div class="lock">
          <SourceNote kind="locked" variant="block">Spielertyp und Gegenzug siehst du wieder, sobald du PokerStars schließt. Die Werte bleiben sichtbar.</SourceNote>
        </div>
      {/if}

      {#if rows.length && hud}
          <PlayerTable
            label="Spieler"
            rows={sorted.map((p) => ({ key: p.name, name: p.name, stats: p.stats, hero: heroes.includes(p.name) }))}
            {hud}
            {columns}
            {advice}
            selected={selected?.name ?? null}
            onPick={(k) => (picked = k)}
            sort={{ by: sortBy, asc, on: sortOn }}
            maxHeight="calc(100vh - 290px)"
            bind:more
          />
        <div class="foot"><MeterLegend meters={columns === "core"} types={advice} /></div>
      {:else if loaded}
        <EmptyState
          title={search ? `Kein Spieler mit „${search}“` : "Noch keine Gegner in dieser Auswahl"}
          text={search ? "Prüf die Schreibweise. Gesucht wird in der Auswahl oben rechts." : "Spiel ein paar Hände oder wähle oben rechts einen längeren Zeitraum."}
        >
          {#snippet action()}{#if search}<Button size="sm" onclick={() => (search = "")}>Suche löschen</Button>{/if}{/snippet}
        </EmptyState>
      {:else}
        <EmptyState tone="loading" title="Spieler werden geladen" />
      {/if}
    </section>

    {#if docked && selected && hud}
      <PlayerDetail player={selected} {hud} {advice} hero={heroes.includes(selected.name)} docked onHands={(name, id) => onHands(name, id)} />
    {/if}
  </div>
</div>

{#if !docked && selected && hud}
  <PlayerDetail player={selected} {hud} {advice} hero={heroes.includes(selected.name)} onClose={() => (picked = null)} onHands={(name, id) => onHands(name, id)} />
{/if}

<style>
  .roomy {
    max-width: 1640px;
  }
  .split {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: var(--sp-5);
    align-items: start;
  }
  .split.docked {
    grid-template-columns: minmax(0, 1fr) clamp(380px, 32%, 460px);
  }
  .list {
    container: player-list / inline-size;
    min-width: 0;
    border: 1px solid var(--line);
    border-radius: var(--r-lg);
    background: var(--surface);
    overflow: hidden;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding: var(--sp-3) var(--sp-4);
    border-bottom: 1px solid var(--line);
  }
  .count {
    margin-right: auto;
    color: var(--text-3);
    font-size: var(--fs-sm);
    white-space: nowrap;
  }
  .lock {
    padding: var(--sp-3) var(--sp-4) 0;
  }
  .foot {
    padding: var(--sp-3) var(--sp-4);
    border-top: 1px solid var(--line);
  }
</style>
