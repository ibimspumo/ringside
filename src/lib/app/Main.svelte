<script lang="ts">
  import { onMount } from "svelte";
  import { api, inTauri, on } from "../api";
  import CloudNotice from "../cloud/CloudNotice.svelte";
  import Coach from "../coach/Coach.svelte";
  import Logo from "../Logo.svelte";
  import type { Filter, GameKind, HudConfig, ImportStatus, SpotFilter, TournamentRow } from "../types";
  import Dashboard from "./Dashboard.svelte";
  import { PERIODS, int, since, type Period } from "./format";
  import Hands from "./Hands.svelte";
  import Leaks, { type LearnTarget } from "./Leaks.svelte";
  import HudEditor from "./HudEditor.svelte";
  import LiveTables from "./LiveTables.svelte";
  import Players from "./Players.svelte";
  import Reports from "./Reports.svelte";
  import SettingsView from "./SettingsView.svelte";
  import TournamentPicker from "./TournamentPicker.svelte";
  import Tournaments from "./Tournaments.svelte";
  import Training from "./Training.svelte";
  import PlayPage from "../play/PlayPage.svelte";
  import { playNav } from "../play/nav.svelte";
  import PushFoldCharts from "../charts/PushFoldCharts.svelte";
  import Solver from "../solver/Solver.svelte";
  import Library from "../library/Library.svelte";
  import { handoff } from "../solver/handoff.svelte";
  import { analysis } from "../analysis/store.svelte";
  import JobsDock from "../jobs/JobsDock.svelte";
  import JobToasts from "../jobs/JobToasts.svelte";
  import { jobs } from "../jobs/store.svelte";
  import type { JobTarget } from "../jobs/types";
  import { openSaved } from "../solver/solutions";
  import { solver } from "../solver/store.svelte";

  type Tab =
    | "dashboard"
    | "reports"
    | "leaks"
    | "tournaments"
    | "hands"
    | "coach"
    | "players"
    | "live"
    | "training"
    | "play"
    | "charts"
    | "solver"
    | "library"
    | "hud"
    | "settings";
  type NavItem = { id: Tab; label: string; icon: string };
  const GROUPS: { label: string; items: NavItem[] }[] = [
    {
      label: "Analyse",
      items: [
        { id: "dashboard", label: "Übersicht", icon: "M4 19.5h16M5 15l4.5-4.5 3 3L19 7" },
        { id: "reports", label: "Berichte", icon: "M4.5 5h15v14h-15zM4.5 10h15M4.5 14.5h15M10 10v9" },
        { id: "tournaments", label: "Turniere", icon: "M8 4.5h8V9a4 4 0 0 1-8 0zM8 6.5H5.5v1a3 3 0 0 0 2.7 3M16 6.5h2.5v1a3 3 0 0 1-2.7 3M12 13v3.5M8.5 19.5h7M10 16.5h4" },
        { id: "leaks", label: "Leaks", icon: "M12 4.5c3 4 5.5 6.9 5.5 10a5.5 5.5 0 0 1-11 0c0-3.1 2.5-6 5.5-10zM9.5 15.5a2.5 2.5 0 0 0 2.5 2.5" },
        { id: "hands", label: "Hände", icon: "M10 4h8a1 1 0 0 1 1 1v11a1 1 0 0 1-1 1h-8a1 1 0 0 1-1-1V5a1 1 0 0 1 1-1zM6 7.5V19a1 1 0 0 0 1 1h8" },
        { id: "coach", label: "Coach", icon: "M4.5 5.5h15v10.5h-8l-4.5 3.5v-3.5H4.5zM8.5 9.5h7M8.5 12.5h4.5" },
      ],
    },
    {
      label: "Gegner",
      items: [
        { id: "players", label: "Spieler", icon: "M12 11.5a3.75 3.75 0 1 0 0-7.5 3.75 3.75 0 0 0 0 7.5zM5 20a7 7 0 0 1 14 0" },
        { id: "live", label: "Live-Tische", icon: "M12 6.5c4.7 0 8.5 2.5 8.5 5.5s-3.8 5.5-8.5 5.5S3.5 15 3.5 12 7.3 6.5 12 6.5zM12 9.5c2.8 0 5 1.1 5 2.5s-2.2 2.5-5 2.5-5-1.1-5-2.5 2.2-2.5 5-2.5z" },
      ],
    },
    {
      label: "Lernen",
      items: [
        { id: "training", label: "Training", icon: "M12 5 3.5 9l8.5 4 8.5-4zM7 10.7v4.3c0 1.3 2.2 2.8 5 2.8s5-1.5 5-2.8v-4.3M20.5 9v5" },
        { id: "play", label: "Hände spielen", icon: "M12 5.5c4.4 0 8 2.9 8 6.5s-3.6 6.5-8 6.5-8-2.9-8-6.5 3.6-6.5 8-6.5zM9 10h2.6v4H9zM12.4 10H15v4h-2.6z" },
        { id: "charts", label: "Charts", icon: "M4.5 4.5h15v15h-15zM4.5 9.5h15M4.5 14.5h15M9.5 4.5v15M14.5 4.5v15" },
        { id: "solver", label: "Solver", icon: "M5 19.5V13M10 19.5V8.5M15 19.5v-8M20 19.5V4.5M3.5 19.5h17" },
        { id: "library", label: "Bibliothek", icon: "M5 4.5h3v15H5zM10 4.5h3v15h-3zM15.2 5.3l2.9-.8 3.6 14.4-2.9.8z" },
      ],
    },
    {
      label: "Einrichten",
      items: [
        { id: "hud", label: "HUD", icon: "" },
        { id: "settings", label: "Einstellungen", icon: "M4 7.5h9M17 7.5h3M4 16.5h3M11 16.5h9M15 5.5v4M9 14.5v4" },
      ],
    },
  ];
  const NAV = GROUPS.flatMap((g) => g.items);
  /** Tabs that read hand data get the period filter in the top bar, most also the game filter. */
  const DATA_TABS: Tab[] = ["dashboard", "reports", "leaks", "tournaments", "players", "hands"];
  const NOTES: Partial<Record<Tab, string>> = {
    tournaments: "Klick auf ein Turnier filtert alle Ansichten darauf und zeigt seine Hände.",
    live: "Deine Gegner an den offenen Tischen, mit den Werten aus dem HUD.",
    hud: "Panels direkt auf dem Tisch ziehen. Änderungen gelten sofort an allen offenen Tischen.",
    settings:
      "Ringside arbeitet lokal. Ins Internet geht es nur für Updates, eine Coach-Analyse und Ringside Cloud (abschaltbar, siehe unten).",
    leaks: "Deine Werte je Position und Stacktiefe gegen veröffentlichte Richtwerte.",
    coach: "Ein Sprachmodell bespricht deine Turniere. Jede Zahl darin rechnet Ringside selbst.",
    training: "Kurze Runden, deine eigenen Hände und Wissen zum Nachlesen. Gesperrt, solange PokerStars läuft.",
    play: "Gesperrt, solange PokerStars läuft. Postflop-Spots rechnet Ringside im Hintergrund vor.",
    solver: "Gesperrt, solange PokerStars läuft. Gerechnet wird nur auf diesem Rechner.",
    library: "Gesperrt, solange PokerStars läuft. Gespeichert auf diesem Rechner, gerechnet hier oder geladen aus Ringside Cloud.",
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
  /** One tournament for every data view; game and period do not apply while it is set. */
  let tournament = $state<number | null>(null);
  const filter = $derived<Filter>(
    tournament === null
      ? { game, since: since(period), tournament_id: null }
      : { game: null, since: null, tournament_id: tournament },
  );
  const tournamentFilter = $derived<Filter>({
    game: "tournament",
    since: tournament === null ? since(period) : null,
    tournament_id: null,
  });
  /** All tournaments, for the picker and for names. */
  let tournamentList = $state<TournamentRow[]>([]);

  /** Hands tab: optional player filter and a hand to open, set from other views. */
  let handsPlayer = $state<string | null>(null);
  let handToOpen = $state<number | null>(null);
  /** Coach tab: a tournament to analyse, set from the tournaments page. */
  let analyzeTournament = $state<number | null>(null);

  // "Diesen Spot lösen" in the replayer hands a spot over; the solver page takes it.
  $effect(() => {
    if (handoff.prefill) tab = "solver";
  });
  // The practice asks to be opened (from the learning area) or hands its spot to the solver.
  let playSeen = { requested: playNav.requested, solver: playNav.solver };
  $effect(() => {
    if (playNav.requested !== playSeen.requested) tab = "play";
    if (playNav.solver !== playSeen.solver) tab = "solver";
    playSeen = { requested: playNav.requested, solver: playNav.solver };
  });
  // "Diese Hand analysieren" on another page opens the hand here; the hands page analyses it.
  $effect(() => {
    const id = analysis.request;
    if (id !== null) showHands(null, id);
  });
  /** Hands tab: a leak-finder spot to list. */
  let handsSpot = $state<{ filter: SpotFilter; label: string } | null>(null);
  /** Training tab: an article or exercise to open, set from the leak finder. */
  let learnOpen = $state<LearnTarget | null>(null);

  try {
    const saved = JSON.parse(localStorage.getItem(STORE) ?? "null");
    if (saved && GAMES.some(([g]) => g === saved.game)) game = saved.game;
    if (saved && PERIODS.some(([p]) => p === saved.period)) period = saved.period;
    if (saved && Number.isSafeInteger(saved.tournament)) tournament = saved.tournament;
  } catch {
    // Storage may be unavailable; the defaults are fine.
  }

  $effect(() => {
    const value = JSON.stringify({ game, period, tournament });
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

  $effect(() => {
    void refresh;
    let live = true;
    api.tournaments({ game: "tournament", since: null, tournament_id: null }).then((t) => live && (tournamentList = t.rows));
    return () => (live = false);
  });

  const tournamentNames = $derived(new Map(tournamentList.filter((t) => t.name).map((t) => [t.id, t.name as string])));

  // A task that finishes while another page is open gets a note with "Ansehen".
  $effect(() => {
    jobs.page = tab;
  });

  /** "Ansehen" in the task center: the page of the task, and what it made. */
  function viewTarget(t: JobTarget) {
    const page = NAV.find((n) => n.id === t.page)?.id;
    if (t.page === "hands") return showHands(null, t.id);
    if (t.page === "solver") {
      if (t.id !== null && t.id !== solver.savedId) openSaved(t.id).catch(() => {});
      else if (solver.status === "solved") solver.view = "result";
    }
    if (page) tab = page;
  }

  function showHands(player: string | null, handId: number | null = null) {
    handsPlayer = player;
    handsSpot = null;
    handToOpen = handId;
    tab = "hands";
  }

  /** Game and period stay visible while a tournament is chosen; picking one of them drops it. */
  function setGame(g: GameKind | null) {
    game = g;
    tournament = null;
  }

  function setPeriod(p: Period) {
    period = p;
    tournament = null;
  }

  /** Current state from the backend; events only report changes. */
  function sync() {
    api.importStatus().then((s) => (status = s));
    api.clientWindows().then((w) => (tables = w.filter((x) => x.table_key).length));
  }

  async function chooseFolder() {
    if (!inTauri) return;
    const { open } = await import("@tauri-apps/plugin-dialog");
    const dir = await open({ directory: true, title: "Hand-History-Ordner wählen" });
    if (typeof dir !== "string") return;
    const settings = await api.getSettings();
    if (!settings.hand_history_dirs.includes(dir)) {
      settings.hand_history_dirs = [...settings.hand_history_dirs, dir];
      await api.saveSettings(settings);
    }
    sync();
  }

  onMount(() => {
    document.body.className = "app";
    const subs = [
      on<ImportStatus>("import-status", (s) => {
        status = s;
        refresh++;
      }),
      on<number>("tables-tracked", (n) => (tables = n)),
      on("tournament-named", () => refresh++),
    ];
    // Listen first, then read the state, so nothing sent at startup is missed.
    Promise.all(subs).then(sync);
    const poll = setInterval(sync, 5000);
    return () => {
      clearInterval(poll);
      subs.forEach((s) => s.then((f) => f()));
    };
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
      {#each GROUPS as group (group.label)}
        <div class="group" role="group" aria-labelledby="nav-{group.label}">
          <span class="group-label" id="nav-{group.label}">{group.label}</span>
          {#each group.items as item (item.id)}
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
        </div>
      {/each}
    </nav>

    <footer>
      <JobsDock onView={viewTarget} />
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
      {#if status && !upgrade && status.dirs.length === 0}
        <div class="warn">
          <strong>PokerStars speichert noch keine Hände</strong>
          <span>In PokerStars unter Einstellungen → Spielhistorie das Speichern der Hand-Historys einschalten. Ringside findet den Ordner dann von selbst.</span>
          <button onclick={chooseFolder}>Ordner selbst wählen</button>
        </div>
      {/if}
      <div class="live" class:active={tables > 0}>
        <span class="dot"></span>
        {tables === 0 ? "Kein Tisch offen" : tables === 1 ? "1 Tisch im Blick" : `${tables} Tische im Blick`}
      </div>
      <div class="stored"><span class="num">{int(status?.total_hands ?? 0)}</span> Hände gespeichert</div>
    </footer>
  </aside>

  <main>
    <CloudNotice onSettings={() => (tab = "settings")} />
    <header class="top">
      <div class="title">
        <h1>{NAV.find((n) => n.id === tab)?.label}</h1>
        {#if NOTES[tab]}<p>{NOTES[tab]}</p>{/if}
      </div>
      {#if DATA_TABS.includes(tab)}
        {@const off = tournament !== null}
        {@const offTitle = off ? "Gilt nicht, solange ein Turnier gewählt ist. Klick hebt die Turnierauswahl auf." : undefined}
        <div class="filters">
          <TournamentPicker tournaments={tournamentList} selected={tournament} onSelect={(id) => (tournament = id)} />
          {#if tab !== "tournaments"}
            <div class="seg" class:off role="group" aria-label="Spielart" title={offTitle}>
              {#each GAMES as [id, label] (label)}
                <button class:on={!off && game === id} onclick={() => setGame(id)}>{label}</button>
              {/each}
            </div>
          {/if}
          <div class="seg" class:off role="group" aria-label="Zeitraum" title={offTitle}>
            {#each PERIODS as [id, label] (id)}
              <button class:on={!off && period === id} onclick={() => setPeriod(id)}>{label}</button>
            {/each}
          </div>
        </div>
      {/if}
    </header>

    <div class="content" class:fill={tab === "hands"}>
      {#if tab === "dashboard"}
        <Dashboard
          {filter}
          {hud}
          {refresh}
          onHand={(id) => showHands(null, id >= 0 ? id : null)}
          onLearn={(target) => {
            learnOpen = target;
            tab = "training";
          }}
        />
      {:else if tab === "reports"}
        <Reports {filter} {refresh} />
      {:else if tab === "leaks"}
        <Leaks
          {filter}
          {refresh}
          onHands={(spot, label) => {
            showHands(null);
            handsSpot = { filter: spot, label };
          }}
          onLearn={(target) => {
            learnOpen = target;
            tab = "training";
          }}
        />
      {:else if tab === "tournaments"}
        <Tournaments
          filter={tournamentFilter}
          {refresh}
          selected={tournament}
          onHands={(id) => {
            tournament = id;
            showHands(null);
          }}
          onAnalyze={(id) => {
            analyzeTournament = id;
            tab = "coach";
          }}
        />
      {:else if tab === "coach"}
        <Coach
          analyze={analyzeTournament}
          tournaments={tournamentList}
          onAnalyzeHandled={() => (analyzeTournament = null)}
          onHand={(t, id) => {
            tournament = t;
            showHands(null, id);
          }}
          onTraining={(target) => {
            learnOpen = target;
            tab = "training";
          }}
        />
      {:else if tab === "players"}
        <Players {filter} {hud} {refresh} onHands={(name, id) => showHands(name, id ?? null)} />
      {:else if tab === "live"}
        <LiveTables onHands={(name, id) => showHands(name, id ?? null)} />
      {:else if tab === "training"}
        <Training {hud} open={learnOpen} onOpened={() => (learnOpen = null)} />
      {:else if tab === "play"}
        <PlayPage />
      {:else if tab === "charts"}
        <PushFoldCharts />
      {:else if tab === "solver"}
        <Solver />
      {:else if tab === "library"}
        <Library onOpen={() => (tab = "solver")} />
      {:else if tab === "hands"}
        <Hands
          {filter}
          {refresh}
          player={handsPlayer}
          {tournamentNames}
          openId={handToOpen}
          spot={handsSpot}
          onClearSpot={() => (handsSpot = null)}
          onClearPlayer={() => (handsPlayer = null)}
          onClearTournament={() => (tournament = null)}
          onLearn={(target) => {
            learnOpen = target;
            tab = "training";
          }}
        />
      {:else if tab === "hud"}
        <HudEditor />
      {:else}
        <SettingsView {status} />
      {/if}
    </div>
  </main>
  <JobToasts onView={viewTarget} />
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
    gap: 14px;
    min-height: 0;
    overflow-y: auto;
  }
  /* Low windows: the whole menu stays visible above the footer. */
  @media (max-height: 760px) {
    .brand {
      margin-bottom: 10px;
    }
    nav {
      gap: 8px;
    }
    nav button {
      height: 30px;
    }
  }
  .group {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .group-label {
    padding: 0 10px 4px;
    font-size: 11.5px;
    font-weight: 600;
    letter-spacing: 0.02em;
    color: var(--text-3);
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
    display: grid;
    gap: 5px;
    margin-bottom: 4px;
    padding: 8px 9px;
    border: 1px solid var(--line-2);
    border-radius: var(--r-sm);
    background: var(--surface-2);
    color: var(--text-2);
    font-size: 12px;
    line-height: 1.4;
  }
  .warn strong {
    color: var(--text);
    font-weight: 600;
  }
  .warn button {
    justify-self: start;
    padding: 3px 8px;
    border: 1px solid var(--line-2);
    border-radius: var(--r-sm);
    background: var(--surface-3);
    color: var(--text);
    font-size: 12px;
    cursor: pointer;
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
    align-items: center;
    gap: 8px;
  }
  .seg.off {
    opacity: 0.45;
  }
  .seg.off:hover {
    opacity: 0.8;
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
  /* Short windows (700 px at the minimum size): every nav item stays visible above the footer. */
  @media (max-height: 760px) {
    nav {
      gap: 10px;
    }
    nav button {
      height: 30px;
    }
  }
</style>
