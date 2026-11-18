// Which decisions the page shows: the situation tiles, their counts and good defaults,
// all worked out from the grouped counts without asking the backend again.
import type { History, RangeFilter, RangeGroup } from "./types";
import { NO_RANGE } from "./types";

export type Band = 0 | 1 | 2 | 3;
export type Depth = { kind: "band"; band: Band } | { kind: "free"; min: number; max: number } | { kind: "all" };
export type Size = "all" | "full" | "mid" | "short" | "hu";
export type Behind = "all" | 0 | 1 | 2;

export interface Selection {
  history: History;
  position: string | null;
  /** The raiser or first limper before you; null: anyone. */
  vs: string | null;
  depth: Depth;
  size: Size;
  behind: Behind;
}

/** Stack bands like the leak finder, in BB: [from, below). */
export const BANDS: [number, number | null][] = [
  [0, 10],
  [10, 20],
  [20, 40],
  [40, null],
];
/** The overview counts stacks in whole BB up to this; deeper ones share it. */
export const STACK_CAP = 60;

export const SIZES: Record<Size, [number, number] | null> = {
  all: null,
  full: [7, 10],
  mid: [5, 6],
  short: [3, 4],
  hu: [2, 2],
};

export const BEHIND_BB: [number | null, number | null][] = [
  [null, 10],
  [10, 25],
  [25, null],
];

/** Preflop order, the big blind last. */
export const POSITIONS = ["UTG", "UTG+1", "UTG+2", "UTG+3", "UTG+4", "LJ", "HJ", "CO", "BTN", "SB", "BB"];

export const HISTORIES: History[] = ["unopened", "limped", "vs_open", "squeeze", "vs_three_bet", "vs_shove"];

/** Situations with someone before you who limped or raised. */
export const hasOpponent = (h: History) => h !== "unopened";

type Facet = "history" | "position" | "vs" | "depth" | "size" | "behind";

function depthRange(d: Depth): [number, number | null] | null {
  if (d.kind === "band") return BANDS[d.band];
  if (d.kind === "free") return [d.min, d.max >= STACK_CAP ? null : d.max];
  return null;
}

function matches(g: RangeGroup, s: Selection, skip: Facet | null): boolean {
  if (skip !== "history" && g.history !== s.history) return false;
  if (skip !== "position" && s.position !== null && g.position !== s.position) return false;
  if (skip !== "vs" && s.vs !== null && g.vs_position !== s.vs) return false;
  if (skip !== "depth") {
    const r = depthRange(s.depth);
    if (r && (g.stack < r[0] || (r[1] !== null && g.stack >= r[1]))) return false;
  }
  if (skip !== "size") {
    const r = SIZES[s.size];
    if (r && (g.players < r[0] || g.players > r[1])) return false;
  }
  if (skip !== "behind" && s.behind !== "all" && g.behind !== s.behind) return false;
  return true;
}

/** Decisions per value of one facet, with every other choice applied. */
export function facet<K>(groups: RangeGroup[], s: Selection, f: Facet, key: (g: RangeGroup) => K): Map<K, number> {
  const out = new Map<K, number>();
  for (const g of groups) {
    if (!matches(g, s, f)) continue;
    const k = key(g);
    out.set(k, (out.get(k) ?? 0) + g.count);
  }
  return out;
}

export function total(groups: RangeGroup[], s: Selection): number {
  return groups.reduce((n, g) => n + (matches(g, s, null) ? g.count : 0), 0);
}

export const bandOf = (stack: number): Band => (stack < 10 ? 0 : stack < 20 ? 1 : stack < 40 ? 2 : 3);

function most<K>(m: Map<K, number>): K | null {
  let best: K | null = null;
  let n = 0;
  for (const [k, c] of m) {
    if (c > n) {
      best = k;
      n = c;
    }
  }
  return best;
}

/**
 * Fills in what a new history needs: the position you hold most often there (unless the
 * current one still has hands), its most common depth and the most common opponent.
 */
export function settle(groups: RangeGroup[], s: Selection, keepDepth = false): Selection {
  const next: Selection = { ...s };
  const positions = facet(groups, { ...next, vs: null }, "position", (g) => g.position);
  if (!next.position || !positions.get(next.position)) next.position = most(positions);
  if (!keepDepth) {
    const bands = facet(groups, { ...next, vs: null }, "depth", (g) => bandOf(g.stack));
    const band = most(bands);
    next.depth = band === null ? { kind: "all" } : { kind: "band", band };
  }
  next.vs = hasOpponent(next.history) ? most(facet(groups, next, "vs", (g) => g.vs_position)) : null;
  return next;
}

/** The starting view: first in with everyone folded where you have hands, else the commonest history. */
export function initial(groups: RangeGroup[]): Selection {
  const blank: Selection = { history: "unopened", position: null, vs: null, depth: { kind: "all" }, size: "all", behind: "all" };
  const histories = facet(groups, blank, "history", (g) => g.history);
  const history = histories.get("unopened") ? "unopened" : (most(histories) ?? "unopened");
  return settle(groups, { ...blank, history });
}

export function toFilter(s: Selection): RangeFilter {
  const d = depthRange(s.depth);
  const size = SIZES[s.size];
  const behind = s.behind === "all" ? null : BEHIND_BB[s.behind];
  return {
    ...NO_RANGE,
    history: s.history,
    position: s.position,
    vs_position: hasOpponent(s.history) ? s.vs : null,
    min_bb: d ? d[0] || null : null,
    max_bb: d ? d[1] : null,
    min_players: size ? size[0] : null,
    max_players: size ? size[1] : null,
    min_behind_bb: behind ? behind[0] : null,
    max_behind_bb: behind ? behind[1] : null,
  };
}

/** Decisions per whole BB of stack (0 … 60+), with everything but the depth applied. */
export function stackHistogram(groups: RangeGroup[], s: Selection): number[] {
  const bins = new Array(STACK_CAP + 1).fill(0);
  for (const g of groups) if (matches(g, s, "depth")) bins[Math.min(g.stack, STACK_CAP)] += g.count;
  return bins;
}
