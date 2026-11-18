// A heads-up postflop spot described the way players think about it: table size, the two
// seats, how the pot came about and the stack depth. Ranges, pot and effective stack follow
// from the engine's preflop model.
import { api } from "../api";
import type { PreflopAnte, PreflopKind, PreflopSpot, PreflopSpots } from "../charts/preflop-types";
import { defendRange, openRange, preset } from "./presets";
import { CLASSES, formatRange } from "./range";
import type { ModelLine } from "./types";

export type PotType = "srp" | "3bet" | "limp";
export interface Scenario {
  /** Seats at the table; the preflop model covers 6, 8 and 9. */
  players: number;
  /** The two seats in the hand, by position name; order does not matter. */
  seats: [string, string];
  pot: PotType;
  /** Starting stack of everyone, in BB. */
  stack: number;
  ante: PreflopAnte;
}

export const ANTES: { id: PreflopAnte; label: string }[] = [
  { id: "big_blind", label: "BB-Ante" },
  { id: "classic", label: "Ante" },
  { id: "none", label: "Ohne" },
];

export const POT_TYPES: { id: PotType; label: string; hint: string }[] = [
  { id: "srp", label: "Raise + Call", hint: "Single-Raised-Pot (SRP): einer erhöht, einer geht mit" },
  { id: "3bet", label: "3-Bet", hint: "3-Bet-Pot: Open, 3-Bet und Call" },
  { id: "limp", label: "Limp", hint: "Limp-Pot: Small Blind limpt, Big Blind checkt" },
];

/** Preflop action order, as in `ringside_engine::multiway::position_names`. */
export function positionsFor(players: number): string[] {
  if (players <= 2) return ["BTN", "BB"];
  const late = ["LJ", "HJ", "CO"];
  const rest = players - 3;
  const early = ["UTG", "UTG+1", "UTG+2"].slice(0, Math.max(0, rest - late.length));
  return [...early, ...late.slice(late.length - (rest - early.length)), "BTN", "SB", "BB"];
}

/** After the flop the blinds act first and the button last. */
export function postflopRank(players: number, pos: string): number {
  const order = positionsFor(players);
  const i = order.indexOf(pos);
  return pos === "SB" ? -2 : pos === "BB" ? -1 : i;
}

export function oopIp(players: number, seats: [string, string]): { oop: string; ip: string } {
  const [a, b] = seats;
  return postflopRank(players, a) < postflopRank(players, b) ? { oop: a, ip: b } : { oop: b, ip: a };
}

/** Who acted first before the flop, i.e. who opened. */
function byPreflop(players: number, seats: [string, string]): [string, string] {
  const order = positionsFor(players);
  return order.indexOf(seats[0]) < order.indexOf(seats[1]) ? [seats[0], seats[1]] : [seats[1], seats[0]];
}

/** Why the pot type does not fit the seats, or null. */
export function potTypeProblem(s: Pick<Scenario, "seats" | "pot">): string | null {
  if (s.pot !== "limp") return null;
  const set = new Set(s.seats);
  return set.has("SB") && set.has("BB") ? null : "Im Preflop-Modell limpt nur der Small Blind; wähle SB und BB.";
}

type Want = { pos: string; line: { pos: string; kind: PreflopKind }[]; take: PreflopKind };

/** The decisions whose frequencies make up each player's range, in order. */
function wanted(s: Scenario): { first: string; second: string; first_steps: Want[]; second_steps: Want[] } {
  const [first, second] = byPreflop(s.players, s.seats);
  if (s.pot === "limp") {
    return {
      first,
      second,
      first_steps: [{ pos: first, line: [], take: "limp" }],
      second_steps: [{ pos: second, line: [{ pos: first, kind: "limp" }], take: "check" }],
    };
  }
  const open: Want = { pos: first, line: [], take: "raise" };
  if (s.pot === "srp") {
    return { first, second, first_steps: [open], second_steps: [{ pos: second, line: [{ pos: first, kind: "raise" }], take: "call" }] };
  }
  const three: Want = { pos: second, line: [{ pos: first, kind: "raise" }], take: "raise" };
  const call: Want = {
    pos: first,
    line: [
      { pos: first, kind: "raise" },
      { pos: second, kind: "raise" },
    ],
    take: "call",
  };
  return { first, second, first_steps: [open, call], second_steps: [three] };
}

function findSpot(spots: PreflopSpots, w: Want): { spot: PreflopSpot; action: number } | null {
  const spot = spots.nodes.find(
    (n) =>
      n.position === w.pos &&
      n.line.length === w.line.length &&
      n.line.every((step, i) => step.position === w.line[i].pos && step.kind === w.line[i].kind),
  );
  if (!spot) return null;
  const action = spot.actions.findIndex((a) => a.kind === w.take);
  return action < 0 ? null : { spot, action };
}

export interface Derived {
  oop: string;
  ip: string;
  oopRange: string;
  ipRange: string;
  /** What each range is, e.g. "Open 2,2 BB". */
  oopNote: string;
  ipNote: string;
  pot: number;
  stack: number;
  /** The model depth the ranges come from; can differ from the chosen stack. */
  modelStack: number;
  /** Chips each player put in before the flop, for the table. */
  put: Record<string, number>;
  labels: Record<string, string>;
  /** The preflop line, when ranges, pot and stack are the model's own: the spot is shared. */
  model: ModelLine | null;
}

const fmt = (x: number) => x.toLocaleString("de-DE", { maximumFractionDigits: 2 });

function nearest(list: number[], x: number): number {
  return list.reduce((best, v) => (Math.abs(v - x) < Math.abs(best - x) ? v : best), list[0]);
}

const spotsCache = new Map<string, Promise<PreflopSpots>>();

function spotsFor(ante: PreflopAnte, players: number, stack: number): Promise<PreflopSpots> {
  const key = `${ante}/${players}/${stack}`;
  let hit = spotsCache.get(key);
  if (!hit) {
    hit = api.preflopSpots(ante, players, stack);
    hit.catch(() => spotsCache.delete(key));
    spotsCache.set(key, hit);
  }
  return hit;
}

/**
 * Ranges, pot and effective stack for `s`. Ranges come from the nearest precomputed depth so
 * that nothing has to be solved live; pot and stack use the depth chosen.
 */
export async function derive(s: Scenario): Promise<Derived> {
  const problem = potTypeProblem(s);
  if (problem) throw new Error(problem);
  const grid = await api.preflopGrid();
  if (!grid.players.includes(s.players)) throw new Error(`Das Preflop-Modell gibt es für ${grid.players.join(", ")} Spieler.`);
  const modelStack = nearest(grid.stacks, s.stack);
  const spots = await spotsFor(s.ante, s.players, modelStack);
  if (spots.players !== s.players) throw new Error(`Für ${s.players} Spieler liegen hier keine Preflop-Daten vor.`);
  const w = wanted(s);

  const weightsOf = async (steps: Want[]): Promise<number[]> => {
    let weights = new Array<number>(CLASSES).fill(1);
    for (const step of steps) {
      const hit = findSpot(spots, step);
      if (!hit) throw new Error(`Die Linie „${describe(step)}“ gibt es im Preflop-Modell nicht.`);
      const node = await api.preflopNode(s.ante, s.players, modelStack, hit.spot.id);
      const freq = node.freq[hit.action];
      weights = weights.map((x, i) => x * (freq[i] / 255));
    }
    // Two decimals keep the range text readable; rarer hands barely change the solve.
    return weights.map((x) => (x < 0.03 ? 0 : Math.round(x * 100) / 100));
  };
  const [firstW, secondW] = await Promise.all([weightsOf(w.first_steps), weightsOf(w.second_steps)]);
  if (!firstW.some((x) => x > 0) || !secondW.some((x) => x > 0)) throw new Error("Für diese Linie spielt das Modell keine Hände.");

  // Chips in front before the flop, from the model's own sizes.
  const toOf = (want: Want) => {
    const hit = findSpot(spots, want);
    return hit ? hit.spot.actions[hit.action].to_bb : 0;
  };
  const steps = [...w.first_steps, ...w.second_steps];
  const to = Math.max(...steps.map(toOf), 1);
  const blinds = (s.seats.includes("SB") ? 0 : 0.5) + (s.seats.includes("BB") ? 0 : 1);
  const antes = s.players * spots.ante_bb + spots.bb_ante_bb;
  const pot = Math.round((2 * to + blinds + antes) * 100) / 100;
  const behind = (pos: string) => s.stack - to - spots.ante_bb - (pos === "BB" ? spots.bb_ante_bb : 0);
  const stack = Math.round(Math.min(behind(s.seats[0]), behind(s.seats[1])) * 100) / 100;
  if (stack <= 0) throw new Error("Bei dieser Tiefe ist preflop schon alles im Pot.");

  const { oop, ip } = oopIp(s.players, s.seats);
  const raise3 = s.pot === "3bet" ? toOf(w.second_steps[0]) : 0;
  const open = toOf(w.first_steps[0]);
  const labels: Record<string, string> =
    s.pot === "limp"
      ? { [w.first]: "Limp", [w.second]: "Check" }
      : s.pot === "srp"
        ? { [w.first]: `Open ${fmt(open)}`, [w.second]: "Call" }
        : { [w.first]: `Open ${fmt(open)} · Call`, [w.second]: `3-Bet ${fmt(raise3)}` };
  const notes: Record<string, string> =
    s.pot === "limp"
      ? { [w.first]: "Limp aus dem Small Blind", [w.second]: "Check im Big Blind" }
      : s.pot === "srp"
        ? { [w.first]: `Open-Raise auf ${fmt(open)} BB`, [w.second]: `Call gegen ${w.first}` }
        : { [w.first]: `Open, Call gegen die 3-Bet`, [w.second]: `3-Bet auf ${fmt(raise3)} BB` };
  const range = { [w.first]: formatRange(firstW), [w.second]: formatRange(secondW) };
  // At a model depth the model's own spot is used as is, as practice and the cloud know it.
  const own = modelStack === s.stack ? await api.solverModelSpot(s).catch(() => null) : null;
  return {
    oop,
    ip,
    oopRange: own?.oop_range ?? range[oop],
    ipRange: own?.ip_range ?? range[ip],
    oopNote: notes[oop],
    ipNote: notes[ip],
    pot: own?.pot ?? pot,
    stack: own?.stack ?? stack,
    modelStack,
    put: { [s.seats[0]]: to, [s.seats[1]]: to },
    labels,
    model: own?.model ?? null,
  };
}

function describe(w: Want): string {
  const before = w.line.map((l) => `${l.pos} ${l.kind}`).join(", ");
  return `${before ? before + ", " : ""}${w.pos} ${w.take}`;
}

/** Where the ranges come from, in a few words. */
export function originText(s: Scenario, modelStack: number): string {
  const depth = modelStack === s.stack ? `${fmt(modelStack)} BB` : `${fmt(modelStack)} BB (nächste vorgerechnete Tiefe)`;
  const ante = s.ante === "none" ? "ohne Ante" : s.ante === "classic" ? "mit Ante" : "mit BB-Ante";
  return `aus dem Preflop-Modell, ${s.players}-max, ${depth}, ${ante}`;
}

/** Simple template ranges and standard sizes, for when the preflop model is not available. */
export function template(s: Scenario): Derived {
  const [first, second] = byPreflop(s.players, s.seats);
  const { oop, ip } = oopIp(s.players, s.seats);
  const roles: Record<string, { id: string; note: string }> =
    s.pot === "limp"
      ? { [first]: { id: "limp", note: "Limp" }, [second]: { id: "random", note: "Big Blind checkt im Limp-Pot" } }
      : s.pot === "srp"
        ? { [first]: openRange(first), [second]: defendRange(second, first) }
        : { [first]: { id: "call3", note: "Call gegen die 3-Bet" }, [second]: { id: "3bet", note: `3-Bet aus ${second}` } };
  const to = s.pot === "limp" ? 1 : s.pot === "srp" ? 2.5 : 8;
  const blinds = (s.seats.includes("SB") ? 0 : 0.5) + (s.seats.includes("BB") ? 0 : 1);
  const labels: Record<string, string> =
    s.pot === "limp"
      ? { [first]: "Limp", [second]: "Check" }
      : s.pot === "srp"
        ? { [first]: `Open ${fmt(to)}`, [second]: "Call" }
        : { [first]: "Open · Call", [second]: `3-Bet ${fmt(to)}` };
  return {
    oop,
    ip,
    oopRange: preset(roles[oop].id),
    ipRange: preset(roles[ip].id),
    oopNote: roles[oop].note,
    ipNote: roles[ip].note,
    pot: 2 * to + blinds,
    stack: Math.max(1, s.stack - to),
    modelStack: s.stack,
    put: { [s.seats[0]]: to, [s.seats[1]]: to },
    labels,
    model: null,
  };
}
