import type { PushFoldAnte } from "../types";

/** A rated preflop decision's spot in the bundled charts (`analysis_chart_target`). */
export interface ChartTarget {
  action_index: number;
  kind: "pushfold" | "preflop";
  ante: PushFoldAnte;
  players: number;
  stack_bb: number;
  /** Hero's seat in the chart, in preflop action order. */
  seat: number;
  shover: number | null;
  callers: number[];
  /** Preflop model: id of the spot. */
  node: number | null;
  /** Hand class, e.g. "62o". */
  hand: string;
}

/**
 * A spot to open on the ranges page, and the hand it came from. The main view switches to
 * the page when `target` is set, and back to the hand when `back` is requested.
 */
export const chartsHandoff = $state<{ target: ChartTarget | null; hand: number | null; at: number | null; back: number }>({
  target: null,
  hand: null,
  at: null,
  back: 0,
});

export function openInCharts(target: ChartTarget, handId: number) {
  chartsHandoff.hand = handId;
  chartsHandoff.at = target.action_index;
  chartsHandoff.target = target;
}

export function backToHand() {
  chartsHandoff.back += 1;
}
