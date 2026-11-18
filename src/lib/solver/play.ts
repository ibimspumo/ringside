// Walking a solved tree: action labels, bet sizes in % of the pot, the nearest size the
// tree has for one typed in, and replaying a line in a freshly solved tree.
import { bb } from "./actions";
import type { SolverAction, SolverNode, SolverPlayer, SolverSpot, SolverStep } from "./types";

export const STREET_NAME = { flop: "Flop", turn: "Turn", river: "River" } as const;
export const STREET_INDEX = { flop: 0, turn: 1, river: 2 } as const;

export interface Crumb {
  step: SolverStep;
  /** Who acted, or null for a dealt card. */
  player: SolverPlayer | null;
  label: string;
  street: SolverNode["street"];
  /** Kind and amount, to find the same action in another tree. */
  kind?: SolverAction["kind"];
  amount?: number;
}

const side = (p: SolverPlayer) => (p === "oop" ? 0 : 1);

export function toCall(node: SolverNode): number {
  if (!node.player) return 0;
  const me = side(node.player);
  return Math.max(0, node.street_bets[1 - me] - node.street_bets[me]);
}

/**
 * The size in the solver's terms: bets in % of the pot, raises in % of the pot after calling,
 * both counting what is already in front of the players.
 */
export function sizePct(a: Pick<SolverAction, "kind" | "amount">, node: SolverNode): number {
  if (!node.player || a.amount <= 0) return 0;
  const facing = node.street_bets[1 - side(node.player)];
  const call = toCall(node);
  if (a.kind === "bet" || (a.kind === "all_in" && facing === 0)) return (a.amount / node.pot) * 100;
  return ((a.amount - facing) / (node.pot + call)) * 100;
}

export function isAggressive(kind: SolverAction["kind"]): boolean {
  return kind === "bet" || kind === "raise" || kind === "all_in";
}

/** Main label and the chips it costs, e.g. "Bet 33 %" and "1,8 BB". */
export function actionParts(a: SolverAction, node: SolverNode): { label: string; chips: string } {
  const pct = Math.round(sizePct(a, node));
  switch (a.kind) {
    case "fold":
      return { label: "Fold", chips: "" };
    case "check":
      return { label: "Check", chips: "" };
    case "call":
      return { label: "Call", chips: `${bb(toCall(node), 2)} BB` };
    case "bet":
      return { label: `Bet ${pct} % Pot`, chips: `${bb(a.amount, 2)} BB` };
    case "raise":
      return { label: `Raise ${pct} % Pot`, chips: `auf ${bb(a.amount, 2)} BB` };
    case "all_in":
      return { label: "All-in", chips: `${bb(a.amount, 2)} BB` };
  }
}

export function actionText(a: SolverAction, node: SolverNode): string {
  const p = actionParts(a, node);
  return p.chips ? `${p.label} (${p.chips})` : p.label;
}

/** Amount to bet or raise to on this street for `value` typed in BB or % of the pot. */
export function targetAmount(node: SolverNode, value: number, unit: "bb" | "pct"): number {
  if (!node.player) return 0;
  const facing = node.street_bets[1 - side(node.player)];
  if (unit === "bb") return value;
  if (facing === 0) return (value / 100) * node.pot;
  return facing + (value / 100) * (node.pot + toCall(node));
}

export interface SizeMatch {
  /** Index of the closest bet, raise or all-in in the tree. */
  index: number;
  /** The amount asked for, and how far the tree's size is off, in BB. */
  wanted: number;
  off: number;
  /** Within rounding of a size the tree has. */
  exact: boolean;
  /** The wanted size as a solver size in percent, for solving it again. */
  pct: number;
  kind: "bet" | "raise";
  /** More than the stack allows. */
  tooBig: boolean;
}

/** The tree's closest aggressive action to betting or raising to `wanted` BB. */
export function nearestSize(node: SolverNode, wanted: number): SizeMatch | null {
  if (!node.player) return null;
  const me = side(node.player);
  const facing = node.street_bets[1 - me];
  const max = node.street_bets[me] + node.stacks[me];
  let index = -1;
  node.actions.forEach((a, k) => {
    if (!isAggressive(a.kind)) return;
    if (index < 0 || Math.abs(a.amount - wanted) < Math.abs(node.actions[index].amount - wanted)) index = k;
  });
  if (index < 0) return null;
  const off = node.actions[index].amount - wanted;
  const kind = facing > 0 ? "raise" : "bet";
  const pct = sizePct({ kind, amount: Math.min(wanted, max) }, node);
  // The solver rounds sizes to a hundredth of a BB and merges sizes within 10 % of each other.
  const exact = Math.abs(off) <= Math.max(0.02, wanted * 0.03);
  return { index, wanted, off, exact, pct, kind, tooBig: wanted > max + 0.005 };
}

/** `spot` with `pct` added to the sizes of `street`; a full list gives up its closest size. */
export function withSize(spot: SolverSpot, street: number, kind: "bet" | "raise", pct: number, max: number): SolverSpot {
  const sizes = spot.sizes.map((s) => ({ bet: [...s.bet], raise: [...s.raise] })) as SolverSpot["sizes"];
  const list = sizes[street][kind];
  const value = Math.round(pct);
  if (!list.includes(value)) {
    if (list.length >= max) {
      let worst = 0;
      list.forEach((v, i) => Math.abs(v - value) < Math.abs(list[worst] - value) && (worst = i));
      list.splice(worst, 1);
    }
    list.push(value);
    list.sort((a, b) => a - b);
  }
  return { ...spot, sizes };
}

/** The step in `node` that matches `crumb`: same card, or the same kind of action closest in size. */
export function matchStep(node: SolverNode, crumb: Crumb): SolverStep | null {
  if ("card" in crumb.step) return node.kind === "chance" && node.cards.includes(crumb.step.card) ? crumb.step : null;
  if (node.kind !== "action") return null;
  const same = (k: SolverAction["kind"]) => k === crumb.kind || (isAggressive(k) && isAggressive(crumb.kind ?? "check"));
  let best = -1;
  node.actions.forEach((a, k) => {
    if (!same(a.kind)) return;
    if (best < 0 || Math.abs(a.amount - (crumb.amount ?? 0)) < Math.abs(node.actions[best].amount - (crumb.amount ?? 0))) best = k;
  });
  return best < 0 ? null : { action: best };
}
