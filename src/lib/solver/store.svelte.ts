import { api, onSolverDone, onSolverProgress, onSolverReleased } from "../api";
import type { Crumb } from "./play";
import { defaultSpot } from "./presets";
import { oopIp, positionsFor, type Scenario } from "./scenario";
import type { SolveInfo, SolverPlayer, SolverPrefill, SolverProgress, SolverSettings, SolverSpot, SolverStep, SolverSummary } from "./types";

/** What the table shows around a spot: its size, seats and the preflop action. */
export interface TableInfo {
  players: number;
  positions: { oop: string; ip: string };
  /** Short preflop action per position, e.g. "Open 2,2". */
  labels: Record<string, string>;
}

export type Status = "idle" | "running" | "solved" | "cancelled" | "failed";

function defaultThreads(): number {
  const n = typeof navigator !== "undefined" ? navigator.hardwareConcurrency || 4 : 4;
  return Math.max(1, n - 1);
}

/** Who sits at the bottom: the hero of a hand, else your choice ("Ich bin"), else OOP. */
export const meSide = (): SolverPlayer => solver.hero?.player ?? solver.me ?? "oop";

/** The solver page's state; it lives here so a running solve survives switching tabs. */
export const solver = $state({
  spot: defaultSpot() as SolverSpot,
  settings: { target_pct: 0.5, max_iterations: 1000, threads: defaultThreads() } as SolverSettings,
  /** Where each range came from, shown as an assumption when it was guessed. */
  notes: { oop: "", ip: "" },
  positions: { oop: "BB", ip: "BTN" },
  source: "",
  hero: null as SolverPrefill["hero"],
  /** Which of the two you are when no hand says so; you sit at the bottom of the table. */
  me: null as SolverPlayer | null,
  scenario: { players: 6, seats: ["BTN", "BB"], pot: "srp", stack: 40, ante: "big_blind" } as Scenario,
  /** Ranges changed by hand since they were derived. */
  custom: { oop: false, ip: false },
  /** Where the derived ranges came from, e.g. "aus dem Preflop-Modell, 8-max, 40 BB". */
  origin: "",
  labels: {} as Record<string, string>,
  view: "setup" as "setup" | "result" | "practice" | "saved",
  status: "idle" as Status,
  run: 0,
  progress: null as SolverProgress | null,
  /** First measured exploitability, the left end of the progress bar. */
  startPct: null as number | null,
  summary: null as SolverSummary | null,
  /** The solution on screen in "Meine Lösungen", and whether it opened from there unsolved. */
  savedId: null as number | null,
  reused: false,
  /** The saved solution holds only the first street and the next card's first decision. */
  partial: false,
  /** The spot the current solution belongs to, and its table. */
  solved: null as SolverSpot | null,
  /** Set with the run it belongs to; spots opened from elsewhere bring their own run. */
  solvedTable: null as (TableInfo & { run: number }) | null,
  error: null as string | null,
  /**
   * A line to walk again once the next solve is done, ending in a size typed in; or the exact
   * `path` of a hand. `notice` says why the page solves meanwhile.
   */
  replay: null as {
    line: Crumb[];
    then: { kind: "bet" | "raise"; amount: number } | null;
    path?: SolverStep[];
    notice?: string;
  } | null,
});

/** How the spot on the page is listed in "Meine Lösungen". */
function solveInfo(): SolveInfo {
  const { oop, ip } = solver.positions;
  const fromLibrary = solver.source.startsWith("Bibliothek");
  const ranges =
    solver.custom.oop || solver.custom.ip
      ? "von dir angepasst"
      : solver.origin || [solver.notes.oop, solver.notes.ip].filter(Boolean).join(" · ") || "Standard-Ranges";
  return {
    title: `${ip} gegen ${oop}`,
    oop,
    ip,
    ranges,
    source: fromLibrary ? "library" : solver.source ? "hand" : "own",
    hand_id: null,
  };
}

/**
 * Solves the spot being edited; the page shows progress and switches to the result. A spot
 * solved before opens from "Meine Lösungen" unless `fresh`, which also reaches deeper streets.
 */
export async function startSolve(fresh = false) {
  solver.error = null;
  solver.progress = null;
  solver.startPct = null;
  solver.summary = null;
  solver.savedId = null;
  solver.reused = false;
  solver.partial = false;
  solver.status = "running";
  const spot = $state.snapshot(solver.spot);
  solver.solved = spot;
  solver.run += 1;
  solver.solvedTable = { ...currentTable(), run: solver.run };
  try {
    await api.solverStart(spot, $state.snapshot(solver.settings), solver.run, solveInfo(), fresh);
  } catch (e) {
    solver.status = "failed";
    solver.error = String(e);
  }
}

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
      solver.savedId = d.saved;
      solver.reused = d.reused;
      solver.partial = d.reused && boardCards(solver.solved?.board ?? "") < 5;
      solver.view = "result";
    } else if (d.status === "cancelled") {
      solver.status = "cancelled";
      solver.error = d.locked ? "Abgebrochen, weil PokerStars gestartet wurde." : null;
    } else {
      solver.status = "failed";
      solver.error = d.error;
    }
  });
  onSolverReleased(() => {
    if (solver.status === "solved") solver.partial = boardCards(solver.solved?.board ?? "") < 5;
  });
}

const boardCards = (b: string) => b.replace(/[^AKQJT2-9cdhs]/gi, "").length / 2;

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
  solver.custom = { oop: false, ip: false };
  solver.origin = "";
  solver.labels = {};
  const players = tableFor(p.players ?? solver.scenario.players, [p.oop.position, p.ip.position]);
  // Roughly the depth the hand started at: what is left plus each player's half of the pot.
  const stack = Math.round(p.stack + p.pot / 2);
  solver.scenario = { ...solver.scenario, players, seats: [p.oop.position, p.ip.position], stack };
  solver.source = p.source;
  solver.hero = p.hero;
  solver.view = "setup";
}

/** The smallest usual table that has both positions, so the seats can be drawn. */
function tableFor(players: number, seats: string[]): number {
  const fits = (n: number) => seats.every((s) => positionsFor(n).includes(s));
  if (players >= 2 && players <= 10 && fits(players)) return players;
  return [6, 8, 9].find(fits) ?? 9;
}

/** The table of the spot being edited. */
export function currentTable(): TableInfo {
  return { players: solver.scenario.players, positions: { ...solver.positions }, labels: { ...solver.labels } };
}

/** The table of the solution on screen; spots opened elsewhere fall back to their positions. */
export function solvedTable(): TableInfo {
  if (solver.solvedTable?.run === solver.run) return solver.solvedTable;
  const seats = [solver.positions.oop, solver.positions.ip];
  const players = tableFor(solver.scenario.players, seats);
  const known = seats.every((s) => positionsFor(players).includes(s));
  return { players, positions: known ? { ...solver.positions } : oopIp(players, ["BB", "BTN"]), labels: {} };
}
