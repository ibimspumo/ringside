import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import * as demo from "./demo";
import type {
  Dashboard,
  GameKind,
  Hand,
  HandSummary,
  ImportStatus,
  PlayerView,
  SeatPos,
  Settings,
  TableView,
} from "./types";

/** False when the UI runs in a normal browser (design work, screenshots). */
export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

function call<T>(cmd: string, args: Record<string, unknown>, fallback: () => T): Promise<T> {
  return inTauri ? invoke<T>(cmd, args) : Promise.resolve(fallback());
}

export const api = {
  overlayView: (label: string) => call<TableView | null>("overlay_view", { label }, () => demo.demoTable()),
  previewView: () => call<TableView | null>("preview_view", {}, () => demo.demoTable()),
  setHitRegions: (label: string, regions: { x: number; y: number; w: number; h: number }[]) =>
    call<void>("set_hit_regions", { label, regions }, () => undefined),
  getSettings: () => call<Settings>("get_settings", {}, () => structuredClone(demo.demoSettings)),
  saveSettings: (settings: Settings) => call<void>("save_settings", { settings }, () => undefined),
  saveSeat: (maxSeats: number, visual: number, pos: SeatPos) =>
    call<void>("save_seat", { maxSeats, visual, pos }, () => undefined),
  resetLayout: (maxSeats: number) => call<void>("reset_layout", { maxSeats }, () => undefined),
  importStatus: () =>
    call<ImportStatus>("import_status", {}, () => ({
      dirs: ["C:\\Users\\du\\AppData\\Local\\PokerStars.EU\\HandHistory"],
      files_read: 12,
      hands_imported: 1840,
      malformed: 0,
      last_error: null,
    })),
  detectedDirs: () => call<string[]>("detected_dirs", {}, () => []),
  dashboard: (game: GameKind | null) => call<Dashboard>("dashboard", { game }, demo.demoDashboard),
  players: (game: GameKind | null, search: string, limit = 200) =>
    call<PlayerView[]>("players", { game, search, limit }, () =>
      demo.demoPlayers().filter((p) => p.name.toLowerCase().includes(search.toLowerCase())),
    ),
  hands: (game: GameKind | null, player: string | null, limit = 100, offset = 0) =>
    call<HandSummary[]>("hands", { game, player, limit, offset }, demo.demoHands),
  hand: (id: number) => call<Hand | null>("hand", { id }, () => demo.demoHand),
};

export async function on<T>(event: string, handler: (payload: T) => void): Promise<UnlistenFn> {
  if (!inTauri) return () => {};
  return listen<T>(event, (e) => handler(e.payload));
}
