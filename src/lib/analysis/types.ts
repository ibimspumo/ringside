import type { SkillChange } from "../skill/types";
import type { MoveKind } from "../play/words";
import type { Street } from "../types";

export type Rating = "best" | "mix" | "good" | "inaccuracy" | "mistake" | "blunder";
export type Source = "equilibrium" | "model" | "solver" | "estimate" | "forced";
export type DecisionStatus = "rated" | "pending" | "forced" | "unrated";
export type SpotType =
  | "open"
  | "steal"
  | "three_bet"
  | "vs_open"
  | "vs_limp"
  | "vs_three_bet"
  | "shove"
  | "call_shove"
  | "c_bet"
  | "vs_c_bet"
  | "flop"
  | "turn"
  | "river";

export interface Choice {
  label: string;
  /** Share of the time the source takes it with the hero's hand, 0–1. */
  freq: number;
  ev_bb: number | null;
  played: boolean;
  kind: MoveKind | "";
}

export interface DecisionBrief {
  action_index: number;
  street: Street;
  spot: SpotType;
  status: DecisionStatus;
  played: string;
  best: string | null;
  ev_loss_bb: number | null;
  rating: Rating | null;
  source: Source | null;
  reason: string | null;
  /** The reason in plain words (equity, range); the headline is built where it is shown. */
  why: string;
  /** Pot at the decision, BB: grades are cut in percent of it. */
  pot_bb: number | null;
  /** For the estimate: share of the checked spots where its grade was at most one step off. */
  reliability: number | null;
}

export interface Decision extends DecisionBrief {
  hand_id: number;
  strategy: Choice[];
  played_freq: number | null;
  /** Assumptions and how far the hand is from what was computed. */
  note: string;
  /** JSON: the flop spot for the solver page plus the path to this decision. */
  solver: string | null;
  engine: string;
}

export interface DecisionFilter {
  min_rating: Rating | null;
  min_loss_bb: number | null;
  spots: SpotType[];
  street: Street | null;
  source: Source | null;
  unrated: boolean;
}

export interface AnalysisCounts {
  hands: number;
  analyzed: number;
  decisions: number;
  rated: number;
  pending: number;
  queued: number;
}

export interface Solving {
  hand_id: number;
  iteration: number;
  max_iterations: number;
  exploitability_pct: number;
  elapsed_ms: number;
}

export type JobState = "waiting" | "running" | "paused_pokerstars" | "done" | "cancelled" | "failed";

/** A long-running task in plain words, as sent with `analysis-progress`. */
export interface JobProgress {
  id: string;
  title: string;
  detail: string;
  /** Done share 0–1, when known. */
  share: number | null;
  eta_secs: number | null;
  state: JobState;
}

export interface AnalysisStatus {
  counts: AnalysisCounts;
  enabled: boolean;
  state: "idle" | "preflop" | "postflop" | "locked";
  solving: Solving | null;
  eta_secs: number | null;
  marked: number[];
  threads: number;
  last_error: string | null;
  cancelled: boolean;
  /** The automatic run over the new hands is on (it may wait for PokerStars to close). */
  auto_run: boolean;
  progress: JobProgress;
}

/** The automatic run rated new hands, sent as `analysis-auto-done`. */
export interface AutoDone {
  hands: number;
  /** Rated decisions that are mistakes or worse. */
  mistakes: number;
  /** The run's entry in the task center, whose own note this one replaces. */
  job: number | null;
  /** How far the new hands moved the Spielwertung. */
  skill: SkillChange | null;
}

export interface SpotLoss {
  spot: SpotType | null;
  decisions: number;
  rated: number;
  /** Inaccuracies or worse. */
  flawed: number;
  loss_bb: number;
}

export interface CostlyDecision extends DecisionBrief {
  hand_id: number;
  played_at: string;
  hero_cards: string;
  position: string | null;
}

export interface AnalysisSummary {
  hands: number;
  analyzed: number;
  decisions: number;
  rated: number;
  loss_bb: number;
  /** Rated decisions per rating, best first. */
  ratings: number[];
  by_source: [Source, number][];
  by_spot: SpotLoss[];
  costliest: CostlyDecision[];
}
