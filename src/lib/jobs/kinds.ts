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


export function statusText(j: Job, locked: boolean): string {
  switch (j.status) {
    case "running":
      return j.cancelling ? "wird abgebrochen …" : j.progress === null ? "läuft" : `${Math.floor(j.progress * 100)} %`;
    case "paused":
      return locked ? "pausiert, Poker-Client läuft" : "pausiert";
    case "done":
      return "fertig";
    case "cancelled":
      return j.by_client ? "gestoppt, PokerStars lief" : "abgebrochen";
    case "failed":
      return "fehlgeschlagen";
  }
}
