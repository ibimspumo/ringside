/** A longer calculation in the task center, see `src-tauri/src/jobs.rs`. */
export type JobKind = "solver" | "preflop" | "push_fold" | "library" | "packs" | "coach" | "rebuild" | "analysis" | "training" | "play";
export type JobStatus = "running" | "paused" | "done" | "cancelled" | "failed";

/** A page of the app and, for some, what to open there. */
export interface JobTarget {
  page: string;
  id: number | null;
}

export interface Job {
  id: number;
  kind: JobKind;
  title: string;
  detail: string | null;
  status: JobStatus;
  /** 0–1, null while nobody can say. */
  progress: number | null;
  steps: [number, number] | null;
  eta_secs: number | null;
  elapsed_ms: number;
  started_at: number;
  finished_at: number | null;
  cancellable: boolean;
  cancelling: boolean;
  /** Ended because PokerStars started. */
  by_client: boolean;
  error: string | null;
  target: JobTarget;
}

export interface JobsView {
  jobs: Job[];
  /** PokerStars runs; calculations wait or stop. */
  locked: boolean;
}
