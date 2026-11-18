import type { Job, JobKind } from "./types";

/** What each kind of task is, in a word, and the page its "Ansehen" opens. */
export const KIND_LABEL: Record<JobKind, string> = {
  solver: "Solver",
  preflop: "Preflop-Chart",
  push_fold: "Push/Fold-Chart",
  library: "Bibliothek",
  packs: "Download",
  coach: "Coach",
  rebuild: "Datenbank",
  analysis: "Hand-Analyse",
  training: "Training",
  play: "Hände spielen",
};

export const PAGE_LABEL: Record<string, string> = {
  solver: "Solver",
  charts: "Ranges nach Stack",
  library: "Bibliothek",
  coach: "Coach",
  hands: "Hände",
  dashboard: "Rückblick",
  overview: "Übersicht",
  training: "Training",
  play: "Hände spielen",
};

export const isActive = (j: Job) => j.status === "running" || j.status === "paused";

/** "noch etwa 3 Min." – rounded so the number doesn't flicker every second. */
export function etaText(secs: number | null): string | null {
  if (secs === null || !Number.isFinite(secs)) return null;
  if (secs < 10) return "gleich fertig";
  if (secs < 60) return `noch etwa ${Math.ceil(secs / 10) * 10} s`;
  if (secs < 3600) return `noch etwa ${Math.ceil(secs / 60)} Min.`;
  const h = Math.floor(secs / 3600);
  const m = Math.round((secs % 3600) / 60);
  return `noch etwa ${h} Std.${m ? ` ${m} Min.` : ""}`;
}

export function elapsedText(ms: number): string {
  const s = Math.round(ms / 1000);
  if (s < 60) return `${s} s`;
  if (s < 3600) return `${Math.floor(s / 60)} Min. ${s % 60 ? `${s % 60} s` : ""}`.trim();
  return `${Math.floor(s / 3600)} Std. ${Math.round((s % 3600) / 60)} Min.`;
}

export function statusText(j: Job, locked: boolean): string {
  switch (j.status) {
    case "running":
      return j.cancelling ? "wird abgebrochen …" : j.progress === null ? "läuft" : `${Math.floor(j.progress * 100)} %`;
    case "paused":
      return locked ? "pausiert, PokerStars läuft" : "pausiert";
    case "done":
      return "fertig";
    case "cancelled":
      return j.by_client ? "gestoppt, PokerStars lief" : "abgebrochen";
    case "failed":
      return "fehlgeschlagen";
  }
}
