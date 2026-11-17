import type { SolverPrefill } from "./types";

/** A spot to open in the solver; the main view switches there and the page takes it. */
export const handoff = $state<{ prefill: SolverPrefill | null }>({ prefill: null });

export function openInSolver(prefill: SolverPrefill) {
  handoff.prefill = prefill;
}
