import type { SolverAction } from "./types";

export function bb(x: number, digits = 2): string {
  return x.toLocaleString("de-DE", { maximumFractionDigits: digits });
}

/** E.g. "Bet 1,8 (33 %)", "Raise auf 6,2", "All-in 37,5". `pot` is the pot at the node. */
export function actionLabel(a: SolverAction, pot: number): string {
  switch (a.kind) {
    case "fold":
      return "Fold";
    case "check":
      return "Check";
    case "call":
      return "Call";
    case "bet":
      return `Bet ${bb(a.amount, 1)} (${Math.round((a.amount / pot) * 100)} %)`;
    case "raise":
      return `Raise auf ${bb(a.amount, 1)}`;
    case "all_in":
      return `All-in ${bb(a.amount, 1)}`;
  }
}

const PASSIVE: Record<string, string> = { fold: "#4f8fca", check: "#5fae45", call: "#5fae45" };
/** Light to dark: small bets amber, big bets and all-ins deep red. */
const AGGRESSIVE = ["#f2b12e", "#ec8a2f", "#e5603a", "#d23f32", "#a92c25"];

export function actionColors(actions: SolverAction[]): string[] {
  const aggressive = actions.filter((a) => !(a.kind in PASSIVE)).length;
  let k = 0;
  return actions.map((a) => {
    if (a.kind in PASSIVE) return PASSIVE[a.kind];
    const step = aggressive <= 1 ? 2 : Math.round((k++ * (AGGRESSIVE.length - 1)) / (aggressive - 1));
    return a.kind === "all_in" ? AGGRESSIVE[AGGRESSIVE.length - 1] : AGGRESSIVE[Math.min(step, AGGRESSIVE.length - 1)];
  });
}

export function bytes(n: number): string {
  if (n >= 2 ** 30) return `${(n / 2 ** 30).toLocaleString("de-DE", { maximumFractionDigits: 1 })} GB`;
  return `${Math.max(1, Math.round(n / 2 ** 20)).toLocaleString("de-DE")} MB`;
}

export function duration(seconds: number): string {
  if (seconds < 1) return "unter 1 s";
  if (seconds < 90) return `${Math.round(seconds)} s`;
  if (seconds < 3600) return `${Math.round(seconds / 60)} min`;
  return `${(seconds / 3600).toLocaleString("de-DE", { maximumFractionDigits: 1 })} h`;
}
