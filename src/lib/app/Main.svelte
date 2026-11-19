<script lang="ts">
  import { onMount } from "svelte";
  import { api, inTauri, on } from "../api";
  import CloudNotice from "../cloud/CloudNotice.svelte";
  import Coach from "../coach/Coach.svelte";
  import type { Filter, GameKind, HudConfig, ImportStatus, SpotFilter, TournamentRow } from "../types";
  import AtTable from "./AtTable.svelte";
  import ClientLock from "./ClientLock.svelte";
  import { adviceLocked, pokerClient, watchPokerClient } from "./client.svelte";
  import Dashboard from "./Dashboard.svelte";
  import { PERIODS, int, since, type Period } from "./format";
  import Hands from "./Hands.svelte";
  import Leaks, { type LearnTarget } from "./Leaks.svelte";
  import type { StepHands } from "../next/types";
  import HudEditor from "./HudEditor.svelte";
  import LiveTables from "./LiveTables.svelte";
  import Players from "./Players.svelte";
  import Reports from "./Reports.svelte";
  import SettingsView from "./SettingsView.svelte";
  import TournamentPicker from "./TournamentPicker.svelte";
  import Tournaments from "./Tournaments.svelte";
  import Training from "./Training.svelte";
  import TournamentReport from "../report/TournamentReport.svelte";
  import PlayPage from "../play/PlayPage.svelte";
  import { playNav } from "../play/nav.svelte";
  import RangesPage from "../charts/RangesPage.svelte";
  import { chartsHandoff } from "../charts/handoff.svelte";
  import Ranges from "../ranges/Ranges.svelte";
  import SessionReview from "../session/SessionReview.svelte";
  import type { RangePreset, SessionScope } from "../session/types";
  import Tag from "../ui/Tag.svelte";
  import type { RangeFilter } from "../ranges/types";
  import Solver from "../solver/Solver.svelte";
  import Library from "../library/Library.svelte";
  import { handoff } from "../solver/handoff.svelte";
  import { analysis } from "../analysis/store.svelte";
  import JobToasts from "../jobs/JobToasts.svelte";
  import AutoDoneToast from "../analysis/AutoDoneToast.svelte";
  import { jobs } from "../jobs/store.svelte";
  import type { JobTarget } from "../jobs/types";
  import { openSaved } from "../solver/solutions";
  import { solver } from "../solver/store.svelte";
  import ChoiceMenu from "../ui/ChoiceMenu.svelte";
  import PageHeader from "../ui/PageHeader.svelte";
  import SegmentedControl from "../ui/SegmentedControl.svelte";
  import { followPage, pageFrame, pageHead } from "../ui/page.svelte";
  import TopBar from "../nav/TopBar.svelte";
  import { ALL_PAGES, LOCKED_PAGES, PAGES, type Page } from "../nav/pages";

  type Tab = Page;
  /** Tabs that read hand data get the period filter in the top bar, most also the game filter. */
  const DATA_TABS: Tab[] = ["overview", "reports", "leaks", "ranges", "tournaments", "players", "hands"];
  const NOTES: Partial<Record<Tab, string>> = {
    overview: "Alle Hände im gewählten Zeitraum: Ergebnis, Spielstil und wo du Chips verlierst.",
    tournaments: "Klick auf ein Turnier öffnet seinen Bericht: Ergebnis, Glück, deine Fehler und was du als Nächstes übst.",
    reports: "Starthände, Positionen und Sessions über den gewählten Zeitraum.",
    players: "Ein Klick auf einen Gegner zeigt Typ, Gegenzug und Bedeutung.",
    live: "Deine Gegner an den offenen Tischen. Ein Klick zeigt alle Werte mit Bedeutung.",
    hud: "Panels direkt auf dem Tisch ziehen. Änderungen gelten sofort an allen offenen Tischen.",
    settings:
      "Ringside arbeitet lokal. Ins Internet geht es nur für Updates, den KI-Coach und die Cloud.",
    leaks: "Was dich am meisten kostet, das Wichtigste zuerst. Darunter deine Werte gegen veröffentlichte Richtwerte.",
    ranges: "Welche Hände du preflop wirklich spielst – je Position, Tiefe und Vorgeschichte, neben dem Modell.",
    coach: "Optional: Ein Sprachmodell erklärt deine Zahlen und bespricht Turniere ausführlich. Jede Zahl rechnet Ringside selbst.",
    training: "Kurze Runden, deine eigenen Hände und Wissen zum Nachlesen.",
    play: "Postflop-Spots rechnet Ringside im Hintergrund vor.",
    solver: "Gerechnet wird nur auf diesem Rechner.",
    library: "Gespeichert auf diesem Rechner, gerechnet hier oder geladen aus Ringside Cloud.",
  };
  /** While the client runs: the lines of pages whose usual line promises a rating. */
  const LOCKED_NOTES: Partial<Record<Tab, string>> = {
    tournaments: "Ergebnisse deiner Turniere. Den Bericht gibt es, sobald du PokerStars schließt.",
    ranges: "Welche Hände du preflop wirklich spielst – je Position, Tiefe und Vorgeschichte.",
    leaks: "Deine Werte gegen Richtwerte – sobald du PokerStars schließt.",
  };
  watchPokerClient();
  const clientRunning = $derived(pokerClient.running === true);
  const GAMES: [GameKind | null, string][] = [
    ["tournament", "Turniere"],
    ["cash", "Cash"],
    [null, "Alle"],
  ];
  const STORE = "ringside.filter";

  const query = new URLSearchParams(window.location.search);
  const initialTab = query.get("tab") as Tab | null;
  let tab = $state<Tab>(initialTab && ALL_PAGES.includes(initialTab) ? initialTab : "dashboard");
  let status = $state<ImportStatus | null>(null);
  let tables = $state(0);
  let refresh = $state(0);
  let hud = $state<HudConfig | null>(null);

  let game = $state<GameKind | null>(null);
  let period = $state<Period>("all");
  /** One tournament for every data view; game and period do not apply while it is set. */
  let tournament = $state<number | null>(null);
  /** One session, chosen from the review; like a tournament, game and period do not apply meanwhile. */
  let session = $state<SessionScope | null>(null);
  const filter = $derived<Filter>(
    session !== null
      ? { game: null, since: session.since, until: session.until, tournament_id: null }
      : tournament === null
        ? { game, since: since(period), tournament_id: null }
        : { game: null, since: null, tournament_id: tournament },
  );
  /** "Deine Ranges" opens on this situation once. */
  let rangesPreset = $state<RangePreset | null>(null);
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
  let handOpenAt = $state<number | null>(null);
  /** Coach tab: a tournament to analyse, or a stored review to open, set from a report. */
  let analyzeTournament = $state<number | null>(null);
  let coachOpen = $state<number | null>(null);
  /** Tournaments tab: the tournament whose report is open instead of the list. */
  const initialReport = Number(query.get("report"));
  let reportTournament = $state<number | null>(Number.isSafeInteger(initialReport) && initialReport > 0 ? initialReport : null);
  const showingReport = $derived(tab === "tournaments" && reportTournament !== null);

  function openReport(id: number) {
    reportTournament = id;
    tab = "tournaments";
  }

  // "Range ansehen" in a hand opens the ranges page; its "Zurück zur Hand" returns.
  let chartsBack = chartsHandoff.back;
  $effect(() => {
    if (chartsHandoff.target) tab = "charts";
    // Back to the hand as it was: the list keeps its filters.
    if (chartsHandoff.back !== chartsBack) {
      chartsBack = chartsHandoff.back;
      handToOpen = chartsHandoff.hand;
      handOpenAt = chartsHandoff.at;
      tab = "hands";
    }
  });
  // "Im Solver nachrechnen" in the replayer hands a spot over; the solver page takes it.
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
  let handsSpot = $state<{ filter?: SpotFilter | null; range?: RangeFilter | null; profile?: string | null; label: string } | null>(null);
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
    const page = ALL_PAGES.find((p) => p === t.page);
    if (t.page === "hands") return showHands(null, t.id);
    if (t.page === "solver") {
      if (t.id !== null && t.id !== solver.savedId) openSaved(t.id).catch(() => {});
      else if (solver.status === "solved") solver.view = "result";
    }
    if (page) tab = page;
  }

  function go(id: Tab) {
    // "Turniere" again leads from a report back to the list.
    if (id === "tournaments") reportTournament = null;
    tab = id;
  }

  function showHands(player: string | null, handId: number | null = null) {
    handsPlayer = player;
    handsSpot = null;
    handToOpen = handId;
    handOpenAt = null;
    tab = "hands";
  }

  /** „Hände ansehen“ of a step of „Was als Nächstes?“. */
  function stepHands(h: StepHands) {
    if (h.by === "decision") {
      showHands(null, h.hand_id);
      handOpenAt = h.action_index;
    } else {
      showHands(null);
      handsSpot =
        h.by === "spot"
          ? { profile: h.key, label: h.label }
          : { filter: { stat: h.stat, made: h.made, positions: h.positions, band: h.band, game: h.game }, label: h.label };
    }
  }

  /** Game and period stay visible while a tournament is chosen; picking one of them drops it. */
  function setGame(g: GameKind | null) {
    game = g;
    tournament = null;
    session = null;
  }

  function setPeriod(p: Period) {
    period = p;
    tournament = null;
    session = null;
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

</script>

<div
  class="shell"
  style:--frame-left={pageFrame.current ? `${pageFrame.current.left}px` : null}
  style:--frame-right={pageFrame.current ? `${pageFrame.current.right}px` : null}
>
  <TopBar
    {tab}
    {filter}
    tournaments={tournamentList}
    {tables}
    {status}
    {hud}
    {refresh}
    onSelect={go}
    onView={viewTarget}
    onChooseFolder={chooseFolder}
  />

  <main>
    <CloudNotice onSettings={() => (tab = "settings")} />
    <PageHeader title={PAGES[tab].label} sub={(clientRunning && LOCKED_NOTES[tab]) || NOTES[tab]} sections={pageHead.current} locked={clientRunning && LOCKED_PAGES.includes(tab)}>
      {#snippet actions()}
      {#if DATA_TABS.includes(tab) && !showingReport}
        {@const off = tournament !== null || session !== null}
        {@const offTitle = off ? `Gilt nicht, solange ${session ? "eine Session" : "ein Turnier"} gewählt ist. Klick hebt die Auswahl auf.` : undefined}
        <div class="filters">
          {#if session}<Tag onremove={() => (session = null)} removeLabel="Nur diese Session: aufheben">{session.label}</Tag>{/if}
          <TournamentPicker
            tournaments={tournamentList}
            selected={tournament}
            onSelect={(id) => {
              tournament = id;
              session = null;
            }}
          />
          {#if tab !== "tournaments"}
            <span class="wide-game" class:dim={off} title={offTitle}>
              <SegmentedControl
                label="Spielart"
                value={off ? "" : (game ?? "all")}
                options={GAMES.map(([id, label]) => ({ value: id ?? "all", label }))}
                onchange={(v) => setGame(v === "all" ? null : (v as GameKind))}
              />
            </span>
            <span class="narrow-game">
              <ChoiceMenu label="Spielart" dim={off} title={offTitle} options={GAMES} value={off ? undefined : game} onchange={setGame} />
            </span>
          {/if}
          <span class="wide-period" class:dim={off} title={offTitle}>
            <SegmentedControl
              label="Zeitraum"
              value={off ? "" : period}
              options={PERIODS.map(([id, label]) => ({ value: id as string, label }))}
              onchange={(v) => setPeriod(v as Period)}
            />
          </span>
          <span class="narrow-period">
            <ChoiceMenu label="Zeitraum" dim={off} title={offTitle} options={PERIODS} value={off ? undefined : period} onchange={setPeriod} />
          </span>
        </div>
      {/if}
      {/snippet}
    </PageHeader>

    <div class="content" class:fill={tab === "hands"} use:followPage>
      <div class="frame">
      {#if tab === "dashboard"}
        <SessionReview
          {refresh}
          {filter}
          onStepHands={stepHands}
          onReport={openReport}
          onHand={(scope, hand, at) => {
            session = scope;
            tournament = null;
            showHands(null, hand);
            handOpenAt = at;
          }}
          onRanges={(scope, preset) => {
            session = scope;
            tournament = null;
            rangesPreset = preset;
            tab = "ranges";
          }}
          onHands={(scope, range, label) => {
            session = scope;
            tournament = null;
            showHands(null);
            handsSpot = { range, label };
          }}
          onLearn={(target) => {
            learnOpen = target;
            tab = "training";
          }}
        />
      {:else if tab === "overview"}
        <Dashboard
          {filter}
          {hud}
          {refresh}
          onHand={(id) => showHands(null, id >= 0 ? id : null)}
          onStepHands={stepHands}
          onLearn={(target) => {
            learnOpen = target;
            tab = "training";
          }}
        />
      {:else if tab === "reports"}
        <Reports {filter} {refresh} />
      {:else if tab === "leaks" && adviceLocked()}
        <AtTable hands={status?.total_hands} />
      {:else if tab === "leaks"}
        <Leaks
          {filter}
          {refresh}
          onHands={(spot, label) => {
            showHands(null);
            handsSpot = { filter: spot, label };
          }}
          onStepHands={stepHands}
          onLearn={(target) => {
            learnOpen = target;
            tab = "training";
          }}
        />
      {:else if tab === "ranges"}
        <Ranges
          {filter}
          {refresh}
          preset={rangesPreset}
          onPresetUsed={() => (rangesPreset = null)}
          onHands={(range, label) => {
            showHands(null);
            handsSpot = { range, label };
          }}
        />
      {:else if tab === "tournaments" && reportTournament !== null}
        {@const id = reportTournament}
        <ClientLock feature="Der Turnier-Bericht">
          <TournamentReport
            tournamentId={id}
            {refresh}
            onBack={() => (reportTournament = null)}
            onHand={(hand, at) => {
              tournament = id;
              showHands(null, hand);
              handOpenAt = at;
            }}
            onHands={() => {
              tournament = id;
              showHands(null);
            }}
            onStepHands={stepHands}
            onLearn={(target) => {
              learnOpen = target;
              tab = "training";
            }}
            onCoach={(what) => {
              if (what === "analyze") analyzeTournament = id;
              if (what === "review") coachOpen = id;
              tab = "coach";
            }}
          />
        </ClientLock>
      {:else if tab === "tournaments"}
        <Tournaments filter={tournamentFilter} {refresh} selected={tournament} onReport={openReport} />
      {:else if tab === "coach"}
        <Coach
          analyze={analyzeTournament}
          open={coachOpen}
          tournaments={tournamentList}
          onAnalyzeHandled={() => (analyzeTournament = null)}
          onOpenHandled={() => (coachOpen = null)}
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
        <LiveTables onHands={(name, id) => showHands(name, id ?? null)} onPlayers={() => (tab = "players")} />
      {:else if tab === "training"}
        <Training
          {hud}
          {filter}
          open={learnOpen}
          onOpened={() => (learnOpen = null)}
          onStepHands={stepHands}
        />
      {:else if tab === "play"}
        <PlayPage />
      {:else if tab === "charts"}
        <RangesPage />
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
          openAt={handOpenAt}
          spot={handsSpot}
          onClearSpot={() => (handsSpot = null)}
          onClearPlayer={() => (handsPlayer = null)}
          onClearTournament={() => (tournament = null)}
          onLearn={(target) => {
            learnOpen = target;
            tab = "training";
          }}
          onCoachSetup={() => (tab = "coach")}
        />
      {:else if tab === "hud"}
        <HudEditor />
      {:else}
        <SettingsView {status} />
      {/if}
      </div>
    </div>
  </main>
  <JobToasts onView={viewTarget}>
    <AutoDoneToast onView={() => (tab = "dashboard")} />
  </JobToasts>
</div>

<style>
  .shell {
    display: flex;
    flex-direction: column;
    height: 100vh;
  }
  main {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }
  .filters {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
  .dim {
    opacity: 0.45;
    transition: opacity var(--dur-fast) var(--ease-standard);
  }
  .dim:hover {
    opacity: 0.8;
  }
  /* Narrow windows: Spielart and Zeitraum as compact menus, so the filters stay in one row. */
  .narrow-game,
  .narrow-period {
    display: none;
  }
  @media (max-width: 1279px) {
    .wide-game {
      display: none;
    }
    .narrow-game {
      display: contents;
    }
  }
  @media (max-width: 1439px) {
    .wide-period {
      display: none;
    }
    .narrow-period {
      display: contents;
    }
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
  /* No page is wider than the app's one page width, whatever it sets itself. */
  .frame {
    max-width: var(--page-max);
    margin-inline: auto;
  }
  .fill .frame {
    height: 100%;
  }
</style>
