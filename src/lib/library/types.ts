// Mirrors src-tauri/src/library.rs.
import type { SolverSpot, SolverSummary } from "../solver/types";

export type LibraryMode = "off" | "idle" | "always";
export type SpotSource = "local" | "cloud";
export type WorkerState = "off" | "waiting" | "packs" | "cloud" | "solving" | "done";

export interface WorkerStatus {
  state: WorkerState | "";
  reason: string | null;
  scenario: string | null;
  flop: string | null;
  iteration: number;
  exploitability_pct: number;
  elapsed_ms: number;
  threads: number;
  solved: number;
  fetched: number;
  shared: number;
  last_error: string | null;
}

export interface LibraryScenario {
  id: string;
  pot_type: "srp" | "three_bet";
  oop: string;
  ip: string;
  stack_bb: number;
  pot: number;
  stack: number;
  /** Per catalog flop. */
  status: (SpotSource | null)[];
}

export interface LibraryOverview {
  catalog: number;
  target_pct: number;
  flops: string[];
  flop_weights: number[];
  scenarios: LibraryScenario[];
  disk_bytes: number;
  mode: LibraryMode;
  worker: WorkerStatus;
}

export interface OpenedSpot {
  spot: SolverSpot;
  summary: SolverSummary;
  oop: string;
  ip: string;
  oop_note: string;
  ip_note: string;
  source: SpotSource;
}
