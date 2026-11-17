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
  | { status: "solved"; run: number; summary: SolverSummary }
  | { status: "cancelled"; run: number; locked: boolean }
  | { status: "failed"; run: number; error: string };

export type SolverStep = { action: number } | { card: string };
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
  source: string;
}
