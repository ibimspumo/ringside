/** The playing profile (crates/ringside-core/src/profile.rs) and practice from it (src-tauri/src/drill.rs). */
import type { Decision, DecisionBrief, Source } from "../analysis/types";
import type { Hand } from "../types";

export type Lage =
  | "first_in"
  | "push_fold"
  | "vs_limp"
  | "vs_open"
  | "vs_three_bet"
  | "vs_all_in"
  | "c_bet"
  | "vs_c_bet"
  | "flop_bet"
  | "flop_vs_bet"
  | "turn_bet"
  | "turn_vs_bet"
  | "river_bet"
  | "river_vs_bet";

export interface SpotChange {
  before: number;
  before_error_pct: number;
  after: number;
  after_error_pct: number;
}

/** One kind of spot: situation, seat (preflop only), stack band and table size. */
export interface SpotGroup {
  /** "vs_open|BB|1|9". */
  key: string;
  lage: Lage;
  seat: "EP" | "MP" | "CO" | "BTN" | "SB" | "BB" | null;
  band: number | null;
  table: "heads_up" | "short" | "full";
  /** "Gegen Open im Big Blind". */
  title: string;
  /** "10–20 BB · 7–10 Spieler". */
  detail: string;
  /** Rated decisions. */
  decisions: number;
  flawed: number;
  error_pct: number;
  loss_bb: number;
  mean_loss_bb: number;
  open: number;
  sources: [Source, number][];
  /** Share of rated decisions from equilibrium, solver or model, percent. */
  computed_pct: number;
  /** Share of the loss from the estimate, percent. */
  estimate_loss_pct: number;
  /** How often the estimate agreed with the solver (0–1), weighted by loss. */
  estimate_reliability: number | null;
  /** Most of the loss rests on the estimate. */
  unsure: boolean;
  /** Too few decisions to rank. */
  few: boolean;
  rank: number | null;
  last_played_at: string;
  change: SpotChange | null;
}

export interface PlayProfile {
  decisions: number;
  loss_bb: number;
  open: number;
  min_decisions: number;
  /** Ranked groups first, costliest first. */
  groups: SpotGroup[];
}

export interface DrillChoice {
  label: string;
  kind: string;
}

/** A decision to make again: from a real hand (`seed` null) or a variation of one. */
export interface DrillSpot {
  key: string;
  title: string;
  detail: string;
  hand_id: number;
  action_index: number;
  seed: number | null;
  played_at: string;
  /** The hand up to the decision; a variation with the hero's new cards. */
  hand: Hand;
  hero_cards: string[];
  /** The hero's seat, e.g. "BB". */
  position: string | null;
  choices: DrillChoice[];
  source: Source | null;
  reliability: number | null;
  pot_bb: number | null;
}

export interface DrillResult {
  decision: Decision;
  score: number;
  correct: boolean;
  /** What was played back then, for a real hand. */
  then: DecisionBrief | null;
}

/** "17.11." from "2026/11/17 21:03:11". */
export function dayOf(playedAt: string): string {
  const [y, m, d] = playedAt.split(" ")[0].split("/");
  return y && m && d ? `${Number(d)}.${Number(m)}.` : playedAt;
}

const nf = (d: number) => new Intl.NumberFormat("de-DE", { minimumFractionDigits: d, maximumFractionDigits: d });

/** "0,4 BB", one decimal from 0,1 on, two below. */
export function bbText(x: number): string {
  const v = Math.abs(x);
  return `${nf(v >= 0.095 || v === 0 ? 1 : 2).format(v)} BB`;
}

/** "38-mal gespielt, kostet dich etwa 0,4 BB pro Mal". */
export function costLine(g: SpotGroup): string {
  const times = g.decisions === 1 ? "einmal gespielt" : `${g.decisions}-mal gespielt`;
  if (g.mean_loss_bb < 0.005) return `${times}, kostet dich so gut wie nichts`;
  return `${times}, kostet dich etwa ${bbText(g.mean_loss_bb)} pro Mal`;
}

/** The honest note for a group rated mostly by the estimate, or null. */
export function unsureLine(g: SpotGroup): string | null {
  if (!g.unsure) return null;
  const r = g.estimate_reliability;
  const tenths = r === null ? null : Math.round(r * 10);
  return tenths === null
    ? "Der Verlust stammt überwiegend aus Einschätzungen, nicht aus einer Lösung."
    : `Der Verlust stammt überwiegend aus Einschätzungen. Die liegen in etwa ${tenths} von 10 Prüf-Spots nah am Solver.`;
}

/** "seit dem Üben: Fehlerquote 34 % → 18 %", or null below enough hands. */
export function changeLine(g: SpotGroup): string | null {
  const c = g.change;
  if (!c) return null;
  return `seit dem Üben: Fehlerquote ${Math.round(c.before_error_pct)} % → ${Math.round(c.after_error_pct)} %`;
}

/** The kinds of spot worth practising: ranked, costliest first. */
export const weakGroups = (p: PlayProfile | null) => (p?.groups ?? []).filter((g) => g.rank !== null);
