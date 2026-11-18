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

/** Spot catalog of the postflop library; mirrors crates/ringside-solver/data/library-v1.json. */
export const POSTFLOP_CATALOG = 1;
export const POSTFLOP_SCENARIOS: readonly string[] = [
  "srp-btn-bb-20", "srp-co-bb-20", "srp-utg-bb-20", "srp-sb-bb-20",
  "srp-btn-bb-40", "srp-co-bb-40", "srp-utg-bb-40", "srp-sb-bb-40",
  "3bp-btn-bb-40", "3bp-btn-sb-40", "3bp-co-btn-40", "srp-btn-bb-75",
  "srp-co-bb-75", "srp-utg-bb-75", "srp-sb-bb-75", "3bp-btn-bb-75",
  "3bp-btn-sb-75", "3bp-co-btn-75",
];
export const POSTFLOP_FLOPS: readonly string[] = `
  AsKhTd AsKh5d AsQhTd AsQh5d AsJh9d AsJh4d AsTh7d AsTh2d As9h4d As8h5d As7h5d As6h4d
  As5h2d KsQhTd KsQh5d KsJh9d KsJh4d KsTh7d KsTh2d Ks9h4d Ks8h5d Ks7h5d Ks6h4d Ks5h2d
  QsJh9d QsJh4d QsTh7d QsTh2d Qs9h3d Qs8h4d Qs7h4d Qs6h3d Qs4h3d JsTh7d JsTh2d Js9h4d
  Js8h5d Js7h5d Js6h4d Js5h2d Ts9h7d Ts9h2d Ts8h3d Ts7h3d Ts6h2d Ts4h2d 9s8h4d 9s7h4d
  9s6h3d 9s4h3d 8s7h4d 8s6h3d 8s4h3d 7s6h3d 7s4h3d 6s5h2d 5s4h2d AsKs7h AsKhTs AsKh5s
  AsQs9h AsQhTs AsQh5h AsJs7h AsJh8s AsJh3s AsTs2h AsTh5h As9s5h As9h5s As8s5h As8h4h
  As7h6s As6s5h As6h2s As5h2h KsQsTh KsQhJh KsQh6h KsJs9h KsJh9h KsJh4h KsTs4h KsTh6h
  Ks9s7h Ks9h6h Ks8s6h Ks8h5h Ks7s3h Ks7h2s Ks6h3h Ks5h2s Ks3h2h QsJhTs QsJh5h QsTs6h
  QsTh7h QsTh2h Qs9h7s Qs9h2h Qs8h6h Qs7s5h Qs7h3s Qs6h4s Qs5h3s Qs3s2h JsTs2h JsTh5h
  Js9s4h Js9h5h Js8s4h Js8h4h Js7h6s Js6s4h Js6h2h Js4s3h Ts9s7h Ts9h6s Ts8s6h Ts8h5h
  Ts7s3h Ts7h2s Ts6h3h Ts5h2s Ts3h2h 9s8h6h 9s7s5h 9s7h3h 9s6h4h 9s5h3s 9s3s2h 8s7h5s
  8s6s2h 8s5s3h 8s4h3s 7s6s2h 7s5s2h 7s4h3h 6s5h4h 6s4h2h 5s3s2h AsQs8s As9s4s KsQs6s
  Ks8s7s QsJs3s Qs6s4s Js8s3s Ts8s2s 9s6s3s 7s5s3s AsAhJd AsAh6d AsKhKd As8h8d As3h3d
  KsKh9d KsKh4d KsThTd Ks5h5d QsQhTd QsQh5d QsThTd Qs5h5d JsJh9d JsJh4d Js7s7h Js2s2h
  TsTh5s Ts8s8h Ts3s3h 9s9h5s 9s7s7h 9s2s2h 8s8h3s 8s4s4h 7s7h4s 7s4s4h 6s6h3s 6s2s2h
  5s3s3h 4s2s2h JsJhJd 5s5h5d
`.trim().split(/\s+/);
/** Largest stored spot in bytes (MAX_BYTES in kinds/postflop.rs), as base64 characters. */
const POSTFLOP_MAX_BASE64 = Math.ceil(180_000 / 3) * 4;
const BASE64_RE = /^[A-Za-z0-9+/]*={0,2}$/;

/**
 * Library spots: the input names a catalog entry, the result is the stored spot (base64).
 * Only the header is read here; the verifier reads and re-solves the rest.
 */
const postflop: KindSpec = {
  engineVersions: [1],
  maxResultBytes: POSTFLOP_MAX_BASE64 + 64,
  checkInput(input) {
    const r = record(input, "input", ["catalog", "scenario", "flop"]);
    int(r.catalog, "catalog", POSTFLOP_CATALOG, POSTFLOP_CATALOG);
    if (typeof r.scenario !== "string" || !POSTFLOP_SCENARIOS.includes(r.scenario)) {
      throw new InvalidError("scenario is not in the catalog");
    }
    if (typeof r.flop !== "string" || !POSTFLOP_FLOPS.includes(r.flop)) throw new InvalidError("flop is not in the catalog");
  },
  normalizeResult(result, input) {
    const data = storedData(result);
    checkHeader(data, input as { catalog: number; scenario: string; flop: string });
    return { data };
  },
};

const RANKS = "23456789TJQKA";
const SUITS = "cdhs";

/**
 * The canonical flop of a suit class, as `library::flops::canonical` in Rust: of all 24
 * suit renamings the one that sorts first with cards highest rank first, suits s, h, d, c.
 */
export function canonicalFlop(flop: string): string {
  if (!/^([2-9TJQKA][cdhs]){3}$/.test(flop)) throw new InvalidError("board must be a flop like AsKs2h");
  const cards = [0, 2, 4].map((i) => ({ rank: RANKS.indexOf(flop[i]), suit: SUITS.indexOf(flop[i + 1]) }));
  if (new Set(cards.map((c) => c.rank * 4 + c.suit)).size !== 3) throw new InvalidError("board has a card twice");
  const key = (c: { rank: number; suit: number }) => (12 - c.rank) * 4 + (3 - c.suit);
  let best: number[] | undefined;
  for (const perm of permutations([0, 1, 2, 3])) {
    const k = cards.map((c) => key({ rank: c.rank, suit: perm[c.suit] })).sort((a, b) => a - b);
    const first = best === undefined ? -1 : k.findIndex((x, i) => x !== best![i]);
    if (best === undefined || (first >= 0 && k[first] < best[first])) best = k;
  }
  return best!.map((k) => RANKS[12 - Math.floor(k / 4)] + SUITS[3 - (k % 4)]).join("");
}

function permutations(xs: number[]): number[][] {
  if (xs.length <= 1) return [xs];
  return xs.flatMap((x, i) => permutations([...xs.slice(0, i), ...xs.slice(i + 1)]).map((p) => [x, ...p]));
}

/** Versions of the preflop model the verifier rebuilds ranges with; mirrors MODEL_VERSION in kinds/spot.rs. */
export const SPOT_MODELS: readonly number[] = [2];
/** Table families and their table sizes; mirror `preflop_table::Family` in ringside-engine. */
export const SPOT_FAMILIES: Readonly<Record<string, readonly number[]>> = {
  plain: [6, 8, 9],
  limps: [2, 3, 4, 5, 6, 8, 9],
};
export const SPOT_STACKS: readonly number[] = [15, 20, 25, 30, 40, 50, 75, 100];
const SPOT_ANTES = ["none", "classic", "big_blind"];
const SPOT_ACTIONS = ["check", "call", "limp", "raise", "all_in"];
const SPOT_MAX_LINE = 8;
const SPOT_MAX_CBB = 50_000;
const SPOT_MAX_SIZES = 4;

/**
 * Heads-up flops off the catalog (kinds/spot.rs): the ranges are named by the preflop
 * model's configuration and line, never sent; the verifier rebuilds them. Only the shape
 * is checked here: whether the line exists in the model and the tree fits is the
 * verifier's call.
 */
const spot: KindSpec = {
  engineVersions: [1],
  maxResultBytes: POSTFLOP_MAX_BASE64 + 64,
  checkInput(input) {
    const r = record(input, "input", [
      "model",
      "family",
      "ante",
      "players",
      "stack_bb",
      "line",
      "board",
      "pot_cbb",
      "stack_cbb",
      "sizes",
      "allin_pct",
    ]);
    if (!SPOT_MODELS.includes(r.model as number)) throw new InvalidError("model is not a known model version");
    const sizes = typeof r.family === "string" && Object.hasOwn(SPOT_FAMILIES, r.family) ? SPOT_FAMILIES[r.family] : null;
    if (!sizes) throw new InvalidError(`family must be one of ${Object.keys(SPOT_FAMILIES).join(", ")}`);
    if (typeof r.ante !== "string" || !SPOT_ANTES.includes(r.ante)) throw new InvalidError("unknown ante");
    const players = int(r.players, "players", 2, 9);
    if (!sizes.includes(players)) throw new InvalidError("players has no model table in this family");
    if (!SPOT_STACKS.includes(int(r.stack_bb, "stack_bb", 1, 1000))) throw new InvalidError("stack_bb has no model table");
    if (!Array.isArray(r.line) || r.line.length < 1 || r.line.length > SPOT_MAX_LINE) {
      throw new InvalidError(`line must have 1 to ${SPOT_MAX_LINE} actions`);
    }
    r.line.forEach((step, i) => {
      if (!Array.isArray(step) || step.length !== 3) throw new InvalidError(`line[${i}] must be [seat, action, to_mbb]`);
      int(step[0], `line[${i}] seat`, 0, players - 1);
      if (typeof step[1] !== "string" || !SPOT_ACTIONS.includes(step[1])) throw new InvalidError(`line[${i}] action`);
      int(step[2], `line[${i}] to_mbb`, 0, 1_000_000);
    });
    if (typeof r.board !== "string" || canonicalFlop(r.board) !== r.board) {
      throw new InvalidError("board must be the canonical flop of its suit class");
    }
    int(r.pot_cbb, "pot_cbb", 100, SPOT_MAX_CBB);
    int(r.stack_cbb, "stack_cbb", 1, SPOT_MAX_CBB);
    if (!Array.isArray(r.sizes) || r.sizes.length !== 6) throw new InvalidError("sizes must have six lists");
    r.sizes.forEach((list, i) => {
      const v = intArray(list, `sizes[${i}]`, 0, SPOT_MAX_SIZES, 1, 1000);
      if (v.some((x, j) => j > 0 && x <= v[j - 1])) throw new InvalidError(`sizes[${i}] must ascend`);
    });
    int(r.allin_pct, "allin_pct", 0, 1000);
  },
  normalizeResult(result, input) {
    const data = storedData(result);
    checkHeader(data, { catalog: SPOT_CATALOG, scenario: SPOT_SCENARIO, flop: (input as { board: string }).board });
    return { data };
  },
};

/** Header of a stored spot of kind `spot` (CATALOG_VERSION and SCENARIO in kinds/spot.rs). */
export const SPOT_CATALOG = 0;
export const SPOT_SCENARIO = "spot";

function storedData(result: unknown): string {
  const r = record(result, "result", ["data"]);
  const data = r.data;
  if (typeof data !== "string" || data.length === 0 || data.length > POSTFLOP_MAX_BASE64) {
    throw new InvalidError(`data must be base64 of at most ${POSTFLOP_MAX_BASE64} characters`);
  }
  if (data.length % 4 !== 0 || !BASE64_RE.test(data)) throw new InvalidError("data must be base64");
  return data;
}

/** Magic "RSLS", format 1, catalog u16, scenario (u8 length + ASCII) and flop must match the input. */
function checkHeader(data: string, input: { catalog: number; scenario: string; flop: string }): void {
  const want = 4 + 1 + 2 + 1 + input.scenario.length + 6;
  let b: Uint8Array;
  try {
    b = Uint8Array.from(atob(data.slice(0, Math.ceil(want / 3) * 4)), (c) => c.charCodeAt(0));
  } catch {
    throw new InvalidError("data must be base64");
  }
  const text = (from: number, len: number) => String.fromCharCode(...b.subarray(from, from + len));
  const ok = b.length >= want && text(0, 4) === "RSLS" && b[4] === 1 && (b[5] | (b[6] << 8)) === input.catalog &&
    b[7] === input.scenario.length && text(8, b[7]) === input.scenario && text(8 + b[7], 6) === input.flop;
  if (!ok) throw new InvalidError("data is not a stored spot for this input");
}

export const KINDS: Readonly<Record<string, KindSpec>> = {
  "pushfold-hu": pushfoldHu,
  "pushfold-mw": pushfoldMw,
  postflop,
  spot,
};

export function kindSpec(kind: string): KindSpec | undefined {
  return Object.hasOwn(KINDS, kind) ? KINDS[kind] : undefined;
}
