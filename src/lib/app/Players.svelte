<script lang="ts">
  import { api } from "../api";
  import { STAT_META, fmt, stat, toneColor } from "../stats";
  import type { Filter, HudConfig, PlayerView, StatId } from "../types";
  import { int } from "./format";
  import PlayerDetail from "./PlayerDetail.svelte";
  import Wheel from "./Wheel.svelte";

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

  const COLS: StatId[] = [
    "hands",
    "vpip",
    "pfr",
    "three_bet",
    "fold_to_three_bet",
    "steal",
    "fold_to_steal",
    "cbet",
    "fold_to_cbet",
    "wtsd",
    "wsd",
    "af",
    "afq",
  ];
  const LIMIT = 500;
  /** Below this many chances a value is still noise; it stays visible but recedes. */
  const SMALL_SAMPLE = 20;

  let search = $state("");
  let rows = $state<PlayerView[]>([]);
  let loaded = $state(false);
  let sortBy = $state<StatId | "name">("hands");
  let asc = $state(false);
  let selected = $state<PlayerView | null>(null);

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

  function sortOn(id: StatId | "name") {
    if (sortBy === id) asc = !asc;
    else {
      sortBy = id;
      asc = id === "name";
    }
  }
</script>

<div class="page">
  <section class="panel">
    <div class="panel-head">
      <div class="search">
        <svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true"><path d="M10.5 17a6.5 6.5 0 1 0 0-13 6.5 6.5 0 0 0 0 13zM15.5 15.5 20 20" /></svg>
        <input class="field" type="search" placeholder="Spieler suchen" aria-label="Spieler suchen" bind:value={search} />
      </div>
      <span class="faint count">
        {#if loaded}
          {rows.length >= LIMIT ? `Die ${int(LIMIT)} mit den meisten Händen` : `${int(rows.length)} Spieler`}
        {/if}
      </span>
    </div>

    {#if rows.length && hud}
      <div class="scroll">
        <table class="table">
          <thead>
            <tr>
              <th
                class="sortable name"
                class:sorted={sortBy === "name"}
                class:asc={sortBy === "name" && asc}
                onclick={() => sortOn("name")}
                aria-sort={sortBy === "name" ? (asc ? "ascending" : "descending") : "none"}>Spieler</th
              >
              {#each COLS as id (id)}
                <th
                  class="sortable"
                  class:sorted={sortBy === id}
                  class:asc={sortBy === id && asc}
                  title={`${STAT_META[id].name}: ${STAT_META[id].help}`}
                  aria-label={STAT_META[id].name}
                  onclick={() => sortOn(id)}
                  aria-sort={sortBy === id ? (asc ? "ascending" : "descending") : "none"}>{STAT_META[id].short}</th
                >
              {/each}
            </tr>
          </thead>
          <tbody>
            {#each sorted as p (p.name)}
              <tr class:on={selected?.name === p.name} onclick={() => (selected = p)}>
                <td class="name">
                  <button onclick={(e) => { e.stopPropagation(); selected = p; }}>
                    <Wheel stats={p.stats} {hud} size={16} />
                    <span title={p.name}>{p.name}</span>
                  </button>
                </td>
                {#each COLS as id (id)}
                  {@const s = stat(p.stats, id)}
                  <td
                    class:thin={id !== "hands" && (s?.opp ?? 0) < SMALL_SAMPLE}
                    style={id === "hands" ? "" : `color:${toneColor(hud, s, "var(--text-3)")}`}
                    title={id === "hands" || !s ? undefined : `${STAT_META[id].name}: ${s.made} von ${s.opp}`}>{fmt(s)}</td
                  >
                {/each}
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {:else if loaded}
      <div class="empty-state">
        {search ? `Kein Spieler mit „${search}“ in dieser Auswahl.` : "Noch keine Gegner in diesem Zeitraum. Spiel ein paar Hände oder wähle oben einen längeren Zeitraum."}
      </div>
    {/if}
  </section>
</div>

{#if selected && hud}
  <PlayerDetail
    player={selected}
    {hud}
    onClose={() => (selected = null)}
    onHands={(name, id) => onHands(name, id)}
  />
{/if}

<style>
  .page {
    padding-top: 20px;
  }
  .panel {
    overflow: hidden;
    max-width: 1080px;
  }
  .panel-head {
    padding-bottom: 12px;
  }
  .search {
    position: relative;
    display: flex;
    align-items: center;
  }
  .search svg {
    position: absolute;
    left: 10px;
    fill: none;
    stroke: var(--text-3);
    stroke-width: 1.7;
    stroke-linecap: round;
  }
  .search input {
    width: 260px;
    padding-left: 32px;
  }
  .count {
    font-size: 12.5px;
  }
  .scroll {
    overflow: auto;
    max-height: calc(100vh - 56px - 20px - 48px - 60px);
  }
  .table th {
    padding: 8px 8px;
  }
  .table td {
    padding: 0 8px;
    height: 28px;
    font-size: 13.5px;
  }
  .table td.thin {
    opacity: 0.45;
  }
  .table th.name {
    width: 1%;
    padding-left: 18px;
  }
  td.name {
    padding: 0 !important;
  }
  td.name button {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    min-width: 170px;
    max-width: 240px;
    height: 28px;
    padding: 0 12px 0 18px;
    border: 0;
    background: transparent;
    font-family: var(--font-ui);
    font-size: 13px;
    text-align: left;
    cursor: pointer;
  }
  td.name span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  tbody tr {
    cursor: pointer;
  }
  tbody tr.on td {
    background: var(--surface-2);
  }
</style>
