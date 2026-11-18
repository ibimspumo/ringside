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
export type Source = "model" | "equilibrium" | "solver" | "live" | "library" | "saved" | "estimate";
export type EndReason = "folded" | "showdown";
/** Why a move has no grade: the hand never gets here in the solution, a multiway pot, a line the model does not play, a stopped live solve. */
export type Unrated = "not_reached" | "multiway" | "off_model" | "no_solve";
/** Why a hand waits for a live solve. */
export type SolveWhy = "new_line" | "no_spot" | "deeper";

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
  /** Null where nothing covers the decision. */
  source: Source | null;
  /** Null: nothing grades the move, `unrated` says why. */
  strategy: number[] | null;
  unrated: Unrated | null;
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
  /** For the estimate: how often its grade was within one step of the solver's. */
  reliability: number | null;
  /** Why the best move is best, where the source says it. */
  why: string | null;
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
  /** A source grades this decision; else it stays "nicht bewertet". */
  graded: boolean;
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
  /** The spot the hand waits for while it is solved live. */
  solving: Solving | null;
  /** Played on without a solution from here. */
  free: FreeInfo | null;
}

export interface Solving {
  why: SolveWhy;
  street: Street;
  oop: number;
  ip: number;
  pot: number;
  stack: number;
  /** Precision of the live solve, % of the pot. */
  target_pct: number;
}

export interface FreeInfo {
  why: Unrated;
  street: Street;
  /** How the opponents play from here. */
  rule: string;
  /** Why the live solve stopped. */
  note: string | null;
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

/** The live solve the hand waits for (urgency "now") or one solved ahead. */
export interface LiveStatus {
  urgency: "now" | "ahead";
  status: PoolState;
  fraction: number;
  secs_left: number | null;
  target_pct: number;
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
