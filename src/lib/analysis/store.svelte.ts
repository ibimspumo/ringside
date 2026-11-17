import { api, onAnalysisHand, onAnalysisStatus } from "../api";
import type { AnalysisStatus } from "./types";

/**
 * The analysis status shared by every view, kept current by the worker's events. `solved`
 * counts finished postflop spots so lists can reload; `request` is a hand another page
 * asked to analyse, picked up by the hands page when it opens it.
 */
export const analysis = $state<{
  status: AnalysisStatus | null;
  solved: number;
  lastSolved: number | null;
  request: number | null;
}>({ status: null, solved: 0, lastSolved: null, request: null });

let started = false;

export function watchAnalysis() {
  if (started) return;
  started = true;
  const load = () =>
    api.analysisStatus().then(
      (s) => (analysis.status = s),
      () => {},
    );
  load();
  onAnalysisStatus((s) => (analysis.status = s));
  onAnalysisHand((id) => {
    analysis.lastSolved = id;
    analysis.solved++;
    load();
  });
}

/** Opens the hands page on `handId` and analyses it there. */
export function requestAnalysis(handId: number) {
  analysis.request = handId;
}
