/** Mirrors `ringside_coach` (crates/ringside-coach). */

export type Verdict = "stark" | "ok" | "fraglich" | "fehler";
export type FigureSource = "berechnet" | "einschätzung";

export interface Figure {
  label: string;
  value: string;
  source: FigureSource;
}

export interface CoachSpot {
  hand_id: number;
  played_at: string;
  hero_cards: string[];
  /** The board as far as it was dealt when the hero decided (all of it after an all-in). */
  board: string[];
  position: string | null;
  stack_bb: number;
  action: string;
  verdict: Verdict;
  better: string | null;
  why: string;
  numbers: Figure[];
  /** Id from `CONCEPTS`. */
  concept: string;
}

export interface CoachMeta {
  model: string;
  created_at: number;
  prompt_tokens: number;
  completion_tokens: number;
  cost_usd: number | null;
  seconds: number;
  steps: number;
  tool_calls: number;
  hands_total: number;
  hands_sent: number;
  /** Figures the model called computed that no calculator produced; shown as estimates. */
  downgraded: number;
}

export interface CoachReport {
  tournament_id: number;
  summary: string;
  spots: CoachSpot[];
  focus: string[];
  meta: CoachMeta;
}

export interface ReportListItem {
  tournament_id: number;
  created_at: number;
  model: string;
  spots: number;
  /** Spots judged "fehler" or "fraglich". */
  mistakes: number;
  cost_usd: number | null;
  name: string | null;
  game: string;
  started_at: string;
}

export interface Estimate {
  token: string;
  tournament_id: number;
  model: string;
  hands_total: number;
  hands_sent: number;
  hands_without_decision: number;
  hands_dropped: number;
  spots: number;
  input_tokens: number;
  prompt_tokens: number;
  completion_tokens: number;
  max_prompt_tokens: number;
  max_completion_tokens: number;
  prompt_per_m: number | null;
  completion_per_m: number | null;
  cost_usd: number | null;
  max_cost_usd: number | null;
}

export interface CoachProgress {
  tournament_id: number;
  step: number;
  max_steps: number;
  stage: "thinking" | "tool" | "retry" | "fixing";
  message: string;
  tool: string | null;
  tool_calls: number;
  prompt_tokens: number;
  completion_tokens: number;
  cost_usd: number;
}

export interface ModelInfo {
  id: string;
  name: string;
  /** US dollars per million tokens. */
  prompt_per_m: number | null;
  completion_per_m: number | null;
  context_length: number | null;
  tools: boolean;
  reasoning: boolean;
}

export interface KeyInfo {
  label: string | null;
  usage: number | null;
  limit: number | null;
  free_tier: boolean;
}

export interface CoachStatus {
  has_key: boolean;
  model: string;
  running: boolean;
}

/** How often a concept came up in stored reports, weakest first. */
export interface ConceptCount {
  id: string;
  name: string;
  /** Exercise kind in the learning area. */
  exercise: string | null;
  articles: string[];
  spots: number;
  mistakes: number;
  questionable: number;
  tournaments: number;
}
