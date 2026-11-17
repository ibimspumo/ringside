import type { LibraryScenario } from "./types";

/** Opener first, as in the scenario id ("srp-btn-bb-40", "3bp-co-btn-75"). */
function seats(s: LibraryScenario): [string, string] {
  const [, a, b] = s.id.split("-");
  return [(a ?? s.ip).toUpperCase(), (b ?? s.oop).toUpperCase()];
}

const NAMES: Record<string, { name: string; at: string; against: string }> = {
  BTN: { name: "Button", at: "am Button", against: "den Button" },
  CO: { name: "Cutoff", at: "im Cutoff", against: "den Cutoff" },
  UTG: { name: "frühe Position", at: "in früher Position", against: "die frühe Position" },
  SB: { name: "Small Blind", at: "im Small Blind", against: "den Small Blind" },
  BB: { name: "Big Blind", at: "im Big Blind", against: "den Big Blind" },
};
const pos = (p: string) => NAMES[p] ?? { name: p, at: `auf ${p}`, against: p };

/** E.g. "Du am Button gegen den Big Blind". You are the player who opened. */
export function titleOf(s: LibraryScenario): string {
  const [opener, other] = seats(s);
  return `Du ${pos(opener).at} gegen ${pos(other).against}`;
}

/** E.g. "40 BB, normaler Raise" or "40 BB, der Big Blind hat 3-gebettet". */
export function situationOf(s: LibraryScenario): string {
  const [, other] = seats(s);
  const what = s.pot_type === "srp" ? "normaler Raise" : `${pos(other).name} hat 3-gebettet`;
  return `${s.stack_bb} BB, ${what}`;
}

/** Title and situation in one line. */
export function labelOf(s: LibraryScenario): string {
  return `${titleOf(s)}, ${situationOf(s)}`;
}

/** What happened before the flop, in words. */
export function storyOf(s: LibraryScenario): string {
  const [opener, other] = seats(s);
  const o = pos(opener).name;
  const x = pos(other).name;
  return s.pot_type === "srp"
    ? `Du erhöhst vom ${o}, der ${x} geht mit.`
    : `Du erhöhst vom ${o}, der ${x} erhöht noch einmal (3-Bet), du gehst mit.`;
}

/** Who acts first after the flop, in words. */
export function firstOf(s: LibraryScenario): string {
  const [opener] = seats(s);
  return s.oop === opener ? "Du handelst nach dem Flop zuerst." : `Der ${pos(s.oop).name} handelt nach dem Flop zuerst, du hast Position.`;
}

/** Stack depth in words. */
export function depthOf(stack: number): string {
  return stack <= 25 ? "kurzer Stack" : stack <= 50 ? "mittlerer Stack" : "tiefer Stack";
}

const SUITS: Record<string, string> = { s: "♠", h: "♥", d: "♦", c: "♣" };

/** "AsKhTd" → [["A","s"],["K","h"],["T","d"]]. */
export function cards(flop: string): [string, string][] {
  return [0, 2, 4].map((i) => [flop[i], flop[i + 1]] as [string, string]);
}

export function suit(s: string): string {
  return SUITS[s] ?? s;
}
