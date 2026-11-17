import { onSolverDone, onSolverProgress } from "../api";
import { defaultSpot } from "./presets";
import type { SolverPrefill, SolverProgress, SolverSettings, SolverSpot, SolverSummary } from "./types";

export type Status = "idle" | "running" | "solved" | "cancelled" | "failed";

function defaultThreads(): number {
  const n = typeof navigator !== "undefined" ? navigator.hardwareConcurrency || 4 : 4;
  return Math.max(1, n - 1);
}

/** The solver page's state; it lives here so a running solve survives switching tabs. */
export const solver = $state({
  spot: defaultSpot() as SolverSpot,
  settings: { target_pct: 0.5, max_iterations: 1000, threads: defaultThreads() } as SolverSettings,
  /** Where each range came from, shown as an assumption when it was guessed. */
  notes: { oop: "", ip: "" },
  positions: { oop: "BB", ip: "BTN" },
  source: "",
  hero: null as SolverPrefill["hero"],
  view: "setup" as "setup" | "result",
  status: "idle" as Status,
  run: 0,
  progress: null as SolverProgress | null,
  /** First measured exploitability, the left end of the progress bar. */
  startPct: null as number | null,
  summary: null as SolverSummary | null,
  /** The spot the current solution belongs to. */
  solved: null as SolverSpot | null,
  error: null as string | null,
});

let listening = false;

export function listen() {
  if (listening) return;
  listening = true;
  onSolverProgress((p) => {
    if (p.run !== solver.run || solver.status !== "running") return;
    solver.progress = p;
    if (solver.startPct === null && p.iteration > 0) solver.startPct = p.exploitability_pct;
  });
  onSolverDone((d) => {
    if (d.run !== solver.run) return;
    if (d.status === "solved") {
      solver.status = "solved";
      solver.summary = d.summary;
      solver.view = "result";
    } else if (d.status === "cancelled") {
      solver.status = "cancelled";
      solver.error = d.locked ? "Abgebrochen, weil PokerStars gestartet wurde." : null;
    } else {
      solver.status = "failed";
      solver.error = d.error;
    }
  });
}

export function applyPrefill(p: SolverPrefill) {
  solver.spot = {
    ...solver.spot,
    board: p.board.join(""),
    pot: p.pot,
    stack: p.stack,
    oop_range: p.oop.range,
    ip_range: p.ip.range,
  };
  solver.notes = { oop: p.oop.note, ip: p.ip.note };
  solver.positions = { oop: p.oop.position, ip: p.ip.position };
  solver.source = p.source;
  solver.hero = p.hero;
  solver.view = "setup";
}
