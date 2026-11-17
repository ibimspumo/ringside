// Practice against the solution: dealing from the ranges, the opponent's random choice by
// frequency, and grading a decision by what it costs against the best action.
import { RANKS, comboClass, parseRange } from "./range";
import type { SolverHand, SolverNode } from "./types";

const SUITS = "shdc";
export const DECK = RANKS.split("").flatMap((r) => SUITS.split("").map((s) => r + s));

export function sameCombo(combo: string, cards: string[]): boolean {
  const a = combo.slice(0, 2);
  const b = combo.slice(2, 4);
  return (a === cards[0] && b === cards[1]) || (a === cards[1] && b === cards[0]);
}

export function handOf(node: SolverNode, cards: string[]): SolverHand | undefined {
  return node.hands.find((h) => sameCombo(h.combo, cards));
}

/** Picks an index with probability proportional to `weights`. */
export function pick(weights: number[], random = Math.random): number {
  const total = weights.reduce((a, b) => a + Math.max(0, b), 0);
  let r = random() * total;
  for (let i = 0; i < weights.length; i++) {
    r -= Math.max(0, weights[i]);
    if (r < 0) return i;
  }
  return weights.findLastIndex((w) => w > 0);
}

/** One combo from `range`, each weighted by its class weight, avoiding `dead` cards. */
export function dealFrom(range: string, dead: string[], random = Math.random): string[] | null {
  const weights = parseRange(range);
  if (!weights) return null;
  const blocked = new Set(dead);
  const combos: string[][] = [];
  const w: number[] = [];
  for (let i = 0; i < DECK.length; i++) {
    for (let j = i + 1; j < DECK.length; j++) {
      const [a, b] = [DECK[i], DECK[j]];
      if (blocked.has(a) || blocked.has(b)) continue;
      const x = weights[comboClass(a, b)];
      if (x > 0) {
        combos.push([a, b]);
        w.push(x);
      }
    }
  }
  return combos.length ? combos[pick(w, random)] : null;
}

export type Grade = "best" | "good" | "inaccurate" | "mistake";

/**
 * Thresholds in percent of the pot at the decision, so they mean the same in small and big
 * pots. 0.5 % is the solver's default accuracy target: below it, two actions are equally good
 * within what the solve can tell apart.
 */
export const THRESHOLDS = { best: 0.5, good: 2, inaccurate: 6 } as const;
/** "Best" also asks that the solution really plays the action with this hand. */
export const BEST_MIN_FREQUENCY = 0.1;

export const GRADES: Record<Grade, { label: string; points: number }> = {
  best: { label: "Beste Wahl", points: 100 },
  good: { label: "Gut", points: 70 },
  inaccurate: { label: "Ungenau", points: 30 },
  mistake: { label: "Fehler", points: 0 },
};

export interface Verdict {
  grade: Grade;
  /** How often the solution takes the chosen action with this hand, 0–1. */
  frequency: number;
  /** EV given up against the best action, in BB and in % of the pot; null without EVs. */
  loss: number | null;
  lossPct: number | null;
  best: number;
}

export function judge(hand: SolverHand, chosen: number, pot: number): Verdict {
  const frequency = hand.strategy[chosen] ?? 0;
  if (hand.evs.length !== hand.strategy.length) {
    // Without EVs per action only the frequencies tell how good a choice was.
    const best = hand.strategy.indexOf(Math.max(...hand.strategy));
    const grade: Grade = frequency >= 0.3 || chosen === best ? "best" : frequency >= 0.1 ? "good" : frequency >= 0.02 ? "inaccurate" : "mistake";
    return { grade, frequency, loss: null, lossPct: null, best };
  }
  const top = Math.max(...hand.evs);
  const best = hand.evs.indexOf(top);
  const loss = Math.max(0, top - hand.evs[chosen]);
  const lossPct = (loss / pot) * 100;
  const grade: Grade =
    lossPct <= THRESHOLDS.best && frequency >= BEST_MIN_FREQUENCY
      ? "best"
      : lossPct <= THRESHOLDS.good
        ? "good"
        : lossPct <= THRESHOLDS.inaccurate
          ? "inaccurate"
          : "mistake";
  return { grade, frequency, loss, lossPct, best };
}

// Showdown: the best five of seven cards, as comparable tuples.

const CATEGORY = ["Höchste Karte", "Paar", "Zwei Paare", "Drilling", "Straße", "Flush", "Full House", "Vierling", "Straight Flush"];

function straightTop(ranks: Set<number>): number {
  // Ranks are 12 (ace) down to 0 (deuce); the wheel counts the ace as low.
  for (let top = 12; top >= 3; top--) {
    const need = top === 3 ? [3, 2, 1, 0, 12] : [top, top - 1, top - 2, top - 3, top - 4];
    if (need.every((r) => ranks.has(r))) return top;
  }
  return -1;
}

/** [category, tie-breakers…]; higher compares better. */
export function rank7(cards: string[]): number[] {
  const values = cards.map((c) => 12 - RANKS.indexOf(c[0]));
  const bySuit = new Map<string, number[]>();
  cards.forEach((c, i) => bySuit.set(c[1], [...(bySuit.get(c[1]) ?? []), values[i]]));
  const flush = [...bySuit.values()].find((v) => v.length >= 5);
  if (flush) {
    const sf = straightTop(new Set(flush));
    if (sf >= 0) return [8, sf];
  }
  const count = new Map<number, number>();
  values.forEach((v) => count.set(v, (count.get(v) ?? 0) + 1));
  const groups = [...count.entries()].sort((a, b) => b[1] - a[1] || b[0] - a[0]);
  const kickers = (skip: number[], n: number) =>
    [...new Set(values)]
      .filter((v) => !skip.includes(v))
      .sort((a, b) => b - a)
      .slice(0, n);
  const [g0, g1] = groups;
  if (g0[1] === 4) return [7, g0[0], ...kickers([g0[0]], 1)];
  if (g0[1] === 3 && g1 && g1[1] >= 2) return [6, g0[0], g1[0]];
  if (flush) return [5, ...flush.sort((a, b) => b - a).slice(0, 5)];
  const st = straightTop(new Set(values));
  if (st >= 0) return [4, st];
  if (g0[1] === 3) return [3, g0[0], ...kickers([g0[0]], 2)];
  if (g0[1] === 2 && g1 && g1[1] === 2) return [2, g0[0], g1[0], ...kickers([g0[0], g1[0]], 1)];
  if (g0[1] === 2) return [1, g0[0], ...kickers([g0[0]], 3)];
  return [0, ...kickers([], 5)];
}

export function compare(a: number[], b: number[]): number {
  for (let i = 0; i < Math.max(a.length, b.length); i++) {
    const d = (a[i] ?? -1) - (b[i] ?? -1);
    if (d) return d;
  }
  return 0;
}

export function categoryName(rank: number[]): string {
  return CATEGORY[rank[0]];
}
