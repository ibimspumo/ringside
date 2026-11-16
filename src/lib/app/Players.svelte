<script lang="ts">
  import { api } from "../api";
  import { STAT_META, fmt, stat, tone } from "../stats";
  import type { GameKind, HudConfig, PlayerView, StatId } from "../types";
  import GameFilter from "./GameFilter.svelte";

  let { refresh, onHands }: { refresh: number; onHands: (name: string) => void } = $props();

  let game = $state<GameKind | null>("cash");
  let search = $state("");
  let rows = $state<PlayerView[]>([]);
  let hud = $state<HudConfig | null>(null);

  const COLS: StatId[] = ["hands", "vpip", "pfr", "three_bet", "fold_to_three_bet", "steal", "cbet", "fold_to_cbet", "af", "wtsd", "wsd"];

  $effect(() => {
    void refresh;
    const g = game;
    const q = search;
    const t = setTimeout(() => api.players(g, q).then((r) => (rows = r)), 150);
    return () => clearTimeout(t);
  });
  $effect(() => {
    api.getSettings().then((s) => (hud = s.hud));
  });
</script>

<div class="page">
  <div class="page-head">
    <div>
      <h1>Spieler</h1>
      <p>Alle Gegner, gegen die du gespielt hast. Klick auf eine Zeile zeigt die gemeinsamen Hände.</p>
    </div>
    <div class="tools">
      <input class="field" placeholder="Name suchen …" bind:value={search} />
      <GameFilter bind:value={game} />
    </div>
  </div>

  <div class="panel wrap">
    {#if rows.length && hud}
      <table>
        <thead>
          <tr>
            <th class="name">Spieler</th>
            {#each COLS as id (id)}
              <th title={STAT_META[id].help}>{STAT_META[id].short}</th>
            {/each}
          </tr>
        </thead>
        <tbody>
          {#each rows as p (p.name)}
            <tr onclick={() => onHands(p.name)}>
              <td class="name">{p.name}</td>
              {#each COLS as id (id)}
                {@const s = stat(p.stats, id)}
                <td class="num tone-{id === 'hands' ? 'mid' : tone(hud, s)}">{fmt(s)}</td>
              {/each}
            </tr>
          {/each}
        </tbody>
      </table>
    {:else}
      <div class="empty">Keine Spieler gefunden.</div>
    {/if}
  </div>
</div>

<style>
  .tools {
    display: flex;
    gap: 10px;
  }
  .tools input {
    width: 220px;
  }
  .wrap {
    overflow: auto;
  }
  table {
    width: 100%;
    border-collapse: collapse;
  }
  th {
    position: sticky;
    top: 0;
    padding: 12px 10px;
    background: var(--panel);
    color: var(--text-dim);
    font-size: 11.5px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-align: right;
    border-bottom: 1px solid var(--line);
    cursor: help;
  }
  td {
    padding: 9px 10px;
    text-align: right;
    font-size: 15px;
    border-bottom: 1px solid var(--line);
  }
  .name {
    text-align: left;
    padding-left: 18px;
    font-family: var(--font-ui);
    font-size: 14px;
  }
  tbody tr {
    cursor: pointer;
  }
  tbody tr:hover td {
    background: rgba(255, 255, 255, 0.025);
  }
</style>
