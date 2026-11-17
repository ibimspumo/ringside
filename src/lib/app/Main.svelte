<script lang="ts">
  import { onMount } from "svelte";
  import { api, on } from "../api";
  import Logo from "../Logo.svelte";
  import type { Filter, GameKind, HudConfig, ImportStatus } from "../types";
  import Dashboard from "./Dashboard.svelte";
  import { PERIODS, int, since, type Period } from "./format";
  import Hands from "./Hands.svelte";
  import HudEditor from "./HudEditor.svelte";
  import Players from "./Players.svelte";
  import Reports from "./Reports.svelte";
  import SettingsView from "./SettingsView.svelte";

  type Tab = "dashboard" | "reports" | "players" | "hands" | "hud" | "settings";
  const NAV: { id: Tab; label: string; icon: string }[] = [
    { id: "dashboard", label: "Übersicht", icon: "M4 19.5h16M5 15l4.5-4.5 3 3L19 7" },
    { id: "reports", label: "Berichte", icon: "M4.5 5h15v14h-15zM4.5 10h15M4.5 14.5h15M10 10v9" },
    { id: "players", label: "Spieler", icon: "M12 11.5a3.75 3.75 0 1 0 0-7.5 3.75 3.75 0 0 0 0 7.5zM5 20a7 7 0 0 1 14 0" },
    { id: "hands", label: "Hände", icon: "M10 4h8a1 1 0 0 1 1 1v11a1 1 0 0 1-1 1h-8a1 1 0 0 1-1-1V5a1 1 0 0 1 1-1zM6 7.5V19a1 1 0 0 0 1 1h8" },
    { id: "hud", label: "HUD", icon: "" },
    { id: "settings", label: "Einstellungen", icon: "M4 7.5h9M17 7.5h3M4 16.5h3M11 16.5h9M15 5.5v4M9 14.5v4" },
  ];
  /** Tabs that read hand data get the game and period filters in the top bar. */
  const DATA_TABS: Tab[] = ["dashboard", "reports", "players", "hands"];
  const NOTES: Partial<Record<Tab, string>> = {
    hud: "Panels direkt auf dem Tisch ziehen. Änderungen gelten sofort an allen offenen Tischen.",
    settings: "Ringside arbeitet komplett lokal. Ins Internet geht es nur, wenn du nach Updates suchst.",
  };
  const GAMES: [GameKind | null, string][] = [
    ["tournament", "Turniere"],
    ["cash", "Cash"],
    [null, "Alle"],
  ];
  const STORE = "ringside.filter";

  const initialTab = new URLSearchParams(window.location.search).get("tab") as Tab | null;
  let tab = $state<Tab>(initialTab && NAV.some((n) => n.id === initialTab) ? initialTab : "dashboard");
  let status = $state<ImportStatus | null>(null);
  let tables = $state(0);
  let refresh = $state(0);
  let hud = $state<HudConfig | null>(null);

  let game = $state<GameKind | null>(null);
  let period = $state<Period>("all");
  const filter = $derived<Filter>({ game, since: since(period) });

  /** Hands tab: optional player filter and a hand to open, set from the player drawer. */
  let handsPlayer = $state<string | null>(null);
  let handToOpen = $state<number | null>(null);

  try {
    const saved = JSON.parse(localStorage.getItem(STORE) ?? "null");
    if (saved && GAMES.some(([g]) => g === saved.game)) game = saved.game;
    if (saved && PERIODS.some(([p]) => p === saved.period)) period = saved.period;
  } catch {
    // Storage may be unavailable; the defaults are fine.
  }

  $effect(() => {
    const value = JSON.stringify({ game, period });
    try {
      localStorage.setItem(STORE, value);
    } catch {
      // ignore
    }
  });

  // Settings may change in the HUD tab; reload them whenever a data view opens.
  $effect(() => {
    if (tab === "hud" || tab === "settings") return;
    api.getSettings().then((s) => (hud = s.hud));
  });

  function showHands(player: string | null, handId: number | null = null) {
    handsPlayer = player;
    handToOpen = handId;
    tab = "hands";
  }

  onMount(() => {
    document.body.className = "app";
    api.importStatus().then((s) => (status = s));
    api.clientWindows().then((w) => (tables = w.filter((x) => x.table_key).length));
    const subs = [
      on<ImportStatus>("import-status", (s) => {
        status = s;
        refresh++;
      }),
      on<number>("tables-tracked", (n) => (tables = n)),
    ];
    return () => subs.forEach((s) => s.then((f) => f()));
  });

  const upgrade = $derived(status?.upgrade ?? null);
</script>

<div class="shell">
  <aside>
    <div class="brand">
      <Logo size={26} />
      <strong>Ringside</strong>
    </div>
    <nav>
      {#each NAV as item (item.id)}
        <button class:on={tab === item.id} aria-current={tab === item.id ? "page" : undefined} onclick={() => (tab = item.id)}>
          <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
            {#if item.id === "hud"}
              <circle cx="12" cy="12" r="7.5" stroke-dasharray="9.8 2" stroke-dashoffset="-1" stroke-linecap="butt" />
            {:else}
              <path d={item.icon} />
            {/if}
          </svg>
          {item.label}
        </button>
      {/each}
    </nav>

    <footer>
      {#if upgrade}
        <div class="upgrade">
          <div class="row">
            <span>Datenbank wird aktualisiert</span>
            <span class="num">{upgrade.total ? Math.floor((upgrade.done / upgrade.total) * 100) : 0} %</span>
          </div>
          <div class="bar"><i style="width:{upgrade.total ? (upgrade.done / upgrade.total) * 100 : 0}%"></i></div>
          <small class="num">{int(upgrade.done)} von {int(upgrade.total)} Händen</small>
        </div>
      {/if}
      {#if status && status.dirs.length === 0}
        <button class="warn" onclick={() => (tab = "settings")}>Kein Hand-History-Ordner gefunden. Ordner wählen</button>
      {/if}
      <div class="live" class:active={tables > 0}>
        <span class="dot"></span>
        {tables === 0 ? "Kein Tisch offen" : tables === 1 ? "1 Tisch im Blick" : `${tables} Tische im Blick`}
      </div>
      <div class="stored"><span class="num">{int(status?.total_hands ?? 0)}</span> Hände gespeichert</div>
    </footer>
  </aside>

  <main>
    <header class="top">
      <div class="title">
        <h1>{NAV.find((n) => n.id === tab)?.label}</h1>
        {#if NOTES[tab]}<p>{NOTES[tab]}</p>{/if}
      </div>
      {#if DATA_TABS.includes(tab)}
        <div class="filters">
          <div class="seg" role="group" aria-label="Spielart">
            {#each GAMES as [id, label] (label)}
              <button class:on={game === id} onclick={() => (game = id)}>{label}</button>
            {/each}
          </div>
          <div class="seg" role="group" aria-label="Zeitraum">
            {#each PERIODS as [id, label] (id)}
              <button class:on={period === id} onclick={() => (period = id)}>{label}</button>
            {/each}
          </div>
        </div>
      {/if}
    </header>

    <div class="content" class:fill={tab === "hands"}>
      {#if tab === "dashboard"}
        <Dashboard {filter} {hud} {refresh} />
      {:else if tab === "reports"}
        <Reports {filter} {refresh} />
      {:else if tab === "players"}
        <Players {filter} {hud} {refresh} onHands={(name, id) => showHands(name, id ?? null)} />
      {:else if tab === "hands"}
        <Hands
          {filter}
          {refresh}
          player={handsPlayer}
          openId={handToOpen}
          onClearPlayer={() => (handsPlayer = null)}
        />
      {:else if tab === "hud"}
        <HudEditor />
      {:else}
        <SettingsView {status} />
      {/if}
    </div>
  </main>
</div>

<style>
  .shell {
    display: grid;
    grid-template-columns: 200px minmax(0, 1fr);
    height: 100vh;
  }
  aside {
    display: flex;
    flex-direction: column;
    padding: 16px 10px 14px;
    background: #0f1114;
    border-right: 1px solid var(--line);
    min-height: 0;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 32px;
    padding: 0 8px;
    margin-bottom: 18px;
  }
  .brand strong {
    font-family: var(--font-num);
    font-size: 17px;
    font-weight: 600;
    letter-spacing: 0.02em;
  }
  nav {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  nav button {
    display: flex;
    align-items: center;
    gap: 11px;
    height: 34px;
    padding: 0 10px;
    border: 0;
    border-radius: var(--r-sm);
    background: transparent;
    color: var(--text-2);
    font-size: 13.5px;
    cursor: pointer;
    text-align: left;
  }
  nav button:hover {
    color: var(--text);
    background: var(--surface);
  }
  nav button.on {
    color: var(--text);
    background: var(--surface-2);
  }
  nav svg {
    flex-shrink: 0;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.6;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  footer {
    margin-top: auto;
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 12px 10px 0;
    border-top: 1px solid var(--line);
    font-size: 12.5px;
    color: var(--text-2);
  }
  .live {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    border: 1.5px solid var(--text-3);
  }
  .live.active {
    color: var(--text);
  }
  .live.active .dot {
    background: var(--text);
    border-color: var(--text);
  }
  .stored .num {
    color: var(--text);
  }
  .upgrade {
    display: flex;
    flex-direction: column;
    gap: 5px;
    padding-bottom: 10px;
    margin-bottom: 4px;
    border-bottom: 1px solid var(--line);
  }
  .upgrade .row {
    display: flex;
    justify-content: space-between;
    color: var(--text);
  }
  .upgrade small {
    color: var(--text-3);
    font-size: 11.5px;
  }
  .bar {
    height: 4px;
    border-radius: 2px;
    background: var(--surface-3);
    overflow: hidden;
  }
  .bar i {
    display: block;
    height: 100%;
    background: var(--text);
    transition: width 0.3s;
  }
  .warn {
    margin-bottom: 4px;
    padding: 7px 9px;
    border: 1px solid var(--line-2);
    border-radius: var(--r-sm);
    background: var(--surface-2);
    color: var(--text);
    font-size: 12.5px;
    line-height: 1.35;
    text-align: left;
    cursor: pointer;
  }
  .warn:hover {
    background: var(--surface-3);
  }

  main {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }
  .top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 10px 20px;
    min-height: 56px;
    padding: 10px 24px;
    border-bottom: 1px solid var(--line);
    background: var(--bg);
  }
  .title {
    display: flex;
    align-items: baseline;
    gap: 14px;
    min-width: 0;
  }
  .top h1 {
    margin: 0;
    font-family: var(--font-num);
    font-size: 20px;
    font-weight: 600;
    white-space: nowrap;
  }
  .title p {
    margin: 0;
    font-size: 13px;
    color: var(--text-2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .filters {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .content {
    flex: 1;
    min-height: 0;
    overflow: auto;
    container-type: inline-size;
  }
  .content.fill {
    overflow: hidden;
  }
</style>
