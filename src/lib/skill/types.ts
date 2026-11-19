// Mirrors crates/ringside-core/src/skill.rs.
import type { Rating, Source, SpotType } from "../analysis/types";

export interface SkillGrades {
  perfect: number;
  good: number;
  inaccurate: number;
  mistake: number;
  blunder: number;
}

export interface SkillEntry {
  /** "t:<id>" or "c:<YYYY/MM/DD>". */
  key: string;
  tournament_id: number | null;
  started_at: string;
  rating: number;
  sigma: number;
  delta: number;
  /** null: too few real decisions to count. */
  performance: number | null;
  decisions: number;
  real: number;
  burned_pct: number;
  grades: SkillGrades;
}

export type SkillArea = "push_fold" | "vs_all_in" | "open" | "blind_vs_blind" | "big_blind" | "vs_open" | "vs_three_bet" | "postflop" | "river";

export interface AreaScore {
  area: SkillArea;
  name: string;
  spots: SpotType[];
  decisions: number;
  real: number;
  rating: number;
  verdict: "strong" | "even" | "weak" | "few";
  burn_share_pct: number;
  grades: SkillGrades;
}

export interface Moment {
  hand_id: number;
  action_index: number;
  played_at: string;
  hero_cards: string;
  played: string;
  best: string | null;
  rating: Rating;
  source: Source;
  area: SkillArea;
  points: number;
  lost_pct: number;
  pot_pct: number;
  bad_luck: boolean;
}

export interface SkillProfile {
  rating: number | null;
  sigma: number;
  provisional: boolean;
  tier: string | null;
  /** (from, name), lowest first. */
  tiers: [number, string][];
  start: number;
  entries: SkillEntry[];
  grades: SkillGrades;
  areas: AreaScore[];
  moments: Moment[];
}

/** How far a run or a session moved the rating. */
export interface SkillChange {
  rating: number;
  sigma: number;
  tier: string;
  /** null: there was no rating before. */
  delta: number | null;
}
