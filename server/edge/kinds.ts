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
  /** The result with exactly the fields that get stored, or InvalidError. */
  normalizeResult(result: unknown): unknown;
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

export const KINDS: Readonly<Record<string, KindSpec>> = {
  "pushfold-hu": pushfoldHu,
};

export function kindSpec(kind: string): KindSpec | undefined {
  return Object.hasOwn(KINDS, kind) ? KINDS[kind] : undefined;
}
