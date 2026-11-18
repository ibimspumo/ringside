// Mirrors src-tauri/src/library.rs.
import type { SolverSpot, SolverSummary } from "../solver/types";

export type LibraryMode = "off" | "idle" | "always";
export type LibraryPower = "gentle" | "normal" | "full";
export type WaitReason = "client" | "halted" | "off" | "idle" | "idle_unknown" | "in_use";
export type ShareState = "submitted" | "verified" | "rejected" | "failed";

/** What became of a spot solved here and offered to the cloud. */
export interface Shared {
  state: ShareState;
  /** Unix seconds of the upload or of the failed try. */
  at: number;
  /** The verifier's reason for a rejection, or the error of a failed upload. */
  note?: string;
}
export type SpotSource = "local" | "cloud";
export type WorkerState = "off" | "waiting" | "packs" | "cloud" | "solving" | "done";

export interface WorkerStatus {
  state: WorkerState | "";
  reason: WaitReason | null;
  /** Started by the user on the page. */
  manual: boolean;
  scenario: string | null;
  flop: string | null;
  iteration: number;
  exploitability_pct: number;
  /** Of the current spot, 0–1, once measured. */
  progress: number | null;
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
  /** Per catalog flop, for spots solved here: what became of sharing it, if known. */
  shared: (Shared | null)[];
}

export interface LibraryOverview {
  catalog: number;
  target_pct: number;
  flops: string[];
  flop_weights: number[];
  scenarios: LibraryScenario[];
  disk_bytes: number;
  mode: LibraryMode;
  power: LibraryPower;
  cores: number;
  /** The user shares own solutions. */
  share: boolean;
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
