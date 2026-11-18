// Browser preview only: a made-up solve with plausible numbers, so the page can be designed
// and screenshotted without the Rust backend.
import { RANKS, classCombos, classLabel, parseRange } from "./range";
import type {
  SolverAction,
  SolverActionKind,
  SolverClass,
  SolverDone,
  SolverEstimate,
  SolverHand,
  SolverNode,
  SolverProgress,
  SolverSettings,
  SolverSpot,
  SolverStep,
} from "./types";

const SUITS = "cdhs";

let spot: SolverSpot | null = null;
let current = 0;
let timer: ReturnType<typeof setInterval> | null = null;
const progressListeners = new Set<(p: SolverProgress) => void>();
const doneListeners = new Set<(d: SolverDone) => void>();

export function onProgress(f: (p: SolverProgress) => void): () => void {
  progressListeners.add(f);
  return () => void progressListeners.delete(f);
}

export function onDone(f: (d: SolverDone) => void): () => void {
  doneListeners.add(f);
  return () => void doneListeners.delete(f);
}

function cards(board: string): string[] {
  return board.replace(/[^AKQJT2-9cdhs]/gi, "").match(/.{2}/g) ?? [];
}

export function demoEstimate(s: SolverSpot, threads: number): SolverEstimate {
  const w = (t: string) => (parseRange(t) ?? []).reduce((sum, x, i) => sum + x * classCombos(i), 0);
  const combos: [number, number] = [w(s.oop_range), w(s.ip_range)];
  const street = cards(s.board).length;
  const branches = s.sizes.slice(street - 3).reduce((p, z) => p * (1 + z.bet.length) * (1 + z.raise.length), 1);
  const cardsLeft = street === 3 ? 49 * 48 : street === 4 ? 48 : 1;
  // Scaled to the real solver: BTN against BB at 40 BB with the default sizes needs about 3 GB.
  const memory = Math.round(combos[0] * combos[1] * branches * cardsLeft * 0.045 + 2 ** 20);
  const limit = 4 * 2 ** 30;
  const compressed = memory > limit && memory / 2 <= limit;
  const bytes = compressed ? memory / 2 : memory;
  return {
    memory_bytes: bytes,
    compressed,
    memory_limit: limit,
    fits: bytes <= limit,
    combos,
    seconds: ((bytes / 2 ** 30) * 1.1 * (street === 3 ? 250 : street === 4 ? 180 : 120)) / Math.max(1, threads) ** 0.8,
  };
}

function stop() {
  if (timer) clearInterval(timer);
  timer = null;
}

/** Shows `s` as solved without a run, as a spot opened from the library. */
export function demoShow(s: SolverSpot) {
  stop();
  spot = s;
}

/** Preview only: `?solve=hold` keeps a solve running half-way, for pictures of the progress. */
const solveParam = typeof window !== "undefined" ? new URLSearchParams(window.location.search).get("solve") : null;
const hold = solveParam === "hold";
/** `?solve=1` finishes at once, so pictures of the result never catch the progress. */
const instant = solveParam === "1";

export function demoStart(s: SolverSpot, settings: SolverSettings, id: number) {
  stop();
  spot = s;
  current = id;
  let iteration = 0;
  const started = Date.now();
  const emit = (it: number) => {
    const pct = Math.max(settings.target_pct * 0.92, 70 * Math.exp(-it / 38) + 0.25 * Math.exp(-it / 400));
    const p = {
      run: id,
      iteration: it,
      max_iterations: settings.max_iterations,
      exploitability: (pct / 100) * s.pot,
      exploitability_pct: pct,
      elapsed_ms: Date.now() - started,
    };
    progressListeners.forEach((f) => f(p));
    return p;
  };
  emit(0);
  timer = setInterval(() => {
    if (hold && iteration >= 60) return;
    iteration += instant ? 400 : 10;
    const p = emit(iteration);
    if (p.exploitability_pct <= settings.target_pct || iteration >= settings.max_iterations) {
      stop();
      const done: SolverDone = {
        status: "solved",
        run: id,
        saved: 1,
        reused: false,
        summary: {
          iterations: iteration,
          exploitability: p.exploitability,
          exploitability_pct: p.exploitability_pct,
          converged: p.exploitability_pct <= settings.target_pct,
          elapsed_ms: p.elapsed_ms,
          memory_bytes: demoEstimate(s, settings.threads).memory_bytes,
          compressed: false,
          threads: settings.threads,
        },
      };
      doneListeners.forEach((f) => f(done));
    }
  }, 90);
}

export function demoCancel() {
  if (!timer) return;
  stop();
  const done: SolverDone = { status: "cancelled", run: current, locked: false };
  doneListeners.forEach((f) => f(done));
}

export function demoClear() {
  spot = null;
}

/** Hand strength 0–1 on `board`, by pairs with the board and high cards. */
function strength(label: string, board: string[]): number {
  const ranks = board.map((c) => RANKS.indexOf(c[0]));
  const hi = RANKS.indexOf(label[0]);
  const lo = RANKS.indexOf(label[1]);
  const top = Math.min(...ranks);
  if (hi === lo) {
    if (ranks.includes(hi)) return 0.97;
    return hi < top ? 0.8 - hi * 0.01 : 0.35 + (12 - hi) * 0.02;
  }
  const hits = [hi, lo].filter((r) => ranks.includes(r));
  if (hits.length === 2) return 0.92;
  if (hits.length === 1) {
    const pairRank = hits[0];
    const kicker = pairRank === hi ? lo : hi;
    return (pairRank === top ? 0.7 : 0.5) + (12 - kicker) * 0.01;
  }
  return 0.05 + ((12 - hi) + (12 - lo)) / 70 + (label.endsWith("s") ? 0.06 : 0);
}

function combosOf(label: string, dead: Set<string>): string[] {
  const [a, b] = [label[0], label[1]];
  const out: string[] = [];
  for (let i = 0; i < 4; i++) {
    for (let j = 0; j < 4; j++) {
      if (a === b && j <= i) continue;
      const suited = i === j;
      if (a !== b && (label.endsWith("s") ? !suited : suited)) continue;
      const x = a + SUITS[i];
      const y = b + SUITS[j];
      if (!dead.has(x) && !dead.has(y)) out.push(x + y);
    }
  }
  return out;
}

type State = {
  street: number;
  board: string[];
  pot: number;
  /** Put in since the root, and on the current street. */
  bets: [number, number];
  put: [number, number];
  toAct: 0 | 1;
  raised: boolean;
  checks: number;
  over: boolean;
  chance: boolean;
};

type Act = { kind: SolverActionKind; amount: number };

function round(x: number): number {
  return Math.round(x * 100) / 100;
}

function actionsAt(s: SolverSpot, st: State): Act[] {
  const sizes = s.sizes[st.street - 3];
  const p = st.toAct;
  const allIn = st.put[p] + s.stack - st.bets[p];
  const toCall = st.put[1 - p] - st.put[p];
  if (toCall > 0) {
    const out: Act[] = [
      { kind: "fold", amount: 0 },
      { kind: "call", amount: 0 },
    ];
    if (!st.raised && sizes.raise.length && allIn > st.put[1 - p]) {
      const to = Math.min(st.put[1 - p] + (sizes.raise[0] / 100) * (st.pot + toCall), allIn);
      out.push({ kind: to >= allIn ? "all_in" : "raise", amount: round(to) });
    }
    return out;
  }
  const out: Act[] = [{ kind: "check", amount: 0 }];
  for (const pct of sizes.bet) {
    const amount = Math.min((pct / 100) * st.pot, allIn);
    out.push({ kind: amount >= allIn ? "all_in" : "bet", amount: round(amount) });
  }
  return out;
}

function walk(s: SolverSpot, path: SolverStep[]): State {
  const board = cards(s.board);
  const st: State = {
    street: board.length,
    board,
    pot: s.pot,
    bets: [0, 0],
    put: [0, 0],
    toAct: 0,
    raised: false,
    checks: 0,
    over: false,
    chance: false,
  };
  const endStreet = () => {
    st.put = [0, 0];
    st.raised = false;
    st.checks = 0;
    if (st.street === 5) st.over = true;
    else st.chance = true;
  };
  const add = (p: 0 | 1, to: number) => {
    const extra = to - st.put[p];
    st.put[p] = to;
    st.bets[p] += extra;
    st.pot += extra;
  };
  for (const step of path) {
    if ("card" in step) {
      st.board = [...st.board, step.card];
      st.street++;
      st.chance = false;
      st.toAct = 0;
      continue;
    }
    const a = actionsAt(s, st)[step.action];
    const p = st.toAct;
    if (a.kind === "fold") st.over = true;
    else if (a.kind === "call") {
      add(p, st.put[1 - p]);
      endStreet();
    } else if (a.kind === "check") {
      st.checks++;
      if (st.checks === 2) endStreet();
      else st.toAct = 1;
    } else {
      st.raised = st.put[1 - p] > 0;
      add(p, a.amount);
      st.toAct = p === 0 ? 1 : 0;
    }
  }
  return st;
}

function strategyFor(kinds: SolverActionKind[], x: number, depth: number): number[] {
  const raw = kinds.map((k, i) => {
    if (k === "fold") return Math.max(0, 0.55 - x) * 2.2;
    if (k === "call") return 0.25 + x * 0.8 - Math.abs(x - 0.55);
    if (k === "check") return 0.55 + (0.5 - Math.abs(x - 0.45)) * 1.4 - depth * 0.05;
    const big = i === kinds.length - 1;
    const value = Math.max(0, x - (big ? 0.72 : 0.6)) * (big ? 5 : 3);
    const bluff = x < 0.2 ? (big ? 0.35 : 0.25) * (0.2 - x) * 5 : 0;
    return value + bluff;
  });
  const sum = raw.reduce((a, b) => a + Math.max(0, b), 0) || 1;
  return raw.map((v) => Math.max(0, v) / sum);
}

export function demoNode(path: SolverStep[]): SolverNode {
  const s = spot;
  if (!s) throw new Error("Noch keine Lösung vorhanden.");
  const st = walk(s, path);
  const dead = new Set(st.board);
  const base = {
    path,
    street: (["flop", "turn", "river"] as const)[st.street - 3],
    board: st.board,
    pot: round(st.pot),
    bets: [round(st.bets[0]), round(st.bets[1])] as [number, number],
    street_bets: [round(st.put[0]), round(st.put[1])] as [number, number],
    stacks: [round(s.stack - st.bets[0]), round(s.stack - st.bets[1])] as [number, number],
    players: [
      { combos: 180, equity: 0.47, ev: round(st.pot * 0.44) },
      { combos: 220, equity: 0.53, ev: round(st.pot * 0.56) },
    ] as SolverNode["players"],
  };
  if (st.over) return { ...base, kind: "terminal", player: null, actions: [], cards: [], hands: [], classes: [] };
  if (st.chance) {
    const deck = RANKS.split("").flatMap((r) => SUITS.split("").map((x) => r + x));
    return { ...base, kind: "chance", player: null, actions: [], cards: deck.filter((c) => !dead.has(c)), hands: [], classes: [] };
  }

  const player = st.toAct;
  const weights = parseRange(player === 0 ? s.oop_range : s.ip_range) ?? [];
  const acts = actionsAt(s, st);
  const kinds = acts.map((a) => a.kind);
  const hands: SolverHand[] = [];
  const classes: SolverClass[] = [];
  weights.forEach((w, i) => {
    if (w <= 0) return;
    const label = classLabel(i);
    const x = strength(label, st.board);
    const reach = w * (path.length ? Math.min(1, 0.35 + x) : 1);
    const strategy = strategyFor(kinds, x, path.length);
    const combos = combosOf(label, dead);
    if (!combos.length) return;
    const equity = Math.min(0.98, Math.max(0.03, x * 0.95 + 0.03));
    const ev = round(equity * st.pot * 0.9 - (1 - equity) * (st.put[1 - player] - st.put[player]) * 0.5);
    const top = Math.max(...strategy);
    combos.forEach((combo, k) => {
      const jitter = ((k * 37) % 11) / 200 - 0.025;
      const own = round(ev + jitter * st.pot);
      // Actions the hand rarely takes cost more, roughly as in a real solve.
      const evs = kinds.map((kind, a) => (kind === "fold" ? 0 : round(own - (top - strategy[a]) * st.pot * 0.09)));
      hands.push({ combo, class: i, weight: reach, strategy, equity: Math.min(1, equity + jitter), ev: own, evs });
    });
    classes.push({ class: i, label, combos: reach * combos.length, strategy, equity, ev });
  });
  const total = classes.reduce((a, c) => a + c.combos, 0) || 1;
  const actions: SolverAction[] = acts.map((a, k) => {
    const combos = classes.reduce((sum, c) => sum + c.combos * c.strategy[k], 0);
    return { kind: a.kind, amount: a.amount, frequency: combos / total, combos };
  });
  return { ...base, kind: "action", player: player === 0 ? "oop" : "ip", actions, cards: [], hands, classes };
}
