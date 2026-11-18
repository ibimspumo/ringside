// "Deine Ranges", mirrored from crates/ringside-core/src/db/ranges.rs,
// crates/ringside-analysis/src/ranges.rs and src-tauri/src/ranges.rs.
import type { PushFoldAnte } from "../types";

export type History = "unopened" | "limped" | "vs_open" | "squeeze" | "vs_three_bet" | "vs_shove";
export type Move = "fold" | "call" | "raise" | "all_in";
export const MOVES: Move[] = ["fold", "call", "raise", "all_in"];

export interface RangeFilter {
  history: History | null;
  position: string | null;
  vs_position: string | null;
  /** Effective stack in BB: at least / below. */
  min_bb: number | null;
  max_bb: number | null;
  /** Players dealt in, inclusive. */
  min_players: number | null;
  max_players: number | null;
  /** Shortest stack still to act behind the hero, BB: at least / below. */
  min_behind_bb: number | null;
  max_behind_bb: number | null;
  /** One hand class (row * 13 + col, ace first), for the hand list. */
  class: number | null;
  action: Move | null;
}

export interface RangeGroup {
  history: History;
  position: string;
  vs_position: string | null;
  /** Whole BB, capped at 60. */
  stack: number;
  players: number;
  /** 0 below 10 BB, 1 below 25 BB, 2 deeper; null when nobody acts after the hero. */
  behind: number | null;
  count: number;
}

export interface RangeCounts {
  total: number;
  /** Per class: fold, call, raise, all-in. */
  cells: [number, number, number, number][];
  stack_bb: number | null;
  players: [number, number] | null;
  ante: [string, number] | null;
  line: [string, number] | null;
}

export interface ModelRange {
  source: "model" | "push_fold";
  players: number;
  stack_bb: number;
  ante: PushFoldAnte;
  /** Per class: fold, call, raise, all-in shares; null when the model never gets here with it. */
  cells: ([number, number, number, number] | null)[];
  raise_to_bb: number;
  all_in_bb: number;
  notes: string[];
}

export interface Comparison {
  you_pct: number;
  model_pct: number;
  missing: string[];
  extra: string[];
}

export interface RangeChart {
  counts: RangeCounts;
  model: ModelRange | null;
  comparison: Comparison | null;
  model_missing: string | null;
  locked: boolean;
}

export const NO_RANGE: RangeFilter = {
  history: null,
  position: null,
  vs_position: null,
  min_bb: null,
  max_bb: null,
  min_players: null,
  max_players: null,
  min_behind_bb: null,
  max_behind_bb: null,
  class: null,
  action: null,
};
