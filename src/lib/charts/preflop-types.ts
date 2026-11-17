// Preflop model charts (deeper stacks), mirrored from src-tauri/src/charts.rs.
import type { PushFoldAnte } from "../types";

export type PreflopAnte = PushFoldAnte;

export type PreflopKind = "fold" | "check" | "call" | "limp" | "raise" | "all_in";

export interface PreflopGrid {
  players: number[];
  stacks: number[];
  /** Ante per player and of the big blind alone, in BB. */
  antes: { ante: PreflopAnte; ante_bb: number; bb_ante_bb: number }[];
  /** Stacks off the grid are solved live only up to this many players. */
  live_max_players: number;
  live_min_stack_bb: number;
  live_max_stack_bb: number;
  target_exploitability_bb: number;
}

export interface PreflopStep {
  seat: number;
  position: string;
  kind: PreflopKind;
  /** Chips in front after the action (blind included, ante not), BB. */
  to_bb: number;
}

export interface PreflopAction {
  kind: PreflopKind;
  to_bb: number;
  /** Share of the range reaching the spot that takes it, in percent. */
  pct: number;
}

export interface PreflopSpot {
  id: number;
  seat: number;
  position: string;
  /** Non-fold actions before this spot; every other seat that acted folded. */
  line: PreflopStep[];
  actions: PreflopAction[];
  reach: number;
}

export interface PreflopRealized {
  line: PreflopStep[];
  /** First to act after the flop first. */
  positions: string[];
  reach: number;
  pot_bb: number;
  spr: number;
  equity: number[];
  realization: number[];
}

export interface PreflopSizes {
  open_bb: number;
  sb_open_bb: number;
  iso_bb: number;
  three_bet_ip: number;
  three_bet_oop: number;
  squeeze_per_caller: number;
  four_bet: number;
  open_jam_max_bb: number;
  three_bet_jam_max_bb: number;
  four_bet_jam_max_bb: number;
  commit: number;
}

export interface PreflopRealization {
  position: number;
  initiative: number;
  pair: number;
  suited: number;
  offsuit: number;
  connected: number;
  gapped: number;
  sharpness: number;
  spr_scale: number;
}

export interface PreflopSpots {
  players: number;
  /** Position names by seat. */
  positions: string[];
  stack_bb: number;
  ante: PreflopAnte;
  ante_bb: number;
  bb_ante_bb: number;
  /** False: solved live because the grid has no such depth. */
  precomputed: boolean;
  converged: boolean;
  exploitability_bb: number;
  iterations: number;
  ev_sum_error_bb: number;
  sizes: PreflopSizes;
  realization: PreflopRealization;
  realized: PreflopRealized[];
  nodes: PreflopSpot[];
}

export interface PreflopNode {
  id: number;
  /** Frequency per action and hand class (row * 13 + col, ace first), 0–255; all 0 for a hand out of range here. */
  freq: number[][];
  /** EV per action and hand class, hundredths of a BB against the starting stack. */
  ev: number[][];
}

/** Event `preflop-progress` while a depth off the grid is solved live. */
export interface PreflopProgress {
  ante: PreflopAnte;
  players: number;
  stack_bb: number;
  iterations: number;
  max_iterations: number;
  /** Best NashConv checked so far, BB per hand. */
  exploitability: number;
  target_exploitability: number;
}
