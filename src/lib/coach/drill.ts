/** Practising one coached decision: where the hand stops, what the hero could do, and what the coach says. */
import type { Action, Hand, Street } from "../types";
import type { CoachSpot } from "./types";

export type Choice = "fold" | "check" | "call" | "raise" | "allin";

const STREETS: Street[] = ["preflop", "flop", "turn", "river"];
const streetOfBoard = (n: number): Street => (n >= 5 ? "river" : n === 4 ? "turn" : n === 3 ? "flop" : "preflop");
const isPost = (a: Action) => a.kind.startsWith("post_");

/**
 * Index of the hero's decision in `hand.actions`: the last own action up to the spot's street,
 * the same rule the coach uses for the spot's board (crates/ringside-coach, `board_at`).
 */
export function decisionIndex(hand: Hand, spot: CoachSpot): number | null {
  const until = STREETS.indexOf(streetOfBoard(spot.board.length));
  for (let i = hand.actions.length - 1; i >= 0; i--) {
    const a = hand.actions[i];
    if (a.player === hand.hero && !isPost(a) && STREETS.indexOf(a.street) <= until) return i;
  }
  return null;
}

/** What the hero had to pay to stay in, and the stack behind, both in big blinds. */
function situation(hand: Hand, index: number) {
  const at = hand.actions[index];
  const put = new Map<string, number>();
  let earlier = 0;
  for (const a of hand.actions.slice(0, index)) {
    if (a.street !== at.street) {
      if (a.player === hand.hero) earlier += a.amount;
      continue;
    }
    put.set(a.player, a.kind === "raise" && a.to != null ? a.to : (put.get(a.player) ?? 0) + a.amount);
  }
  const most = Math.max(0, ...put.values());
  const pot = hand.actions.slice(0, index).reduce((sum, a) => sum + a.amount, 0);
  const bettor = [...put.entries()].find(([p, v]) => v === most && p !== hand.hero)?.[0] ?? null;
  const mine = put.get(hand.hero ?? "") ?? 0;
  const stack = (hand.seats.find((s) => s.name === hand.hero)?.stack ?? 0) - earlier - mine;
  const bb = hand.big_blind || 1;
  return {
    toCall: Math.max(0, Math.min(most - mine, stack)) / bb,
    behind: stack / bb,
    facing: most > mine,
    pot: pot / bb,
    bettor,
    bet: most / bb,
    street: at.street,
  };
}

/** The hero's stack behind at the decision, in big blinds. */
export const behind = (hand: Hand, index: number) => situation(hand, index).behind;

const STREET_NAME: Record<Street, string> = { preflop: "Vor dem Flop", flop: "Flop", turn: "Turn", river: "River" };
const BOARD_SIZE: Record<Street, number> = { preflop: 0, flop: 3, turn: 4, river: 5 };

/** The decision in one line, for reading before the table: street, board, pot and what the hero faces. */
export function scene(hand: Hand, index: number) {
  const s = situation(hand, index);
  return {
    street: STREET_NAME[s.street],
    board: hand.board.slice(0, BOARD_SIZE[s.street]),
    pot: bb(s.pot),
    facing: s.facing && s.bettor ? `${s.bettor} setzt ${bb(s.bet)}` : s.facing ? `${bb(s.toCall)} zu zahlen` : "Niemand hat gesetzt",
    /** The pot already holds the bet the hero faces. */
    betIn: s.facing,
  };
}

export const bb = (v: number) => `${v.toLocaleString("de-DE", { maximumFractionDigits: 1 })} BB`;

/** The choices the hero had at the decision, as buttons. */
export function choices(hand: Hand, index: number): { id: Choice; label: string; hint: string | null }[] {
  const s = situation(hand, index);
  const list: { id: Choice; label: string; hint: string | null }[] = [];
  if (s.facing) {
    list.push({ id: "fold", label: "Fold", hint: "aufgeben" });
    if (s.toCall < s.behind) list.push({ id: "call", label: "Call", hint: bb(s.toCall) });
    if (s.toCall < s.behind) list.push({ id: "raise", label: "Raise", hint: "erhöhen" });
  } else {
    list.push({ id: "check", label: "Check", hint: "schieben" });
    list.push({ id: "raise", label: "Bet", hint: "setzen" });
  }
  list.push({ id: "allin", label: "All-in", hint: bb(s.behind) });
  return list;
}

/** What the hero actually did. */
export function played(hand: Hand, index: number): Choice {
  const a = hand.actions[index];
  if (a.all_in) return "allin";
  if (a.kind === "bet" || a.kind === "raise") return "raise";
  return a.kind === "fold" || a.kind === "check" || a.kind === "call" ? a.kind : "call";
}

/** The plays a free-text suggestion names ("All-in schieben", "Check-Raise all-in", "Fold"). */
export function choicesIn(text: string): Choice[] {
  const t = text.toLowerCase();
  if (/all-?in|schieb|shove|push/.test(t)) return ["allin"];
  const found: Choice[] = [];
  if (/\bfold|aufgeb|wegwerf/.test(t)) found.push("fold");
  if (/raise|erhöh|3-?bet|bet\b|setz/.test(t)) found.push("raise");
  if (/\bcall|mitgeh|bezahl/.test(t)) found.push("call");
  if (/\bcheck\b(?!-)/.test(t)) found.push("check");
  return found;
}

/** Plays the coach accepts at this decision; null when its words name none. */
export function accepted(spot: CoachSpot, actual: Choice): Choice[] | null {
  const better = spot.better ? choicesIn(spot.better) : [];
  if (spot.verdict === "ok" || spot.verdict === "stark") return [actual, ...better];
  return better.length ? better : null;
}

export const CHOICE_NAME: Record<Choice, string> = { fold: "Fold", check: "Check", call: "Call", raise: "Bet/Raise", allin: "All-in" };
