import type { PreflopAction, PreflopKind, PreflopSpot, PreflopStep } from "./preflop-types";

export function bb(x: number): string {
  return x.toLocaleString("de-DE", { maximumFractionDigits: 2 });
}

export function actionColor(kind: PreflopKind): string {
  switch (kind) {
    case "all_in":
      return "color-mix(in srgb, var(--neg) 82%, var(--surface-2))";
    case "raise":
      return "color-mix(in srgb, var(--amber) 80%, var(--surface-2))";
    case "call":
    case "check":
      return "color-mix(in srgb, var(--pos) 70%, var(--surface-2))";
    case "limp":
      return "color-mix(in srgb, var(--focus) 72%, var(--surface-2))";
    default:
      return "var(--surface-2)";
  }
}

/** Raise names by the raises before: open (raise over a limp), 3-bet, 4-bet … */
function raiseName(before: PreflopStep[]): string {
  const raises = raisesIn(before);
  if (raises === 0) return before.some((s) => s.kind === "limp") ? "Raise" : "Open";
  return `${raises + 2}-Bet`;
}

function raisesIn(line: PreflopStep[]): number {
  return line.filter((s) => s.kind === "raise" || s.kind === "all_in").length;
}

/** Name of an action at a spot, with its size. */
export function actionName(a: PreflopAction, line: PreflopStep[] = []): string {
  switch (a.kind) {
    case "fold":
      return "Fold";
    case "check":
      return "Check";
    case "limp":
      return "Limp";
    case "call":
      return `Call ${bb(a.to_bb)}`;
    case "all_in":
      return `All-in ${bb(a.to_bb)}`;
    case "raise":
      return `${raiseName(line)} ${bb(a.to_bb)}`;
  }
}

/** One step of a line, from the point of view of `seat` ("du" for its own actions). */
function stepName(s: PreflopStep, before: PreflopStep[], seat: number): string {
  const who = s.seat === seat ? "du" : s.position;
  switch (s.kind) {
    case "raise":
      return `${who} ${raiseName(before)} ${bb(s.to_bb)}`;
    case "all_in":
      return `${who} All-in`;
    case "call":
      return `${who} Call`;
    case "limp":
      return `${who} Limp`;
    case "check":
      return `${who} Check`;
    default:
      return who;
  }
}

/** The situation in words. */
export function spotLabel(spot: PreflopSpot): string {
  if (!spot.line.length) return "Open – alle vor dir haben gefoldet";
  return spot.line.map((s, i) => stepName(s, spot.line.slice(0, i), spot.seat)).join(" → ");
}

/** Short title for the chart header. */
export function spotTitle(spot: PreflopSpot): string {
  if (!spot.line.length) return `${spot.position} · Open`;
  const last = spot.line[spot.line.length - 1];
  const before = spot.line.slice(0, -1);
  const what =
    last.kind === "raise"
      ? raiseName(before)
      : last.kind === "all_in"
        ? "All-in"
        : last.kind === "limp"
          ? "Limp"
          : last.kind === "call"
            ? "Call"
            : "Check";
  return `${spot.position} · gegen ${what} von ${last.seat === spot.seat ? "dir" : last.position}`;
}

export type SpotGroup = "open" | "vs_open" | "vs_3bet" | "later";

export const GROUP_LABEL: Record<SpotGroup, string> = {
  open: "Erste Aktion",
  vs_open: "Gegen Open oder Limp",
  vs_3bet: "Gegen 3-Bet oder All-in",
  later: "Gegen 4-Bet und später",
};

export function spotGroup(spot: PreflopSpot): SpotGroup {
  if (!spot.line.length) return "open";
  const raises = raisesIn(spot.line);
  if (raises <= 1 && !spot.line.some((s) => s.kind === "all_in")) return "vs_open";
  if (raises <= 2) return "vs_3bet";
  return "later";
}
