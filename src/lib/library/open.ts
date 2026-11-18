import { keepOwnHand, solver } from "../solver/store.svelte";
import { libraryApi } from "./api";
import { labelOf } from "./labels";
import type { LibraryScenario } from "./types";

const num = (x: number) => x.toLocaleString("de-DE", { maximumFractionDigits: 2 });

/** Loads a stored spot and shows it on the solver page as a finished solution. */
export async function openInSolver(s: LibraryScenario, flop: string): Promise<void> {
  const o = await libraryApi.open(s.id, flop);
  keepOwnHand(o.spot.board);
  solver.run += 1;
  solver.spot = o.spot;
  solver.solved = o.spot;
  solver.summary = o.summary;
  solver.status = "solved";
  solver.error = null;
  solver.progress = null;
  solver.positions = { oop: o.oop, ip: o.ip };
  solver.notes = { oop: o.oop_note, ip: o.ip_note };
  solver.custom = { oop: false, ip: false };
  solver.origin = `aus dem Preflop-Modell, 9 Spieler, ${num(s.stack_bb)} BB`;
  solver.source = `Bibliothek: ${labelOf(s)}`;
  solver.replay = null;
  solver.labels = { [o.oop]: shortNote(o.oop_note), [o.ip]: shortNote(o.ip_note) };
  solver.solvedTable = { players: 9, positions: { oop: o.oop, ip: o.ip }, labels: solver.labels, run: solver.run };
  solver.view = "result";
}

/** "Open-Raise aus BTN auf 2,2 BB" → "Open 2,2", "3-Bet aus BB auf 8,25 BB" → "3-Bet 8,25". */
function shortNote(note: string): string {
  const to = note.match(/auf ([\d,]+) BB/)?.[1];
  if (note.startsWith("3-Bet")) return to ? `3-Bet ${to}` : "3-Bet";
  if (note.includes("gegen die 3-Bet")) return "Call 3-Bet";
  if (note.startsWith("Open")) return to ? `Open ${to}` : "Open";
  return "Call";
}
