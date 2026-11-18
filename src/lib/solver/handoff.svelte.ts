import { api } from "../api";
import { duration } from "./actions";
import { openSaved } from "./solutions";
import { applyPrefill, solver, startSolve } from "./store.svelte";
import type { SolverPrefill, SolverStep } from "./types";

/** A spot to open in the solver; the main view switches there and the page takes it. */
export const handoff = $state<{ prefill: SolverPrefill | null }>({ prefill: null });

export function openInSolver(prefill: SolverPrefill) {
  handoff.prefill = prefill;
}

/**
 * Takes a spot sent to the solver page. One with a `path` (from the hand analysis) opens its
 * solution, saved or solved now, and walks the hand's line to the decision; the others fill
 * in the setup.
 */
export async function takePrefill(p: SolverPrefill) {
  applyPrefill(p);
  if (p.sizes) solver.spot = { ...solver.spot, sizes: p.sizes, allin_threshold: p.allin_threshold ?? solver.spot.allin_threshold };
  if (!p.path) return;
  const keep = { hero: p.hero, source: p.source, notes: { ...solver.notes }, positions: { ...solver.positions } };
  const replay = { line: [], then: null, path: p.path };
  if (p.saved != null) {
    try {
      await openSaved(p.saved);
      Object.assign(solver, keep, { replay });
      return;
    } catch {
      // Deleted to fit the disk limit since; solved again below.
    }
  }
  const then = `Danach springt die Ansicht direkt zu deiner Entscheidung am ${streetAt(p.path)}.`;
  solver.replay = { ...replay, notice: await solveNotice("Ringside rechnet den Spot dieser Hand", then) };
  await startSolve();
}

/** The street a line ends on. */
export function streetAt(path: SolverStep[]): "Flop" | "Turn" | "River" {
  return (["Flop", "Turn", "River"] as const)[Math.min(2, path.filter((s) => "card" in s).length)];
}

/** `what` with the solver's own time estimate, then `then`. */
export async function solveNotice(what: string, then: string): Promise<string> {
  const est = await api.solverEstimate($state.snapshot(solver.spot), solver.settings.threads).catch(() => null);
  return `${what}${est ? `, ca. ${duration(est.seconds)}` : ""}. ${then}`;
}
