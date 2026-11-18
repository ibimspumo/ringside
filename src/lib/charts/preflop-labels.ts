import type { PreflopAction, PreflopKind, PreflopSpot, PreflopStep } from "./preflop-types";
import { byPos } from "./words";

export function bb(x: number): string {
  return x.toLocaleString("de-DE", { maximumFractionDigits: 2 });
}

/** Fill of an action. Mixed dark enough that light text keeps at least 4.5:1 on every colour. */
export function actionColor(kind: PreflopKind): string {
  switch (kind) {
    case "all_in":
      return "color-mix(in srgb, var(--neg) 70%, var(--surface-2))";
    case "raise":
      return "color-mix(in srgb, var(--amber) 44%, var(--surface-2))";
    case "call":
    case "check":
      return "color-mix(in srgb, var(--pos) 48%, var(--surface-2))";
    case "limp":
      return "color-mix(in srgb, var(--focus) 46%, var(--surface-2))";
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
      return `Call ${bb(a.to_bb)} BB`;
    case "all_in":
      return `All-in ${bb(a.to_bb)} BB`;
    case "raise":
      return `${raiseName(line)} ${bb(a.to_bb)} BB`;
  }
}

/** An action as a verb phrase: "auf 2,2 BB eröffnen", "callen", "3-Bet auf 7 BB". */
export function actionPhrase(a: PreflopAction, line: PreflopStep[] = []): string {
  switch (a.kind) {
    case "fold":
      return "folden";
    case "check":
      return "checken";
    case "limp":
      return "limpen";
    case "call":
      return "callen";
    case "all_in":
      return "all-in";
    case "raise": {
      const name = raiseName(line);
      if (name === "Open") return `auf ${bb(a.to_bb)} BB eröffnen`;
      if (name === "Raise") return `auf ${bb(a.to_bb)} BB raisen`;
      return `${name} auf ${bb(a.to_bb)} BB`;
    }
  }
}

/** Colour word for the answer sentence, matching `actionColor`. */
export function colorWord(kind: PreflopKind): string {
  return kind === "all_in" ? "rote" : kind === "raise" ? "gelbe" : kind === "limp" ? "blaue" : kind === "fold" ? "leere" : "grüne";
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

/** Short tag for a seat on the small table: "Open", "3-Bet", "Call"; the size is in the situation. */
export function stepTag(s: PreflopStep, before: PreflopStep[]): string {
  switch (s.kind) {
    case "raise":
      return raiseName(before);
    case "all_in":
      return "All-in";
    case "call":
      return "Call";
    case "limp":
      return "Limp";
    case "check":
      return "Check";
    default:
      return "Fold";
  }
}

/** The situation in words. */
export function spotLabel(spot: PreflopSpot): string {
  if (!spot.line.length) return "Open – alle vor dir haben gefoldet";
  return spot.line.map((s, i) => stepName(s, spot.line.slice(0, i), spot.seat)).join(" → ");
}

/** What you face, for the answer sentence: "gegen Open vom Button", or null when it is your first action. */
export function facing(spot: PreflopSpot): string | null {
  if (!spot.line.length) return null;
  const last = spot.line[spot.line.length - 1];
  const before = spot.line.slice(0, -1);
  const what = last.kind === "raise" ? raiseName(before) : last.kind === "all_in" ? "All-in" : last.kind === "limp" ? "Limp" : "Call";
  return `gegen ${what} ${last.seat === spot.seat ? "von dir" : byPos(last.position)}`;
}
