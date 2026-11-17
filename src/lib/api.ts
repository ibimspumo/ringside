import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import * as demo from "./demo";
import * as learnDemo from "./learn/demo";
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
} from "./types";

/** False when the UI runs in a normal browser (design work, screenshots). */
export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export function call<T>(cmd: string, args: Record<string, unknown>, fallback: () => T): Promise<T> {
  return inTauri ? invoke<T>(cmd, args) : Promise.resolve(fallback());
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
      dirs: ["C:\\Users\\du\\AppData\\Local\\PokerStars.EU\\HandHistory"],
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
  tournaments: (filter: Filter) => call<Tournaments>("tournaments", { filter }, demo.demoTournaments),
  playerNote: (player: string) => call<PlayerNote | null>("player_note", { player }, () => demo.demoNote(player)),
  notesSources: () => call<NotesSource[]>("notes_sources", {}, demo.demoNotesSources),
  detectedDirs: () => call<string[]>("detected_dirs", {}, () => []),
  dashboard: (filter: Filter) => call<Dashboard>("dashboard", { filter }, () => demo.demoDashboard(filter)),
  reports: (filter: Filter) => call<Reports>("reports", { filter }, () => demo.demoReports(filter)),
  players: (filter: Filter, search: string, limit = 300) =>
    call<PlayerView[]>("players", { filter, search, limit }, () =>
      demo.demoPlayers(filter).filter((p) => p.name.toLowerCase().includes(search.toLowerCase())),
    ),
  shownHands: (player: string) => call<ShownHand[]>("shown_hands", { player }, demo.demoShown),
  hands: (query: HandQuery) => call<HandSummary[]>("hands", { query }, () => demo.demoHands(query)),
  hand: (id: number) => call<Hand | null>("hand", { id }, () => learnDemo.demoSpotHand(id) ?? demo.demoHandById(id)),
  learnExercise: (kind: LearnKind, level: number, seed: number | null = null) =>
    call<Exercise>("learn_exercise", { kind, level, seed }, () => learnDemo.demoExercise(kind, level, seed)),
  learnCheck: (exercise: Exercise, answer: LearnAnswer) =>
    call<Feedback>("learn_check", { exercise, answer }, () => learnDemo.demoCheck(exercise, answer)),
  learnOwnSpots: (filter: Filter, kind: LearnKind | null = null, limit = 60) =>
    call<OwnSpot[]>("learn_own_spots", { filter, kind, limit }, () => learnDemo.demoOwnSpots(kind, limit)),
  learnCheckOwn: (spotId: string, answer: LearnAnswer) =>
    call<Feedback>("learn_check_own", { spotId, answer }, () => learnDemo.demoCheckOwn(spotId, answer)),
  learnProgress: () => call<LearnProgress>("learn_progress", {}, learnDemo.demoProgress),
};

export async function on<T>(event: string, handler: (payload: T) => void): Promise<UnlistenFn> {
  if (!inTauri) return () => {};
  return listen<T>(event, (e) => handler(e.payload));
}
