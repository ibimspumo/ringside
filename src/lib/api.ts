import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import * as analysisDemo from "./analysis/demo";
import type { AnalysisStatus, AnalysisSummary, Decision } from "./analysis/types";
import * as demo from "./demo";
import * as learnDemo from "./learn/demo";
import * as chartsDemo from "./charts/demo";
import type { PreflopAnte, PreflopGrid, PreflopNode, PreflopProgress, PreflopSpots } from "./charts/preflop-types";
import * as solverDemo from "./solver/demo";
import type {
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
  Hand,
  HandQuery,
  HandSummary,
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
  return inTauri ? invoke<T>(cmd, args) : Promise.resolve(fallback());
}

/**
 * Browser preview: `?data=empty` shows the analysis pages without hands, `?data=error` a failed load,
 * `?data=big` long names, huge numbers and long lists.
 */
const previewData = typeof window === "undefined" ? null : new URLSearchParams(window.location.search).get("data");

function sample<T>(cmd: string, args: Record<string, unknown>, full: () => T, none: () => T, big: () => T): Promise<T> {
  if (!inTauri && previewData === "error") return Promise.reject(new Error("Vorschau: Laden fehlgeschlagen"));
  return call<T>(cmd, args, previewData === "empty" ? none : previewData === "big" ? big : full);
}

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
      total_hands: 6431,
      malformed: 0,
      last_error: null,
      upgrade: null,
    })),
  clientWindows: () =>
    call<ClientWindow[]>("client_windows", {}, () => [
      {
        title: "$5.50 Mini Early Battle - 50/100 Ante 12 - Turnier 4046789300 Tisch 10 - Eingeloggt als Hero",
        class: "PokerStarsTableFrameClass",
        table_key: "t:4046789300:10",
        has_hands: true,
      },
      { title: "PokerStars Lobby", class: "PokerStarsLobbyClass", table_key: null, has_hands: false },
    ]),
  liveTables: () => call<LiveTable[]>("live_tables", {}, demo.demoLiveTables),
  pokerClientRunning: () =>
    call<boolean>("poker_client_running", {}, () => new URLSearchParams(window.location.search).get("locked") === "1"),
  tournaments: (filter: Filter) =>
    sample<Tournaments>("tournaments", { filter }, demo.demoTournaments, () => ({ rows: [], totals: [] }), demo.bigTournaments),
  playerNote: (player: string) => call<PlayerNote | null>("player_note", { player }, () => demo.demoNote(player)),
  notesSources: () => call<NotesSource[]>("notes_sources", {}, demo.demoNotesSources),
  detectedDirs: () => call<string[]>("detected_dirs", {}, () => []),
  dashboard: (filter: Filter) => sample<Dashboard>("dashboard", { filter }, () => demo.demoDashboard(filter), demo.emptyDashboard, () =>
      demo.bigDashboard(filter),
    ),
  reports: (filter: Filter) => call<Reports>("reports", { filter }, () => demo.demoReports(filter)),
  leaks: (filter: Filter) =>
    sample<LeakReport>("leaks", { filter }, () => demo.demoLeaks(filter), () => ({
      ...demo.demoLeaks({ game: "cash", since: null, tournament_id: null }),
      too_small: 0,
    }), () => demo.bigLeaks(filter)),
  players: (filter: Filter, search: string, limit = 300) =>
    call<PlayerView[]>("players", { filter, search, limit }, () =>
      demo.demoPlayers(filter).filter((p) => p.name.toLowerCase().includes(search.toLowerCase())),
    ),
  shownHands: (player: string) => call<ShownHand[]>("shown_hands", { player }, demo.demoShown),
  hands: (query: HandQuery) => call<HandSummary[]>("hands", { query }, () => analysisDemo.withWorst(demo.demoHands(query), query)),
  analysisStatus: () => call<AnalysisStatus>("analysis_status", {}, analysisDemo.demoStatus),
  /** Starts (`true`) or pauses the analysis of the whole history. */
  analysisRun: (on: boolean) => call<void>("analysis_run", { on }, () => analysisDemo.demoRun(on)),
  /** Analyses hands now; their postflop spots go to the front of the queue. Returns the first hand's decisions. */
  analysisHands: (handIds: number[]) =>
    call<Decision[]>("analysis_hands", { handIds }, () => handIds.flatMap((id, i) => (i === 0 ? analysisDemo.demoAnalyze(id) : []))),
  /** Stops the whole analysis; rated decisions stay. */
  analysisCancel: () => call<void>("analysis_cancel", {}, analysisDemo.demoCancel),
  /** Stops one hand: its postflop spot goes back in line. */
  analysisCancelHand: (handId: number) => call<void>("analysis_cancel_hand", { handId }, () => analysisDemo.demoCancelHand(handId)),
  analysisDecisions: (handId: number) => call<Decision[]>("analysis_decisions", { handId }, () => analysisDemo.demoDecisions(handId)),
  analysisSummary: (filter: Filter) => call<AnalysisSummary>("analysis_summary", { filter }, analysisDemo.demoSummary),
  hand: (id: number) => call<Hand | null>("hand", { id }, () => learnDemo.demoSpotHand(id) ?? demo.demoHandById(id)),
  learnExercise: (kind: LearnKind, level: number, seed: number | null = null) =>
    call<Exercise>("learn_exercise", { kind, level, seed }, () => learnDemo.demoExercise(kind, level, seed)),
  learnCheck: (exercise: Exercise, answer: LearnAnswer) =>
    call<Feedback>("learn_check", { exercise, answer }, () => learnDemo.demoCheck(exercise, answer)),
  learnOwnSpots: (filter: Filter, kind: LearnKind | null = null, limit = 60) =>
    call<OwnSpot[]>("learn_own_spots", { filter, kind, limit }, () => learnDemo.demoOwnSpots(kind, limit)),
  learnCheckOwn: (spotId: string, answer: LearnAnswer) =>
    call<Feedback>("learn_check_own", { spotId, answer }, () => learnDemo.demoCheckOwn(spotId, answer)),
  /** Solves a push/fold spot's table ahead of the answer; seconds at a full table. */
  learnPrepareOwn: (spotId: string) => call<void>("learn_prepare_own", { spotId }, () => undefined),
  learnProgress: () => call<LearnProgress>("learn_progress", {}, learnDemo.demoProgress),
  pushfoldGrid: () => call<PushFoldGrid>("pushfold_grid", {}, chartsDemo.demoGrid),
  pushfoldChart: (ante: PushFoldAnte, players: number, stackBb: number) =>
    call<PushFoldChart>("pushfold_chart", { ante, players, stackBb }, chartsDemo.demoChart),
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
  solverCancel: () => call<void>("solver_cancel", {}, solverDemo.demoCancel),
  solverClear: () => call<void>("solver_clear", {}, solverDemo.demoClear),
  solverNode: (path: SolverStep[]) => call<SolverNode>("solver_node", { path }, () => solverDemo.demoNode(path)),
};

export function onSolverProgress(handler: (p: SolverProgress) => void): Promise<UnlistenFn> {
  return inTauri ? on("solver-progress", handler) : Promise.resolve(solverDemo.onProgress(handler));
}

export function onSolverDone(handler: (d: SolverDone) => void): Promise<UnlistenFn> {
  return inTauri ? on("solver-done", handler) : Promise.resolve(solverDemo.onDone(handler));
}

export function onPreflopProgress(handler: (p: PreflopProgress) => void): Promise<UnlistenFn> {
  return inTauri ? on("preflop-progress", handler) : import("./charts/preflop-demo").then((d) => d.onProgress(handler));
}

export function onAnalysisStatus(handler: (s: AnalysisStatus) => void): Promise<UnlistenFn> {
  return inTauri ? on("analysis-status", handler) : Promise.resolve(analysisDemo.onStatus(handler));
}

/** A hand's postflop spot was solved (or failed). */
export function onAnalysisHand(handler: (handId: number) => void): Promise<UnlistenFn> {
  return inTauri ? on("analysis-hand", handler) : Promise.resolve(analysisDemo.onHand(handler));
}

export async function on<T>(event: string, handler: (payload: T) => void): Promise<UnlistenFn> {
  if (!inTauri) return () => {};
  return listen<T>(event, (e) => handler(e.payload));
}
