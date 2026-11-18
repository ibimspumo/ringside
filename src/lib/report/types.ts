// Mirrors `TournamentReport` in src-tauri/src/report.rs and `Report` in ringside-core/src/report.rs.
import type { Rating, Source, SpotType } from "../analysis/types";
import type { TournamentRow } from "../types";

export type Street = "preflop" | "flop" | "turn" | "river";

export interface Luck {
  /** Chips (hundredths) won or lost over the hands. */
  net: number;
  /** The same with every all-in at its expectation. */
  ev: number;
  /** net − ev: above zero the cards ran well. */
  luck: number;
  net_bb: number;
  ev_bb: number;
  luck_bb: number;
  all_ins: number;
}

export interface Score {
  decisions: number;
  good: number;
  inaccuracies: number;
  mistakes: number;
  /** Rated decisions per rating, best first. */
  ratings: number[];
  loss_bb: number;
  /** Loss per 100 rated decisions, BB. */
  per100: number | null;
  good_pct: number | null;
  estimate_loss_pct: number;
}

export interface CostGroup {
  key: string;
  title: string;
  decisions: number;
  good: number;
  flawed: number;
  loss_bb: number;
  estimate_loss_pct: number;
  unsure: boolean;
  spots: SpotType[];
}

export interface Breakdown {
  key: "situation" | "seat" | "street" | "stack";
  title: string;
  groups: CostGroup[];
}

export interface Moment {
  hand_id: number;
  action_index: number;
  /** 1-based hand number in the tournament. */
  n: number;
  played_at: string;
  hero_cards: string;
  position: string | null;
  street: Street;
  spot: SpotType;
  situation: string;
  stack_bb: number | null;
  played: string;
  best: string | null;
  why: string;
  rating: Rating | null;
  source: Source | null;
  loss_bb: number;
  margin_bb: number | null;
  alternative: string | null;
}

export interface StartingHand {
  class: string;
  hands: number;
  net_bb: number;
  loss_bb: number;
  flawed: number;
  hand_ids: number[];
}

export interface StackPoint {
  n: number;
  hand_id: number;
  /** Chips (hundredths) at the start of the hand. */
  stack: number | null;
  big_blind: number;
  net: number;
  loss_bb: number;
  /** Rank among the costliest mistakes when one is in this hand. */
  mistake: number | null;
}

export interface Practice {
  /** Group key of the playing profile, for "Lernen". */
  key: string;
  title: string;
  detail: string;
  lage: string;
  decisions: number;
  flawed: number;
  loss_bb: number;
}

export interface PastTournament {
  tournament_id: number;
  started_at: string;
  name: string | null;
  decisions: number;
  per100: number | null;
  good_pct: number | null;
}

export interface Average {
  tournaments: number;
  decisions: number;
  per100: number | null;
  good_pct: number | null;
}

export interface TournamentReport {
  tournament: TournamentRow;
  hands: number;
  unanalyzed: number;
  pending: number;
  unrated: number;
  luck: Luck;
  score: Score;
  breakdowns: Breakdown[];
  worst: Moment[];
  best: Moment[];
  starting: StartingHand[];
  timeline: StackPoint[];
  practice: Practice | null;
  history: PastTournament[];
  average: Average | null;
  /** A language-model review of this tournament is stored. */
  coach_review: boolean;
}

/** What a question to the coach is about. */
export type AskAbout = { kind: "tournament"; tournament_id: number } | { kind: "hand"; hand_id: number; action_index: number | null };

export interface AskEstimate {
  token: string;
  model: string;
  question: string;
  /** Exactly what goes to the model besides the fixed instructions. */
  sent: string;
  prompt_tokens: number;
  completion_tokens: number;
  max_completion_tokens: number;
  cost_usd: number | null;
  max_cost_usd: number | null;
}

export interface AskAnswer {
  text: string;
  /** Figures of the answer that are not among the facts. */
  unchecked: string[];
  cost_usd: number;
  model: string;
}
