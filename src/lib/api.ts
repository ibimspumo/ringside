import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import * as analysisDemo from "./analysis/demo";
import type { AnalysisStatus, AnalysisSummary, AutoDone, Decision } from "./analysis/types";
import * as demo from "./demo";
import * as learnDemo from "./learn/demo";
import { demoNextSteps, demoWithin } from "./next/demo";
import type { NextStep } from "./next/types";
import type { DrillResult, DrillSpot, PlayProfile } from "./learn/profile";
import type { KeySpots, KeySpotView } from "./learn/spots";
import * as chartsDemo from "./charts/demo";
import type { ChartTarget } from "./charts/handoff.svelte";
import * as rangesDemo from "./ranges/demo";
import * as reportDemo from "./report/demo";
import * as sessionDemo from "./session/demo";
import * as skillDemo from "./skill/demo";
import type { SkillProfile } from "./skill/types";
import type { SessionPage } from "./session/types";
import type { TournamentReport } from "./report/types";
import type { RangeChart, RangeFilter, RangeGroup } from "./ranges/types";
import type { PreflopAnte, PreflopGrid, PreflopNode, PreflopProgress, PreflopSpots, TableOpens } from "./charts/preflop-types";
import * as solverDemo from "./solver/demo";
import type { Scenario } from "./solver/scenario";
import type {
  ModelSpot,
  SolveInfo,
  SolverDone,
  SolverEstimate,
  SolverNode,
  SolverProgress,
  SolverSettings,
  SolverSpot,
  SolverStep,
} from "./solver/types";
import type {
  Exercise,
  Feedback,
  LearnAnswer,
  LearnKind,
  LearnProgress,
  OwnSpot,
  ClientWindow,
  Dashboard,
  Filter,
  GroupView,
  Hand,
  HandQuery,
  HandSummary,
  ImportReport,
  ImportStatus,
  LeakReport,
  LiveTable,
  NotesSource,
  PlayerNote,
  PlayerView,
  Reports,
  SeatPos,
  Settings,
  ShownHand,
  TableView,
  Tournaments,
  PushFoldAnte,
  PushFoldChart,
  PushFoldGrid,
} from "./types";

/** False when the UI runs in a normal browser (design work, screenshots). */
export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export function call<T>(cmd: string, args: Record<string, unknown>, fallback: () => T): Promise<T> {
  // A fallback that throws rejects, as the command would.
  return inTauri ? invoke<T>(cmd, args) : new Promise<T>((resolve) => resolve(fallback()));
}

/**
 * Browser preview: `?data=empty` shows the analysis pages without hands, `?data=error` a failed load,
 * `?data=big` long names, huge numbers and long lists, `?data=scale` the lists of 200 000 hands
 * where a page has them.
 */
const previewData = typeof window === "undefined" ? null : new URLSearchParams(window.location.search).get("data");

function sample<T>(cmd: string, args: Record<string, unknown>, full: () => T, none: () => T, big: () => T): Promise<T> {
  if (!inTauri && previewData === "error") return Promise.reject(new Error("Vorschau: Laden fehlgeschlagen"));
  return call<T>(cmd, args, previewData === "empty" ? none : previewData === "big" ? big : full);
}

/** Browser preview: `?locked=1` plays a running PokerStars client, `?locked=gg` a running GGPoker client. */
const lockedParam = typeof window === "undefined" ? null : new URLSearchParams(window.location.search).get("locked");
const demoLocked = lockedParam === "1" || lockedParam === "gg";

/** In the preview, commands that rate or recommend refuse like the backend while the client runs. */
function advice<T>(fallback: () => T): () => T {
  return () => {
    if (demoLocked) throw new Error("gesperrt");
    return fallback();
  };
}

/** Like the backend, the demo gives no player type while PokerStars runs. */
const unread = <T extends { read: unknown }>(p: T): T => (demoLocked ? { ...p, read: null } : p);

/** Browser-only stand-in for saved settings, so the demo reacts to edits. */
let demoStore: Settings = structuredClone(demo.demoSettings);

export const api = {
  overlayView: (label: string, aspect: number) =>
    call<TableView | null>("overlay_view", { label, aspect }, () => ({
      ...demo.demoTable(),
      hud: structuredClone(demoStore.hud),
    })),
  previewView: (aspect: number) => call<TableView | null>("preview_view", { aspect }, () => null),
  layoutFor: (maxSeats: number, aspect: number) =>
    call<{ key: string; layout: SeatPos[] }>("layout_for", { maxSeats, aspect }, () => {
      const key = `${maxSeats}`;
      return { key, layout: structuredClone(demoStore.layouts[key] ?? demo.defaultLayout(maxSeats)) };
    }),
  setHitRegions: (label: string, regions: { x: number; y: number; w: number; h: number }[]) =>
    call<void>("set_hit_regions", { label, regions }, () => undefined),
  getSettings: () => call<Settings>("get_settings", {}, () => structuredClone(demoStore)),
  saveSettings: (settings: Settings) =>
    call<void>("save_settings", { settings }, () => {
      demoStore = structuredClone(settings);
    }),
  saveSeat: (key: string, visual: number, pos: SeatPos, aspect: number) =>
    call<void>("save_seat", { key, visual, pos, aspect }, () => {
      const max = Number(key);
      const layout = demoStore.layouts[key] ?? demo.defaultLayout(max);
      layout[visual] = pos;
      demoStore.layouts[key] = layout;
    }),
  resetLayout: (key: string) =>
    call<void>("reset_layout", { key }, () => {
      delete demoStore.layouts[key];
    }),
  exportProfile: (path: string) => call<void>("export_profile", { path }, () => undefined),
  importProfile: (path: string) => call<void>("import_profile", { path }, () => undefined),
  resetProfile: () =>
    call<void>("reset_profile", {}, () => {
      demoStore = { ...structuredClone(demo.demoSettings), hand_history_dirs: demoStore.hand_history_dirs };
    }),
  importStatus: () =>
    call<ImportStatus>("import_status", {}, () => ({
      // Browser preview: ?nohistory=1 shows the first-run state without a hand history folder.
      dirs: new URLSearchParams(window.location.search).has("nohistory")
        ? []
        : ["C:\\Users\\du\\AppData\\Local\\PokerStars.EU\\HandHistory"],
      files_read: 12,
      hands_imported: 12,
      total_hands: previewData === "empty" ? 0 : 6431,
      malformed: 0,
      last_error: null,
      upgrade: null,
      gg_dirs: demoStore.gg_dirs,
      sites: previewData === "empty" ? {} : { pokerstars: 6383, ggpoker: 48 },
    })),
  /** Hand histories, summaries or ZIP archives picked or dropped by the user. */
  importHandFiles: (paths: string[]) =>
    call<ImportReport>("import_hand_files", { paths }, () => ({
      files: paths.length,
      new_hands: 0,
      duplicates: 48,
      malformed: 0,
      tournaments: 0,
      skipped: 0,
    })),
  clientWindows: () => call<ClientWindow[]>("client_windows", {}, () => (previewData === "empty" ? [] : demoWindows())),
  liveTables: () =>
    call<LiveTable[]>("live_tables", {}, () =>
      (previewData === "empty" ? [] : demo.demoLiveTables()).map((l) => (l.table ? { ...l, table: { ...l.table, seats: l.table.seats.map(unread) } } : l)),
    ),
  pokerClientRunning: () => call<boolean>("poker_client_running", {}, () => demoLocked),
  /** "PokerStars" or "GGPoker" while one runs. */
  pokerClientName: () =>
    call<string | null>("poker_client_name", {}, () => (lockedParam === "gg" ? "GGPoker" : demoLocked ? "PokerStars" : null)),
  tournaments: (filter: Filter) =>
    sample<Tournaments>("tournaments", { filter }, previewData === "scale" ? demo.scaleTournaments : demo.demoTournaments, () => ({ rows: [], totals: [] }), demo.bigTournaments),
  /** The report of one tournament from Ringside's own numbers; locked while PokerStars runs. */
  tournamentReport: (tournamentId: number) => {
    const made = advice(() => reportDemo.demoReport(tournamentId));
    return sample<TournamentReport>("tournament_report", { tournamentId }, made, made, made);
  },
  /** The latest session (or the one starting at `start`); advice is left out while PokerStars runs. */
  sessionReview: (start: string | null) =>
    sample<SessionPage>("session_review", { start }, () => sessionDemo.demoSession(start), () => sessionDemo.demoSession(start), () => sessionDemo.demoSession(start)),
  /** The Spielwertung over every analysed tournament; locked while PokerStars runs. `?data=few` shows three tournaments. */
  skillProfile: () => {
    const made = advice(() => (previewData === "few" ? skillDemo.fewSkill() : skillDemo.demoSkill()));
    return sample<SkillProfile>("skill_profile", {}, made, advice(skillDemo.emptySkill), made);
  },
  playerNote: (player: string) => call<PlayerNote | null>("player_note", { player }, () => demo.demoNote(player)),
  notesSources: () => call<NotesSource[]>("notes_sources", {}, demo.demoNotesSources),
  detectedDirs: () =>
    call<string[]>("detected_dirs", {}, () =>
      new URLSearchParams(window.location.search).has("nohistory") ? [] : ["C:\\Users\\du\\AppData\\Local\\PokerStars.EU\\HandHistory\\Hero"],
    ),
  dashboard: (filter: Filter) => sample<Dashboard>("dashboard", { filter }, () => demo.demoDashboard(filter), demo.emptyDashboard, () =>
      demo.bigDashboard(filter),
    ),
  reports: (filter: Filter) =>
    sample<Reports>(
      "reports",
      { filter },
      () => demo.demoReports(filter),
      () => ({ positions: [], starting_hands: [], sessions: [] }),
      () => demo.demoReports(filter),
    ),
  /** The overview's results by position, without the rest of `reports`. */
  positions: (filter: Filter) =>
    sample<GroupView[]>("positions", { filter }, () => demo.demoReports(filter).positions, () => [], () => demo.demoReports(filter).positions),
  leaks: (filter: Filter) =>
    sample<LeakReport>("leaks", { filter }, advice(() => demo.demoLeaks(filter)), () => ({
      ...demo.demoLeaks({ game: "cash", since: null, tournament_id: null }),
      too_small: 0,
    }), () => demo.bigLeaks(filter)),
  rangeGroups: (filter: Filter) => call<RangeGroup[]>("range_groups", { filter }, () => rangesDemo.demoGroups(filter)),
  rangeChart: (filter: Filter, range: RangeFilter) =>
    call<RangeChart>("range_chart", { filter, range }, () => rangesDemo.demoChart(filter, range)),
  players: (filter: Filter, search: string, limit = 300) =>
    call<PlayerView[]>("players", { filter, search, limit }, () =>
      (previewData === "empty" ? [] : demo.demoPlayers(filter)).filter((p) => p.name.toLowerCase().includes(search.toLowerCase())).map(unread),
    ),
  shownHands: (player: string) => call<ShownHand[]>("shown_hands", { player }, demo.demoShown),
  hands: (query: HandQuery) =>
    call<HandSummary[]>("hands", { query }, () =>
      demoLocked ? demo.demoHands(query).map((h) => ({ ...h, worst: null })) : analysisDemo.withWorst(demo.demoHands(query), query),
    ),
  analysisStatus: () => call<AnalysisStatus>("analysis_status", {}, analysisDemo.demoStatus),
  /** Starts (`true`) or pauses the analysis of the whole history. */
  analysisRun: (on: boolean) => call<void>("analysis_run", { on }, () => analysisDemo.demoRun(on)),
  /** Analyses hands now; their postflop spots go to the front of the queue. Returns the first hand's decisions. */
  analysisHands: (handIds: number[]) =>
    call<Decision[]>("analysis_hands", { handIds }, advice(() => handIds.flatMap((id, i) => (i === 0 ? analysisDemo.demoAnalyze(id) : [])))),
  /** Stops the whole analysis; rated decisions stay. */
  analysisCancel: () => call<void>("analysis_cancel", {}, analysisDemo.demoCancel),
  /** Stops one hand: its postflop spot goes back in line. */
  analysisCancelHand: (handId: number) => call<void>("analysis_cancel_hand", { handId }, () => analysisDemo.demoCancelHand(handId)),
  /** Where a rated preflop decision sits in the charts; null when no bundled table fits. */
  analysisChartTarget: (handId: number, actionIndex: number) =>
    inTauri ? invoke<ChartTarget | null>("analysis_chart_target", { handId, actionIndex }) : demoLocked ? Promise.reject(new Error("gesperrt")) : (async (): Promise<ChartTarget | null> => {
      // Browser preview: the sample hands' spots in the sample charts (99 first in from the small blind, 86s on the button).
      const d = analysisDemo.demoDecisions(handId).find((x) => x.action_index === actionIndex);
      if (!d || d.street !== "preflop" || d.status !== "rated" || d.source === "solver") return null;
      const base = { action_index: actionIndex, ante: "big_blind" as const, players: 6, callers: [] };
      if (d.source === "equilibrium") {
        const call = d.spot === "call_shove";
        return { ...base, kind: "pushfold" as const, stack_bb: 10, seat: call ? 5 : 3, shover: call ? 3 : null, node: null, hand: call ? "K9o" : "86s" };
      }
      const spots = await import("./charts/preflop-demo").then((m) => m.demoSpots("big_blind", 6, 100));
      const open = spots.nodes.find((n) => n.seat === 4 && !n.line.length);
      return { ...base, kind: "preflop" as const, stack_bb: 100, seat: 4, shover: null, node: open?.id ?? 0, hand: "99" };
    })(),
  analysisDecisions: (handId: number) => call<Decision[]>("analysis_decisions", { handId }, advice(() => analysisDemo.demoDecisions(handId))),
  analysisSummary: (filter: Filter) => call<AnalysisSummary>("analysis_summary", { filter }, advice(analysisDemo.demoSummary)),
  /**
   * „Was als Nächstes?“: the one ranked list every page reads, heaviest first. With `part` (a
   * session: `since` and `until`) only its entries that happened there, in the same order.
   * Locked while PokerStars runs.
   */
  nextSteps: (filter: Filter, part: Filter | null = null) => {
    const made = advice(() => (part ? demoWithin(sessionDemo.SESSION_KINDS, "in dieser Session") : demoNextSteps()));
    return sample<NextStep[]>("next_steps", { filter, part }, made, advice(() => []), made);
  },

  /** "Neue Hände automatisch analysieren"; also while PokerStars runs, it only changes the setting. */
  analysisSetAuto: (auto: boolean) =>
    call<void>("analysis_set_auto", { on: auto }, () => {
      demoStore.analysis = { auto };
    }),
  hand: (id: number) => call<Hand | null>("hand", { id }, () => learnDemo.demoSpotHand(id) ?? demo.demoHandById(id)),
  learnExercise: (kind: LearnKind, level: number, seed: number | null = null) =>
    inTauri ? invoke<Exercise>("learn_exercise", { kind, level, seed }) : learnDemo.demoExercise(kind, level, seed),
  learnCheck: (exercise: Exercise, answer: LearnAnswer) =>
    inTauri ? invoke<Feedback>("learn_check", { exercise, answer }) : learnDemo.demoCheck(exercise, answer),
  learnOwnSpots: (filter: Filter, kind: LearnKind | null = null, limit = 60) =>
    call<OwnSpot[]>("learn_own_spots", { filter, kind, limit }, () => learnDemo.demoOwnSpots(kind, limit)),
  learnCheckOwn: (spotId: string, answer: LearnAnswer) =>
    call<Feedback>("learn_check_own", { spotId, answer }, () => learnDemo.demoCheckOwn(spotId, answer)),
  /** Solves a push/fold spot's table ahead of the answer; seconds at a full table. */
  learnPrepareOwn: (spotId: string) => call<void>("learn_prepare_own", { spotId }, () => undefined),
  learnProgress: () => call<LearnProgress>("learn_progress", {}, learnDemo.demoProgress),
  learnProfile: (filter: Filter) => call<PlayProfile>("learn_profile", { filter }, learnDemo.demoProfile),
  learnDrillReal: (key: string, limit: number) => call<DrillSpot[]>("learn_drill_real", { key, limit }, () => learnDemo.demoDrillReal(key, limit)),
  learnDrillVariant: (key: string, seed: number) =>
    call<DrillSpot>("learn_drill_variant", { key, seed }, () => learnDemo.demoDrillVariant(key, seed)),
  learnDrillCheck: (spot: DrillSpot, choice: number) =>
    call<DrillResult>(
      "learn_drill_check",
      { key: spot.key, handId: spot.hand_id, actionIndex: spot.action_index, seed: spot.seed, choice },
      () => learnDemo.demoDrillCheck(spot.key, spot.hand_id, spot.action_index, spot.seed, choice),
    ),
  learnKeySpots: () => (inTauri ? invoke<KeySpots>("learn_key_spots") : import("./learn/spots").then((m) => advice(() => m.demoKeySpots())())),
  learnKeySpot: (id: string) => (inTauri ? invoke<KeySpotView>("learn_key_spot", { id }) : import("./learn/spots").then((m) => advice(() => m.demoKeySpot(id))())),
  learnKeySpotCheck: (id: string, choice: number) =>
    inTauri ? invoke<DrillResult>("learn_key_spot_check", { id, choice }) : import("./learn/spots").then((m) => advice(() => m.demoKeySpotCheck(id, choice))()),
  pushfoldGrid: () => call<PushFoldGrid>("pushfold_grid", {}, chartsDemo.demoGrid),
  pushfoldChart: (ante: PushFoldAnte, players: number, stackBb: number) =>
    call<PushFoldChart>("pushfold_chart", { ante, players, stackBb }, () => chartsDemo.demoChart(stackBb, players)),
  // The preview sample is large, so the browser loads it only when asked.
  preflopGrid: () =>
    inTauri ? invoke<PreflopGrid>("preflop_grid") : import("./charts/preflop-demo").then((d) => d.demoGrid()),
  /** Off the grid this solves live; progress arrives as `preflop-progress`. */
  preflopSpots: (ante: PreflopAnte, players: number, stackBb: number) =>
    inTauri
      ? invoke<PreflopSpots>("preflop_spots", { ante, players, stackBb })
      : import("./charts/preflop-demo").then((d) => d.demoSpots(ante, players, stackBb)),
  preflopNode: (ante: PreflopAnte, players: number, stackBb: number, node: number) =>
    inTauri
      ? invoke<PreflopNode>("preflop_node", { ante, players, stackBb, node })
      : import("./charts/preflop-demo").then((d) => d.demoNode(ante, players, stackBb, node)),
  /** Every table size at this depth, from the precomputed tables only. */
  tableOpens: (ante: PreflopAnte, stackBb: number, pushfold: boolean) =>
    inTauri
      ? invoke<TableOpens[]>("table_opens", { ante, stackBb, pushfold })
      : import("./charts/preflop-demo").then((d) => d.demoTableOpens(ante, stackBb, pushfold)),
  preflopCancel: () =>
    inTauri ? invoke<void>("preflop_cancel") : import("./charts/preflop-demo").then((d) => d.demoCancel()),
  solverEstimate: (spot: SolverSpot, threads: number) =>
    call<SolverEstimate>("solver_estimate", { spot, threads }, () => solverDemo.demoEstimate(spot, threads)),
  /**
   * Progress and result arrive via `onSolverProgress`/`onSolverDone`, tagged with `run`. A saved
   * solution of the same spot opens instead unless `fresh`; `info` labels it in "Meine Lösungen".
   */
  solverStart: (spot: SolverSpot, settings: SolverSettings, run: number, info: SolveInfo, fresh: boolean) =>
    call<void>("solver_start", { spot, settings, run, info, fresh }, () => solverDemo.demoStart(spot, settings, run)),
  /** The model's exact ranges, pot and stack for a pick, so the spot can be shared. */
  solverModelSpot: (pick: Pick<Scenario, "players" | "ante" | "stack" | "seats" | "pot">) =>
    call<ModelSpot | null>("solver_model_spot", { pick }, () => null),
  solverCancel: () => call<void>("solver_cancel", {}, solverDemo.demoCancel),
  solverClear: () => call<void>("solver_clear", {}, solverDemo.demoClear),
  solverNode: (path: SolverStep[]) => call<SolverNode>("solver_node", { path }, () => solverDemo.demoNode(path)),
  /** Whether the solver page is open; the solved tree is freed soon after it closed. */
  solverPage: (open: boolean) => call<void>("solver_page", { open }, () => {}),
};

export function onSolverProgress(handler: (p: SolverProgress) => void): Promise<UnlistenFn> {
  return inTauri ? on("solver-progress", handler) : Promise.resolve(solverDemo.onProgress(handler));
}

export function onSolverDone(handler: (d: SolverDone) => void): Promise<UnlistenFn> {
  return inTauri ? on("solver-done", handler) : Promise.resolve(solverDemo.onDone(handler));
}

/** The solved tree made room for its saved snapshot, which lacks the deeper streets. */
export function onSolverReleased(handler: () => void): Promise<UnlistenFn> {
  return on("solver-released", handler);
}

export function onPreflopProgress(handler: (p: PreflopProgress) => void): Promise<UnlistenFn> {
  return inTauri ? on("preflop-progress", handler) : import("./charts/preflop-demo").then((d) => d.onProgress(handler));
}

export function onAnalysisStatus(handler: (s: AnalysisStatus) => void): Promise<UnlistenFn> {
  return inTauri ? on("analysis-status", handler) : Promise.resolve(analysisDemo.onStatus(handler));
}

/** The automatic run rated new hands. The demo sends one with `?autodone=1`. */
export function onAnalysisAutoDone(handler: (d: AutoDone) => void): Promise<UnlistenFn> {
  return inTauri ? on("analysis-auto-done", handler) : Promise.resolve(analysisDemo.onAutoDone(handler));
}

/** A hand's postflop spot was solved (or failed). */
export function onAnalysisHand(handler: (handId: number) => void): Promise<UnlistenFn> {
  return inTauri ? on("analysis-hand", handler) : Promise.resolve(analysisDemo.onHand(handler));
}

export async function on<T>(event: string, handler: (payload: T) => void): Promise<UnlistenFn> {
  if (!inTauri) return () => {};
  return listen<T>(event, (e) => handler(e.payload));
}

/** Browser preview: `?windows=0` no PokerStars window, `?windows=many` a busy evening with lobbies. */
function demoWindows(): ClientWindow[] {
  const q = new URLSearchParams(window.location.search);
  const shown = q.get("windows");
  if (shown === "0" || q.get("tables") === "0") return [];
  const table = (title: string, key: string, has_hands = true): ClientWindow => ({ title, class: "PokerStarsTableFrameClass", table_key: key, has_hands });
  const lobby = (title: string): ClientWindow => ({ title, class: "PokerStarsLobbyClass", table_key: null, has_hands: false });
  const first = table("$5.50 Mini Early Battle - 50/100 Ante 12 - Turnier 4046789300 Tisch 10 - Eingeloggt als Hero", "t:4046789300:10");
  if (shown !== "many")
    return [
      first,
      table("Aquila II - $0.05/$0.10 USD - No Limit Hold'em - Eingeloggt als Hero", "c:Aquila II"),
      table("$11 Bounty Hunters - Turnier 4046799120 Tisch 4 - Eingeloggt als Hero", "t:4046799120:4", false),
      lobby("PokerStars Lobby"),
    ];
  return [
    first,
    table(
      "$11 Sunday Storm Special Edition Progressive Knockout Hyper Turbo 6-Max - 400/800 Ante 100 - Turnier 4046791122 Tisch 287 - Eingeloggt als Hero",
      "t:4046791122:287",
      false,
    ),
    table("Halley III - $0.02/$0.05 USD - No Limit Hold'em - Eingeloggt als Hero", "c:halley-iii"),
    lobby("PokerStars Lobby"),
    lobby("Turnier-Lobby: $5.50 Mini Early Battle"),
    lobby("Turnier-Lobby: $11 Sunday Storm Special Edition Progressive Knockout Hyper Turbo 6-Max"),
    lobby("Turnier-Lobby: $3.30 Bounty Builder"),
    lobby("Kassierer"),
    lobby("Spielhistorie"),
  ];
}
