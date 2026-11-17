import { call } from "../api";
import { demoShow } from "./demo";
import { defaultSizes } from "./presets";
import { solver } from "./store.svelte";
import type { SolverSpot, SolverSummary } from "./types";

/** A solution saved on this computer, see `src-tauri/src/solutions.rs`. */
export interface SavedSolution {
  id: number;
  spot_key: string;
  title: string;
  board: string;
  oop: string;
  ip: string;
  pot_bb: number;
  stack_bb: number;
  ranges: string;
  sizes: string;
  exploitability_pct: number;
  target_pct: number;
  /** Every node is stored; otherwise deeper streets are solved again on demand. */
  complete: boolean;
  source: "own" | "hand" | "library" | "training";
  hand_id: number | null;
  file: string;
  bytes: number;
  created_at: number;
  pinned: boolean;
}

export interface SolutionsView {
  items: SavedSolution[];
  bytes: number;
  max_mb: number;
}

interface Opened {
  id: number;
  spot: SolverSpot;
  summary: SolverSummary;
  info: SavedSolution;
}

// Browser preview: a few saved spots so the list can be designed without the backend.
const DAY = 86400;
const now = Math.floor(Date.now() / 1000);
let demo: SavedSolution[] = [
  ["BTN gegen BB", "AhKd7c", "BB", "BTN", 5.5, 37.5, 0.42, "own", 1_080_000, now - 600, true, false],
  ["CO gegen BB", "9s8s3d", "BB", "CO", 5.5, 47.5, 0.5, "hand", 940_000, now - 3 * 3600, false, false],
  ["BTN gegen SB, 3-Bet-Pot", "QhJd4c2s", "SB", "BTN", 18.5, 31.5, 0.31, "own", 612_000, now - DAY, false, true],
  ["BTN gegen BB", "Ks7d2c4h9s", "BB", "BTN", 12, 25, 0.12, "hand", 23_000, now - 3 * DAY, true, false],
  ["HJ gegen BB", "Tc9c6h", "BB", "HJ", 5.5, 37.5, 0.48, "library", 1_020_000, now - 9 * DAY, false, false],
].map(([title, board, oop, ip, pot, stack, expl, source, bytes, at, complete, pinned], i) => ({
  id: i + 1,
  spot_key: `demo${i}`,
  title: title as string,
  board: board as string,
  oop: oop as string,
  ip: ip as string,
  pot_bb: pot as number,
  stack_bb: stack as number,
  ranges: source === "hand" ? "aus dem Preflop-Modell, 8-max, 50 BB" : "aus dem Preflop-Modell, 9 Spieler, 40 BB",
  sizes: (board as string).length >= 10 ? "River 75 %, Raise 60 %" : "Flop 33/75 %, Raise 60 % · Turn 66 % · River 75 %",
  exploitability_pct: expl as number,
  target_pct: 0.5,
  complete: complete as boolean,
  source: source as SavedSolution["source"],
  hand_id: null,
  file: "",
  bytes: bytes as number,
  created_at: at as number,
  pinned: pinned as boolean,
}));
let demoMax = 500;

function demoView(): SolutionsView {
  return { items: structuredClone(demo), bytes: demo.reduce((s, x) => s + x.bytes, 0), max_mb: demoMax };
}

function demoOpen(id: number): Opened {
  const info = demo.find((s) => s.id === id) ?? demo[0];
  const spot: SolverSpot = {
    oop_range: "22-QQ,A2s-AKs,K9s-KQs,QTs-QJs,JTs,T9s,98s,87s,76s,65s,54s,ATo-AQo,KJo-KQo",
    ip_range: "22-JJ,A2s-AQs,K5s-KQs,Q8s-QJs,J8s-JTs,T8s-T9s,97s-98s,86s-87s,75s-76s,65s,54s,A8o-AQo,KTo-KQo,QJo",
    board: info.board,
    pot: info.pot_bb,
    stack: info.stack_bb,
    sizes: defaultSizes(),
    allin_threshold: 67,
  };
  demoShow(spot);
  const summary: SolverSummary = {
    iterations: 220,
    exploitability: (info.exploitability_pct / 100) * info.pot_bb,
    exploitability_pct: info.exploitability_pct,
    converged: true,
    elapsed_ms: 0,
    memory_bytes: info.bytes,
    compressed: false,
    threads: 0,
  };
  return { id, spot, summary, info };
}

export const solutionsApi = {
  list: () => call<SolutionsView>("solutions_list", {}, demoView),
  open: (id: number) => call<Opened>("solutions_open", { id }, () => demoOpen(id)),
  pin: (id: number, pinned: boolean) =>
    call<void>("solutions_pin", { id, pinned }, () => {
      demo = demo.map((s) => (s.id === id ? { ...s, pinned } : s));
    }),
  delete: (id: number) =>
    call<void>("solutions_delete", { id }, () => {
      demo = demo.filter((s) => s.id !== id);
    }),
  /** Returns how many solutions were deleted to fit. */
  setLimit: (maxMb: number) =>
    call<number>("solutions_set_limit", { maxMb }, () => {
      demoMax = maxMb;
      return 0;
    }),
};

/** Shows a saved solution on the solver page at once, without solving. */
export async function openSaved(id: number): Promise<void> {
  const o = await solutionsApi.open(id);
  solver.run += 1;
  solver.spot = o.spot;
  solver.solved = o.spot;
  solver.summary = o.summary;
  solver.status = "solved";
  solver.error = null;
  solver.progress = null;
  solver.savedId = o.id;
  solver.reused = true;
  solver.partial = !o.info.complete;
  solver.positions = { oop: o.info.oop || "BB", ip: o.info.ip || "BTN" };
  solver.notes = { oop: "", ip: "" };
  solver.custom = { oop: false, ip: false };
  solver.origin = o.info.ranges;
  solver.source = "";
  solver.hero = null;
  solver.replay = null;
  solver.labels = {};
  solver.solvedTable = null;
  solver.view = "result";
}
