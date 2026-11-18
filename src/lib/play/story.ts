// The situation at a step of the hand in plain words: who did what since your last move,
// and where you stand after the flop.
import { cardText, num } from "./labels";
import type { HandView, PlayEvent, Street } from "./types";

type Action = Extract<PlayEvent, { type: "action" }>;

const STREET: Record<Street, string> = { preflop: "Preflop", flop: "Flop", turn: "Turn", river: "River" };

/** "BTN, SB und BB" */
export function listJoin(xs: string[]): string {
  return xs.length < 2 ? (xs[0] ?? "") : `${xs.slice(0, -1).join(", ")} und ${xs[xs.length - 1]}`;
}

export interface Situation {
  /** What the opponents did, e.g. "CO hat auf 6,1 BB erhöht". */
  lead: string;
  /** "3-Bet", "4-Bet" for a re-raise before the flop. */
  raise: string | null;
  folds: string;
  /** The card dealt since your last move, e.g. "Turn: 2♥". */
  board: string | null;
  you: string;
  /** Equity a call needs, 0–1. */
  need: number | null;
}

/** What happened since your last move on this street, for the line above your buttons. */
export function situation(view: HandView, at: number): Situation {
  const pos = view.positions;
  const upto = view.events.filter((e) => e.snapshot <= at);
  let from = 0;
  upto.forEach((e, i) => {
    if (e.type === "board" || (e.type === "action" && e.seat === view.hero)) from = i + 1;
  });
  const recent = upto.slice(from).filter((e): e is Action => e.type === "action");
  let board: string | null = null;
  for (const e of upto) {
    if (e.type === "board") board = `${STREET[e.street]}: ${e.cards.map(cardText).join("")}`;
    else if (e.seat === view.hero) board = null;
  }
  const folds = recent.filter((e) => e.choice.kind === "fold").map((e) => pos[e.seat]);
  const aggressive = (e: Action) => e.choice.kind === "bet" || e.choice.kind === "raise" || e.choice.kind === "all_in";
  const lastAgg = [...recent].reverse().find(aggressive);
  const parts: string[] = [];
  let raise: string | null = null;
  if (lastAgg) {
    const c = lastAgg.choice;
    const who = pos[lastAgg.seat];
    parts.push(c.kind === "bet" ? `${who} setzt ${num(c.to_bb)} BB` : c.kind === "raise" ? `${who} hat auf ${num(c.to_bb)} BB erhöht` : `${who} geht all-in (${num(c.to_bb)} BB)`);
    if (lastAgg.street === "preflop") {
      const raises = upto.filter((e): e is Action => e.type === "action" && e.street === "preflop" && e.snapshot <= lastAgg.snapshot && (e.choice.kind === "raise" || e.choice.kind === "all_in")).length;
      if (raises >= 2) raise = `${raises + 1}-Bet`;
    }
    const callers = recent.filter((e) => e.snapshot > lastAgg.snapshot && e.choice.kind === "call").map((e) => pos[e.seat]);
    if (callers.length) parts.push(`${listJoin(callers)} ${callers.length > 1 ? "callen" : "callt"}`);
  } else {
    const passive = recent.filter((e) => e.choice.kind !== "fold");
    for (const e of passive) parts.push(`${pos[e.seat]} ${e.choice.kind === "check" ? "checkt" : e.choice.kind === "limp" ? "limpt" : "callt"}`);
  }
  const foldText = folds.length ? `${listJoin(folds)} ${folds.length > 1 ? "haben" : "hat"} gefoldet` : "";
  const d = view.decision;
  const cards = view.hero_cards.map(cardText).join("");
  let you = `Du bist dran mit ${cards}`;
  let need: number | null = null;
  if (d) {
    you += d.to_call > 0 ? ` – ${num(d.to_call)} BB zum Callen, Pot ${num(d.pot)} BB.` : ` – Pot ${num(d.pot)} BB, du kannst checken.`;
    if (d.to_call > 0) need = d.to_call / (d.pot + d.to_call);
  } else you += ".";
  if (!parts.length && !foldText) return { lead: "Du bist als Erster dran", raise: null, folds: "", board, you, need };
  return { lead: parts.join(", "), raise, folds: foldText, board, you, need };
}

/** Order after the flop: the small blind acts first, the button last. */
function postflopRank(seat: number, n: number): number {
  if (n === 2) return seat === 0 ? 1 : 0;
  return (seat - (n - 2) + n) % n;
}

export interface StreetHead {
  street: string;
  players: string;
  inPosition: boolean;
  others: string;
  pot: number;
  effective: number;
  headsUp: boolean;
}

/** From the flop on: who is still in, whether you act last, pot and effective stack. */
export function streetHead(view: HandView, at: number): StreetHead | null {
  const snap = view.snapshots[at];
  if (!snap || snap.street === "preflop" || snap.folded[view.hero]) return null;
  const n = view.players;
  const left = Array.from({ length: n }, (_, s) => s).filter((s) => !snap.folded[s]);
  const others = left.filter((s) => s !== view.hero);
  if (!others.length) return null;
  const inPosition = others.every((s) => postflopRank(s, n) < postflopRank(view.hero, n));
  const effective = Math.min(snap.stacks[view.hero] + snap.bets[view.hero], Math.max(...others.map((s) => snap.stacks[s] + snap.bets[s])));
  return {
    street: STREET[snap.street],
    players: others.length === 1 ? "Heads-up" : `${left.length} Spieler`,
    inPosition,
    others: listJoin(others.map((s) => view.positions[s])),
    pot: snap.pot,
    effective,
    headsUp: others.length === 1,
  };
}
