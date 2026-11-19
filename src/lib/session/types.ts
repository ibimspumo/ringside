// Mirrors crates/ringside-core/src/session.rs and `SessionPage` in src-tauri/src/report.rs.
import type { Rating, Source } from "../analysis/types";
import type { History } from "../ranges/types";
import type { Street, TournamentReport } from "../report/types";
import type { SkillChange } from "../skill/types";
import type { TournamentRow } from "../types";

export interface SessionSpan {
  start: string;
  end: string;
  hands: number;
  tournaments: number[];
  cash_hands: number;
}

export type Sample = "few" | "rough" | "solid";

export interface ClassGap {
  /** row * 13 + col, ace first. */
  class: number;
  name: string;
  hands: number;
  played: number;
  /** Hands the model would have played. */
  model: number;
  loss_bb: number;
}

export interface Pattern {
  text: string;
  wide: boolean;
  decisions: number;
  played: number;
  model: number;
  loss_bb: number;
  history: History;
  position: string | null;
  vs_position: string | null;
  /** Playing-profile key for "Lernen". */
  practice: string | null;
}

export interface Verdict {
  /** 0 strong … 3 many mistakes. */
  level: 0 | 1 | 2 | 3;
  title: string;
  line: string;
}

export interface PreflopReview {
  hands: number;
  compared: number;
  played: number;
  model_played: number;
  equilibrium: number;
  you_pct: number | null;
  model_pct: number | null;
  classes: ClassGap[];
  wide: number[];
  tight: number[];
  patterns: Pattern[];
  sample: Sample | null;
  decisions: number;
  good: number;
  loss_bb: number;
  verdict: Verdict | null;
}

export interface Swing {
  hand_id: number;
  action_index: number;
  tournament_id: number | null;
  hero_cards: string;
  position: string | null;
  street: Street;
  played: string;
  best: string | null;
  rating: Rating | null;
  source: Source | null;
  why: string;
  net_bb: number;
  ev_bb: number | null;
  luck_bb: number | null;
  loss_bb: number;
  margin_bb: number | null;
  alternative: string | null;
  equity_pct: number | null;
  opponents: string[][];
  board: string[];
}

export type SessionReport = Omit<TournamentReport, "tournament" | "coach_review">;

export interface SessionReview {
  span: SessionSpan;
  report: SessionReport;
  preflop: PreflopReview;
  bad_luck: Swing[];
  good_luck: Swing[];
  locked: boolean;
}

export interface SessionPage {
  sessions: SessionSpan[];
  review: SessionReview | null;
  tournaments: TournamentRow[];
  /** How far the session moved the Spielwertung; null while locked. */
  skill: SkillChange | null;
}

/** What a link from the review narrows the other pages to: this session's hands. */
export interface SessionScope {
  since: string;
  until: string;
  label: string;
}

/** A situation for "Deine Ranges", as a pattern names it. */
export interface RangePreset {
  history: History | null;
  position: string | null;
  vs: string | null;
}
