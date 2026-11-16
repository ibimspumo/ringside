<script lang="ts">
  import { onMount } from "svelte";
  import { api, on } from "../api";
  import Logo from "../Logo.svelte";
  import type { ImportStatus } from "../types";
  import Dashboard from "./Dashboard.svelte";
  import Hands from "./Hands.svelte";
  import HudEditor from "./HudEditor.svelte";
  import Players from "./Players.svelte";
  import SettingsView from "./SettingsView.svelte";

  type Tab = "dashboard" | "players" | "hands" | "hud" | "settings";
  const NAV: { id: Tab; label: string; icon: string }[] = [
    { id: "dashboard", label: "Übersicht", icon: "M4 13h4v7H4zM10 4h4v16h-4zM16 9h4v11h-4z" },
    { id: "players", label: "Spieler", icon: "M12 12a4 4 0 1 0 0-8 4 4 0 0 0 0 8zm-7 8a7 7 0 0 1 14 0" },
    { id: "hands", label: "Hände & Replay", icon: "M7 4h10l3 4-8 12L4 8z" },
    { id: "hud", label: "HUD anpassen", icon: "M12 3a9 9 0 1 0 9 9M12 7a5 5 0 1 0 5 5" },
    { id: "settings", label: "Einstellungen", icon: "M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM4 12h2M18 12h2M12 4v2M12 18v2" },
  ];

  let tab = $state<Tab>((new URLSearchParams(window.location.search).get("tab") as Tab) ?? "dashboard");
  let status = $state<ImportStatus | null>(null);
  let tables = $state(0);
  let playerFilter = $state<string | null>(null);
  let refresh = $state(0);

  function showPlayerHands(name: string) {
    playerFilter = name;
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
</script>

<div class="shell">
  <aside>
    <div class="brand">
      <Logo size={30} />
      <div>
        <strong>Ringside</strong>
        <span>Poker HUD</span>
      </div>
    </div>
    <nav>
      {#each NAV as item (item.id)}
        <button class:on={tab === item.id} onclick={() => (tab = item.id)}>
          <svg viewBox="0 0 24 24" width="18" height="18"><path d={item.icon} /></svg>
          {item.label}
        </button>
      {/each}
    </nav>
    <footer>
      <div class="live" class:active={tables > 0}>
        <span class="dot"></span>
        {tables === 0 ? "Kein Tisch offen" : tables === 1 ? "1 Tisch im Blick" : `${tables} Tische im Blick`}
      </div>
      <div class="dim num">{(status?.total_hands ?? 0).toLocaleString("de-DE")} Hände gespeichert</div>
      {#if status && status.dirs.length === 0}
        <button class="warn" onclick={() => (tab = "settings")}>Kein Hand-History-Ordner gefunden</button>
      {/if}
    </footer>
  </aside>

  <main>
    {#if tab === "dashboard"}
      <Dashboard {refresh} />
    {:else if tab === "players"}
      <Players {refresh} onHands={showPlayerHands} />
    {:else if tab === "hands"}
      <Hands {refresh} player={playerFilter} onClearPlayer={() => (playerFilter = null)} />
    {:else if tab === "hud"}
      <HudEditor />
    {:else}
      <SettingsView {status} />
    {/if}
  </main>
</div>

<style>
  .shell {
    display: grid;
    grid-template-columns: 232px 1fr;
    height: 100vh;
  }
  aside {
    display: flex;
    flex-direction: column;
    padding: 18px 14px;
    background: linear-gradient(180deg, #0e1117, #0b0d12);
    border-right: 1px solid var(--line);
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 2px 8px 22px;
  }
  .brand strong {
    display: block;
    font-family: var(--font-num);
    font-size: 19px;
    letter-spacing: 0.02em;
  }
  .brand span {
    display: block;
    color: var(--text-faint);
    font-size: 11.5px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
  }
  nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  nav button {
    display: flex;
    align-items: center;
    gap: 11px;
    height: 38px;
    padding: 0 10px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: var(--text-dim);
    cursor: pointer;
    text-align: left;
  }
  nav button:hover {
    color: var(--text);
    background: rgba(255, 255, 255, 0.03);
  }
  nav button.on {
    color: var(--text);
    background: var(--panel-2);
    box-shadow: inset 2px 0 0 var(--brand);
  }
  nav svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 1.8;
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
  }
  .live {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--text-dim);
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--text-faint);
  }
  .live.active {
    color: var(--text);
  }
  .live.active .dot {
    background: var(--win);
    box-shadow: 0 0 0 3px rgba(61, 220, 151, 0.18);
  }
  .dim {
    color: var(--text-faint);
  }
  .warn {
    border: 0;
    background: rgba(255, 91, 58, 0.12);
    color: #ffb19f;
    border-radius: 6px;
    padding: 6px 8px;
    text-align: left;
    cursor: pointer;
  }
  main {
    overflow: auto;
    min-width: 0;
  }
</style>
