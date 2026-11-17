/**
 * Whitelist of solution kinds the API accepts, with their input and result schemas.
 * Each kind mirrors an implementation of `Kind` in crates/ringside-cloud/src/kinds/; the
 * verifier there decides whether a result is correct, this only rejects malformed data.
 */

export class InvalidError extends Error {}

export interface KindSpec {
  engineVersions: readonly number[];
  /** Largest accepted result, as re-serialized JSON. */
  maxResultBytes: number;
  /** Throws InvalidError unless `input` is a well-formed input of this kind. */
  checkInput(input: unknown): void;
  /** The result with exactly the fields that get stored, or InvalidError; `input` has passed checkInput. */
  normalizeResult(result: unknown, input: unknown): unknown;
}

function record(x: unknown, what: string, keys: readonly string[]): Record<string, unknown> {
  if (typeof x !== "object" || x === null || Array.isArray(x)) throw new InvalidError(`${what} must be an object`);
  const got = Object.keys(x).sort();
  const want = [...keys].sort();
  if (got.length !== want.length || got.some((k, i) => k !== want[i])) {
    throw new InvalidError(`${what} must have exactly the fields ${want.join(", ")}`);
  }
  return x as Record<string, unknown>;
}

function int(x: unknown, what: string, min: number, max: number): number {
  if (typeof x !== "number" || !Number.isInteger(x) || x < min || x > max) {
    throw new InvalidError(`${what} must be an integer in [${min}, ${max}]`);
  }
  return x;
}

function frequencies(x: unknown, what: string, len: number): number[] {
  if (!Array.isArray(x) || x.length !== len) throw new InvalidError(`${what} must be an array of ${len} numbers`);
  for (const f of x) {
    if (typeof f !== "number" || !(f >= 0 && f <= 1)) throw new InvalidError(`${what} values must be in [0, 1]`);
  }
  return x as number[];
}

const NUM_CLASSES = 169;

/** Heads-up push/fold in chip EV; amounts in milli-BB. */
const pushfoldHu: KindSpec = {
  engineVersions: [1],
  maxResultBytes: 16 * 1024,
  checkInput(input) {
    const r = record(input, "input", ["stack_mbb", "sb_mbb", "ante_mbb", "dead_mbb"]);
    const ante = int(r.ante_mbb, "ante_mbb", 0, 1000);
    int(r.sb_mbb, "sb_mbb", 1, 1000);
    int(r.dead_mbb, "dead_mbb", 0, 50_000);
    int(r.stack_mbb, "stack_mbb", 1000 + ante, 200_000);
  },
  normalizeResult(result) {
    const r = record(result, "result", ["shove", "call"]);
    return { shove: frequencies(r.shove, "shove", NUM_CLASSES), call: frequencies(r.call, "call", NUM_CLASSES) };
  },
};

function intArray(x: unknown, what: string, minLen: number, maxLen: number, min: number, max: number): number[] {
  if (!Array.isArray(x) || x.length < minLen || x.length > maxLen) {
    throw new InvalidError(`${what} must be an array of ${minLen} to ${maxLen} integers`);
  }
  return x.map((v, i) => int(v, `${what}[${i}]`, min, max));
}

/** Decision nodes of the engine's tree; mirrors `node_count` in kinds/pushfold_mw.rs. */
export function nodeCount(players: number, cap: number): number {
  const choose = (n: number, k: number) => {
    let r = 1;
    for (let i = 0; i < k; i++) r = (r * (n - i)) / (i + 1);
    return r;
  };
  let count = players - 1;
  for (let shover = 0; shover < players; shover++) {
    for (let seat = shover + 1; seat < players; seat++) {
      const between = seat - shover - 1;
      for (let k = 0; k <= Math.min(between, cap - 2); k++) count += choose(between, k);
    }
  }
  return count;
}

const MAX_PLAYERS = 9;
const POOL_PPM = 1_000_000;

/** Push/fold for 2–9 players, chip EV or ICM; amounts in milli-BB, payouts in ppm of the pool. */
const pushfoldMw: KindSpec = {
  engineVersions: [1],
  // A pure 9-player profile is about 43 KB; mixed frequencies have six decimals.
  maxResultBytes: 192 * 1024,
  checkInput(input) {
    const r = record(input, "input", [
      "stacks_mbb",
      "sb_mbb",
      "ante_mbb",
      "bb_ante_mbb",
      "dead_mbb",
      "max_all_in",
      "payouts_ppm",
    ]);
    const ante = int(r.ante_mbb, "ante_mbb", 0, 1000);
    const bbAnte = int(r.bb_ante_mbb, "bb_ante_mbb", 0, 2000);
    const stacks = intArray(r.stacks_mbb, "stacks_mbb", 2, MAX_PLAYERS, 1000 + ante + bbAnte, 100_000);
    int(r.sb_mbb, "sb_mbb", 1, 1000);
    int(r.dead_mbb, "dead_mbb", 0, 50_000);
    int(r.max_all_in, "max_all_in", 2, 3);
    const payouts = intArray(r.payouts_ppm, "payouts_ppm", 0, stacks.length, 0, POOL_PPM);
    const sum = payouts.reduce((a, b) => a + b, 0);
    if (payouts.length > 0 && Math.abs(sum - POOL_PPM) > MAX_PLAYERS) {
      throw new InvalidError(`payouts_ppm must add up to ${POOL_PPM}`);
    }
  },
  normalizeResult(result, input) {
    const r = record(result, "result", ["act"]);
    const i = input as { stacks_mbb: number[]; max_all_in: number };
    const nodes = nodeCount(i.stacks_mbb.length, i.max_all_in);
    if (!Array.isArray(r.act) || r.act.length !== nodes) throw new InvalidError(`act must have ${nodes} nodes`);
    return { act: r.act.map((row, n) => frequencies(row, `act[${n}]`, NUM_CLASSES)) };
  },
};

export const KINDS: Readonly<Record<string, KindSpec>> = {
  "pushfold-hu": pushfoldHu,
  "pushfold-mw": pushfoldMw,
};

export function kindSpec(kind: string): KindSpec | undefined {
  return Object.hasOwn(KINDS, kind) ? KINDS[kind] : undefined;
}
