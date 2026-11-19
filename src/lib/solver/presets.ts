import type { Action, Hand, Street } from "../types";
import type { SolverPlayer, SolverPrefill, SolverSpot, StreetSizes } from "./types";
import { handLabel } from "../app/format";

/** Simple 100-BB cash ranges; a starting point, not a solution. */
export const PRESETS: { id: string; label: string; range: string }[] = [
  { id: "utg", label: "UTG-Open", range: "66+,A8s+,A5s-A4s,KTs+,QTs+,JTs,T9s,98s,87s,AJo+,KQo" },
  { id: "lj", label: "LJ-Open", range: "55+,A7s+,A5s-A3s,K9s+,Q9s+,J9s+,T9s,98s,87s,76s,ATo+,KJo+" },
  { id: "hj", label: "HJ-Open", range: "44+,A2s+,K8s+,Q9s+,J9s+,T8s+,97s+,87s,76s,65s,ATo+,KTo+,QJo" },
  { id: "co", label: "CO-Open", range: "22+,A2s+,K6s+,Q8s+,J8s+,T8s+,97s+,86s+,75s+,65s,54s,A8o+,KTo+,QTo+,JTo" },
  {
    id: "btn",
    label: "BTN-Open",
    range: "22+,A2s+,K2s+,Q4s+,J6s+,T6s+,96s+,85s+,75s+,64s+,54s,A2o+,K8o+,Q9o+,J9o+,T8o+,98o",
  },
  {
    id: "sb",
    label: "SB-Open",
    range: "22+,A2s+,K2s+,Q2s+,J4s+,T6s+,96s+,85s+,74s+,64s+,53s+,43s,A2o+,K5o+,Q8o+,J8o+,T8o+,98o,87o",
  },
  {
    id: "bb-btn",
    label: "BB gegen BTN",
    range: "22-99,A2s-AJs,K2s-KJs,Q2s-QJs,J4s+,T6s+,96s+,85s+,74s+,63s+,53s+,43s,A2o-AJo,K8o-KJo,Q9o+,J9o+,T8o+,98o,87o",
  },
  {
    id: "bb-co",
    label: "BB gegen CO",
    range: "22-99,A2s-AJs,K5s-KJs,Q7s-QJs,J7s+,T7s+,97s+,86s+,75s+,64s+,54s,A8o-AJo,KTo-KJo,QTo+,JTo",
  },
  {
    id: "bb-early",
    label: "BB gegen frühe Position",
    range: "22-TT,A2s-AJs,K8s-KQs,Q9s+,J9s+,T8s+,98s,87s,76s,65s,ATo-AQo,KJo+,QJo",
  },
  {
    id: "bb-sb",
    label: "BB gegen SB",
    range: "22-88,A2s-A9s,K2s-KTs,Q2s-QJs,J2s+,T4s+,95s+,84s+,74s+,63s+,53s+,43s,A2o-ATo,K5o-KJo,Q7o+,J7o+,T7o+,97o+,87o,76o",
  },
  { id: "flat", label: "Call gegen Open", range: "22-TT,A9s-AJs,A5s-A4s,KTs-KQs,QTs+,JTs,T9s,98s,87s,76s,AJo-AQo,KQo" },
  { id: "3bet", label: "3-Bet", range: "TT+,AJs+,KQs,A5s-A2s,K9s,Q9s,J9s,T8s,AQo+,KJo" },
  { id: "call3", label: "Call gegen 3-Bet", range: "77-QQ,AJs-AQs,A5s-A4s,KTs+,QTs+,JTs,T9s,98s,AQo,KQo" },
  { id: "4bet", label: "4-Bet", range: "QQ+,AKs,AKo,A5s:0.5" },
  { id: "call4", label: "Call gegen 4-Bet", range: "TT-QQ,AQs-AKs,AKo" },
  {
    id: "limp",
    label: "Limp",
    range: "22-66,A2s-A9s,K2s-KTs,Q4s-QTs,J6s-JTs,T6s+,96s+,85s+,75s+,64s+,54s,A2o-A9o,K7o-KJo,Q9o+,J9o+,T8o+,98o",
  },
  { id: "random", label: "Alle Hände", range: "random" },
];

export function preset(id: string): string {
  return PRESETS.find((p) => p.id === id)?.range ?? "";
}

/** Typical sizes: two on the flop, one on turn and river, one raise each. */
export function defaultSizes(): [StreetSizes, StreetSizes, StreetSizes] {
  return [
    { bet: [33, 75], raise: [60] },
    { bet: [66], raise: [100] },
    { bet: [75], raise: [100] },
  ];
}

export function defaultSpot(): SolverSpot {
  return {
    oop_range: preset("bb-btn"),
    ip_range: preset("btn"),
    board: "Ks7d2c",
    pot: 5.5,
    stack: 37.5,
    sizes: defaultSizes(),
    allin_threshold: 150,
  };
}

/** Postflop order: the small blind acts first, the button last. */
const ORDER = ["SB", "BB", "UTG", "UTG+1", "UTG+2", "UTG+3", "UTG+4", "LJ", "HJ", "CO", "BTN"];
const EARLY = ["UTG", "UTG+1", "UTG+2", "UTG+3", "UTG+4", "LJ", "HJ"];
const BOARD_AT: Record<Street, number> = { preflop: 0, flop: 3, turn: 4, river: 5 };
const STREETS: Street[] = ["preflop", "flop", "turn", "river"];

export function openRange(pos: string): { id: string; note: string } {
  const id = pos === "BTN" ? "btn" : pos === "CO" ? "co" : pos === "SB" ? "sb" : pos === "HJ" ? "hj" : pos === "LJ" ? "lj" : "utg";
  return { id, note: `Open-Raise aus ${pos}` };
}

export function defendRange(pos: string, opener: string): { id: string; note: string } {
  if (pos !== "BB") return { id: "flat", note: `Call gegen Open aus ${opener}` };
  const id = opener === "BTN" ? "bb-btn" : opener === "CO" ? "bb-co" : opener === "SB" ? "bb-sb" : EARLY.includes(opener) ? "bb-early" : "bb-co";
  return { id, note: `Big Blind verteidigt gegen ${opener}` };
}

/**
 * The heads-up spot at the start of `street`, or why there is none. Ranges are simple
 * templates chosen from positions and the preflop action, and say so.
 */
export function spotFromHand(
  hand: Hand,
  street: Street,
  positions: Map<string, string>,
): { prefill: SolverPrefill } | { reason: string } {
  if (street === "preflop") return { reason: "Erst ab dem Flop" };
  const before: Action[] = [];
  for (const a of hand.actions) {
    if (STREETS.indexOf(a.street) >= STREETS.indexOf(street)) break;
    before.push(a);
  }
  const folded = new Set(before.filter((a) => a.kind === "fold").map((a) => a.player));
  const players = hand.seats.filter((s) => !s.sitting_out && !folded.has(s.name)).map((s) => s.name);
  if (players.length !== 2) return { reason: "Nur Heads-up-Spots lassen sich lösen" };
  if (before.some((a) => a.all_in && players.includes(a.player))) return { reason: "Schon all-in, nichts mehr zu entscheiden" };
  if (hand.board.length < BOARD_AT[street]) return { reason: "Board unvollständig" };
  if (hand.big_blind <= 0) return { reason: "Big Blind unbekannt" };

  const put = new Map<string, number>();
  let pot = 0;
  for (const a of before) {
    pot += a.amount;
    put.set(a.player, (put.get(a.player) ?? 0) + a.amount);
  }
  const behind = (name: string) => (hand.seats.find((s) => s.name === name)?.stack ?? 0) - (put.get(name) ?? 0);
  const stack = Math.min(...players.map(behind));
  if (stack <= 0) return { reason: "Kein Stack mehr dahinter" };

  const pos = (n: string) => positions.get(n) ?? "?";
  const [oop, ip] = [...players].sort((a, b) => ORDER.indexOf(pos(a)) - ORDER.indexOf(pos(b)));

  const raises = before.filter((a) => a.street === "preflop" && a.kind === "raise");
  const role = new Map<string, { id: string; note: string }>();
  const last = raises.at(-1)?.player;
  const other = (n: string) => (n === oop ? ip : oop);
  if (raises.length === 0) {
    for (const n of players) {
      role.set(n, pos(n) === "BB" ? { id: "random", note: "Big Blind checkt im Limp-Pot" } : { id: "limp", note: "Limp" });
    }
  } else if (raises.length === 1 && last && players.includes(last)) {
    role.set(last, openRange(pos(last)));
    role.set(other(last), defendRange(pos(other(last)), pos(last)));
  } else if (raises.length === 2 && last && players.includes(last)) {
    role.set(last, { id: "3bet", note: `3-Bet aus ${pos(last)}` });
    role.set(other(last), { id: "call3", note: "Call gegen die 3-Bet" });
  } else if (last && players.includes(last)) {
    role.set(last, { id: "4bet", note: `4-Bet aus ${pos(last)}` });
    role.set(other(last), { id: "call4", note: "Call gegen die 4-Bet" });
  } else {
    // The last raiser folded later; both just called.
    for (const n of players) role.set(n, { id: "flat", note: "Call vor dem Flop" });
  }

  const side = (n: string) => {
    const r = role.get(n) ?? { id: "random", note: "unbekannt" };
    return { position: pos(n), range: preset(r.id), note: r.note };
  };
  const bb = (x: number) => Math.round((x / hand.big_blind) * 100) / 100;
  const heroIn = hand.hero && players.includes(hand.hero) && hand.hero_cards.length === 2;
  const heroSide: SolverPlayer = hand.hero === oop ? "oop" : "ip";
  return {
    prefill: {
      board: hand.board.slice(0, BOARD_AT[street]),
      pot: bb(pot),
      stack: bb(stack),
      oop: side(oop),
      ip: side(ip),
      hero: heroIn ? { player: heroSide, cards: hand.hero_cards } : null,
      players: hand.seats.filter((s) => !s.sitting_out).length,
      source: `Hand #${handLabel(hand.id)}, ${street === "flop" ? "Flop" : street === "turn" ? "Turn" : "River"}`,
    },
  };
}
