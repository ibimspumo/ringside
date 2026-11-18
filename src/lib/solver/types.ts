// Mirrors crates/ringside-solver; amounts in big blinds.

export interface StreetSizes {
  /** First bets, in percent of the pot. */
  bet: number[];
  /** Raises, in percent of the pot after calling. */
  raise: number[];
}

export interface SolverSpot {
  oop_range: string;
  ip_range: string;
  board: string;
  pot: number;
  stack: number;
  /** Flop, turn, river. */
  sizes: [StreetSizes, StreetSizes, StreetSizes];
  /** All-in is offered once it is at most this percentage of the pot. */
  allin_threshold: number;
}

export interface SolverSettings {
  target_pct: number;
  max_iterations: number;
  threads: number;
}

export interface SolverEstimate {
  memory_bytes: number;
  compressed: boolean;
  memory_limit: number;
  fits: boolean;
  combos: [number, number];
  seconds: number;
}

export interface SolverProgress {
  run: number;
  iteration: number;
  max_iterations: number;
  exploitability: number;
  exploitability_pct: number;
  elapsed_ms: number;
}

export interface SolverSummary {
  iterations: number;
  exploitability: number;
  exploitability_pct: number;
  converged: boolean;
  elapsed_ms: number;
  memory_bytes: number;
  compressed: boolean;
  threads: number;
}

export type SolverDone =
  /** `saved`: id in "Meine Lösungen"; `reused`: opened from there instead of solving. */
  | { status: "solved"; run: number; summary: SolverSummary; saved: number | null; reused: boolean }
  | { status: "cancelled"; run: number; locked: boolean }
  | { status: "failed"; run: number; error: string };

export type SolverStep = { action: number } | { card: string };

/** What the page knows about a spot for "Meine Lösungen". */
export interface SolveInfo {
  title: string;
  oop: string;
  ip: string;
  ranges: string;
  source: "own" | "hand" | "library" | "training" | null;
  hand_id: number | null;
  /** The preflop line the ranges come from, when they are the model's own: shared then. */
  model?: ModelLine | null;
}
/** Mirrors `ModelLine` in ringside-engine: a table and the non-fold actions to the flop. */
export interface ModelLine {
  family: "plain" | "limps";
  ante: string;
  players: number;
  stack_bb: number;
  line: { seat: number; action: { kind: string; to_bb: number } }[];
}
/** The model's own flop for a pick on the solver page. */
export interface ModelSpot {
  model: ModelLine;
  oop_range: string;
  ip_range: string;
  pot: number;
  stack: number;
}
export type SolverPlayer = "oop" | "ip";
export type SolverActionKind = "fold" | "check" | "call" | "bet" | "raise" | "all_in";

export interface SolverAction {
  kind: SolverActionKind;
  amount: number;
  frequency: number;
  combos: number;
}

export interface SolverHand {
  combo: string;
  class: number;
  weight: number;
  strategy: number[];
  equity: number;
  ev: number;
  /** `ev` after each action, in the order of `SolverNode.actions`; empty when not stored. */
  evs: number[];
}

export interface SolverClass {
  class: number;
  label: string;
  combos: number;
  strategy: number[];
  equity: number;
  ev: number;
}

export interface SolverNode {
  path: SolverStep[];
  kind: "action" | "chance" | "terminal";
  street: "flop" | "turn" | "river";
  board: string[];
  pot: number;
  bets: [number, number];
  stacks: [number, number];
  /** The part of `bets` put in on the current street: the chips in front of each player. */
  street_bets: [number, number];
  player: SolverPlayer | null;
  actions: SolverAction[];
  cards: string[];
  hands: SolverHand[];
  classes: SolverClass[];
  players: [PlayerSummary, PlayerSummary];
}

export interface PlayerSummary {
  combos: number;
  equity: number;
  ev: number;
}

/** A spot handed over from elsewhere in the app, e.g. the replayer. */
export interface SolverPrefill {
  board: string[];
  pot: number;
  stack: number;
  oop: { position: string; range: string; note: string };
  ip: { position: string; range: string; note: string };
  /** Hero's cards, highlighted in the grid when hero is in the hand. */
  hero: { player: SolverPlayer; cards: string[] } | null;
  /** Seats at the table the hand was played on, to draw it. */
  players?: number;
  source: string;
  /** Sizes and all-in threshold the spot was solved with; otherwise the page's defaults. */
  sizes?: [StreetSizes, StreetSizes, StreetSizes];
  allin_threshold?: number;
  /** From the hand analysis: the line to the decision, opened at once instead of the setup. */
  path?: SolverStep[];
  /** The saved solution of this spot in "Meine Lösungen", if there is one. */
  saved?: number | null;
}
