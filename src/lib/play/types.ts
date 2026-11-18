// Mirrors crates/ringside-play (hand.rs, pool.rs, table.rs) as serialised by src-tauri/src/play.rs.
import type { SolverNode, SolverStep } from "../solver/types";

export type PlayMode = "full" | "push_fold" | "postflop";
export type PlayAnte = "none" | "classic" | "big_blind";

export interface PlayConfig {
  mode: PlayMode;
  players: number;
  ante: PlayAnte;
  stack_bb: number;
  mixed_stacks: boolean;
  /** Seat in preflop action order (0 = first to act); null deals it at random. */
  hero_seat: number | null;
}

export type Street = "preflop" | "flop" | "turn" | "river";
export type ChoiceKind = "fold" | "check" | "call" | "limp" | "bet" | "raise" | "all_in";
export type Grade = "best" | "mix" | "good" | "inaccurate" | "mistake" | "blunder";
export type Source = "model" | "equilibrium" | "solver" | "library" | "saved";
export type EndReason = "folded" | "showdown" | "left_line" | "flop_unsolved" | "not_stored";

export interface Choice {
  kind: ChoiceKind;
  to_bb: number;
  add_bb: number;
  pct_pot: number | null;
}

export interface Snapshot {
  street: Street;
  board: string[];
  bets: number[];
  stacks: number[];
  pot: number;
  folded: boolean[];
  to_act: number | null;
}

export type PlayEvent =
  | { type: "action"; street: Street; seat: number; choice: Choice; verdict: number | null; snapshot: number }
  | { type: "board"; street: Street; cards: string[]; snapshot: number };

export interface Grid {
  /** Per hand class (13×13 index): share of its combos that reaches the decision. */
  reach: number[];
  /** Per hand class: frequency of each choice. */
  strategy: number[][];
}

export interface Judgement {
  grade: Grade;
  /** Highest EV (most frequent without EVs). */
  best: number;
  /** The move the solution plays most often with the hand. */
  main: number;
  frequency: number;
  loss_bb: number | null;
  loss_pct: number | null;
}

export interface Lesson {
  street: Street;
  choices: Choice[];
  source: Source;
  /** Null: the solution never has this hand here, so nothing grades the move. */
  strategy: number[] | null;
  evs: number[] | null;
  pot: number;
  to_call: number;
  equity: number | null;
  /** Hand class, e.g. "KQs". */
  class: string;
  node: SolverNode | null;
  grid: Grid | null;
  path: SolverStep[] | null;
  exploitability_pct: number | null;
}

export interface Verdict extends Lesson {
  chosen: number;
  judgement: Judgement | null;
  hinted: boolean;
  points: number | null;
}

export interface Outcome {
  reason: EndReason;
  winners: number[];
  net: number[];
  shown: (string[] | null)[];
  made: (string | null)[];
}

export interface Decision {
  street: Street;
  choices: Choice[];
  pot: number;
  to_call: number;
  hinted: boolean;
}

export interface SpotInfo {
  id: number;
  source: Source;
  complete: boolean;
  exploitability_pct: number;
  oop: number;
  ip: number;
  pot: number;
  stack: number;
}

export interface HandView {
  seed: number;
  config: PlayConfig;
  players: number;
  positions: string[];
  stack_bb: number;
  ante: number[];
  hero: number;
  hero_cards: string[];
  board: string[];
  events: PlayEvent[];
  snapshots: Snapshot[];
  verdicts: Verdict[];
  decision: Decision | null;
  outcome: Outcome | null;
  preflop_source: Source;
  spot: SpotInfo | null;
  waiting: boolean;
}

export type PoolState = "waiting" | "running" | "paused_client" | "done" | "cancelled" | "error";

export interface PoolStatus {
  status: PoolState;
  title: string;
  ready: number;
  target: number;
  fraction: number;
  eta_secs: number | null;
  next_secs: number | null;
  solve_secs: number | null;
  solved: number;
  error: string | null;
}

export interface OpenedSpot {
  spot: import("../solver/types").SolverSpot;
  summary: import("../solver/types").SolverSummary;
  oop: string;
  ip: string;
  line: { step: SolverStep; kind: import("../solver/types").SolverActionKind | null; amount: number }[];
  hero: ["oop" | "ip", string[]] | null;
}

export interface PastHand {
  seed: number | null;
  at: number;
  score: number;
  ev_loss_bb: number | null;
  mode: PlayMode;
  players: number;
  stack_bb: number;
  position: string;
  cards: string;
  board: string;
  grades: (Grade | null)[];
  net_bb: number;
  postflop: boolean;
}
