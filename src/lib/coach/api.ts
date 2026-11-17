import { invoke } from "@tauri-apps/api/core";
import { call, inTauri, on } from "../api";
import type { TournamentRow } from "../types";
import * as demo from "./demo";
import type {
  CoachProgress,
  CoachReport,
  CoachStatus,
  ConceptCount,
  Effort,
  Estimate,
  KeyInfo,
  ModelInfo,
  ReportListItem,
} from "./types";

export const coachApi = {
  status: () => call<CoachStatus>("coach_status", {}, demo.status),
  setKey: (key: string) => call<KeyInfo>("coach_set_key", { key }, demo.setKey),
  testKey: () => call<KeyInfo>("coach_test_key", {}, demo.keyInfo),
  deleteKey: () => call<void>("coach_delete_key", {}, demo.deleteKey),
  models: () => call<ModelInfo[]>("coach_models", {}, demo.models),
  setModel: (model: string) => call<void>("coach_set_model", { model }, () => demo.setModel(model)),
  setEffort: (effort: Effort) => call<void>("coach_set_effort", { effort }, () => demo.setEffort(effort)),
  /** Picks the hands and estimates the cost; nothing is sent. */
  prepare: (tournamentId: number) => call<Estimate>("coach_prepare", { tournamentId }, () => demo.estimate(tournamentId)),
  /** Sends the analysis of the estimate that carried `token`. */
  run: (token: string, tournamentId: number): Promise<CoachReport> =>
    inTauri ? invoke<CoachReport>("coach_run", { token }) : demo.run(tournamentId),
  cancel: () => call<void>("coach_cancel", {}, demo.cancel),
  reports: (tournaments: TournamentRow[]) =>
    call<ReportListItem[]>("coach_reports", {}, () => demo.list(new Map(tournaments.map((t) => [t.id, t])))),
  report: (tournamentId: number) => call<CoachReport | null>("coach_report", { tournamentId }, () => demo.get(tournamentId)),
  deleteReport: (tournamentId: number) => call<boolean>("coach_delete_report", { tournamentId }, () => demo.remove(tournamentId)),
  /** Concepts of all stored reports, weakest first ("Deine Schwachstellen"). */
  concepts: () => call<ConceptCount[]>("coach_concepts", {}, demo.concepts),
};

export async function onProgress(handler: (p: CoachProgress) => void): Promise<() => void> {
  return inTauri ? on<CoachProgress>("coach-progress", handler) : demo.onProgress(handler);
}

export const EFFORTS: { id: Effort; name: string; hint: string }[] = [
  { id: "high", name: "Gründlich", hint: "Denkt am längsten nach. Ein Schritt kann mehrere Minuten dauern." },
  { id: "medium", name: "Ausgewogen", hint: "Etwa halb so viel Nachdenken, spürbar schneller." },
  { id: "low", name: "Schnell", hint: "Kurzes Nachdenken: am schnellsten und günstigsten, dafür weniger tief." },
];

export const effortName = (e: Effort) => EFFORTS.find((x) => x.id === e)?.name ?? e;

/** 45 s, 2:05 min. */
export const duration = (seconds: number) => {
  const s = Math.max(0, Math.floor(seconds));
  return s < 60 ? `${s} s` : `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")} min`;
};

/** Tauri rejects with the command's error string. */
export const errorText = (e: unknown) => (typeof e === "string" ? e : e instanceof Error ? e.message : "Unbekannter Fehler.");

export const usd = (v: number | null | undefined, digits = 2) =>
  v == null ? "–" : `${v.toLocaleString("de-DE", { minimumFractionDigits: digits, maximumFractionDigits: digits })} $`;

/** Small amounts with enough digits to be meaningful: 0,098 $ rather than 0,10 $. */
export const cost = (v: number | null | undefined) => (v == null ? "–" : usd(v, v < 1 ? 3 : 2));

export const tokens = (n: number) =>
  n >= 10000 ? `${(n / 1000).toLocaleString("de-DE", { maximumFractionDigits: 1 })} Tsd.` : n.toLocaleString("de-DE");
