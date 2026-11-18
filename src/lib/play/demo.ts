// Browser stand-in for the practice: two scripted hands that look like real ones (6-max, 40 BB
// with a 3-bet after your open, to the river, and a 10 BB push/fold spot), so every state of
// the page can be seen without Tauri. Calling the 3-bet leaves the prepared line: the flop is
// "solved live" for a few seconds, and cancelling plays the hand on without grades.
import { CLASSES, classCombos, classLabel, comboClass } from "../solver/range";
import type { SolverNode } from "../solver/types";
import type {
  Choice,
  FreeInfo,
  Grade,
  Grid,
  HandView,
  Lesson,
  LiveStatus,
  OpenedSpot,
  PastHand,
  PlayConfig,
  PlayEvent,
  PoolStatus,
  Snapshot,
  SolveWhy,
  Source,
  Street,
  Unrated,
  Verdict,
} from "./types";

type Decide = {
  street: Street;
  choices: Choice[];
  strategy: number[];
  evs: number[];
  equity: number | null;
  source: Source;
  /** What happens after the hero's choice: the opponents' moves and cards. */
  after: (chosen: number) => Step[];
};
type Step =
  | { kind: "act"; seat: number; choice: Choice }
  | { kind: "board"; street: Street; cards: string[] }
  | { kind: "decide"; d: Decide }
  | { kind: "solve"; why: SolveWhy }
  | { kind: "end"; winners: number[]; shown: number[]; made: (string | null)[]; reason: "folded" | "showdown" };

const c = (kind: Choice["kind"], to_bb: number, add_bb: number, pct_pot: number | null = null): Choice => ({ kind, to_bb, add_bb, pct_pot });

/** A plausible strategy per class: stronger classes take the aggressive choice more often. */
function strength(i: number): number {
  const r = Math.floor(i / 13);
  const col = i % 13;
  const hi = Math.min(r, col);
  const lo = Math.max(r, col);
  const pair = r === col;
  const suited = r < col;
  return Math.max(0, Math.min(1, (pair ? 1.15 - lo * 0.05 : 1 - hi * 0.07 - lo * 0.045) + (suited ? 0.12 : 0)));
}

function grid(n: number, aggressive: number, cut: number, reach = (_: number) => 1): Grid {
  const strategy: number[][] = [];
  const r: number[] = [];
  for (let i = 0; i < CLASSES; i++) {
    const s = strength(i);
    const play = Math.max(0, Math.min(1, (s - cut) * 4));
    const row = new Array(n).fill(0);
    row[aggressive] = play;
    row[0] = 1 - play;
    strategy.push(row);
    r.push(reach(i));
  }
  return { strategy, reach: r };
}

function postNode(street: "flop" | "turn" | "river", board: string[], pot: number, choices: Choice[], strategy: number[], evs: number[], equity: number, hero: string[], facing: number): SolverNode {
  const k = choices.length;
  const classes = [];
  for (let i = 0; i < CLASSES; i++) {
    const s = strength(i);
    if (s < 0.35) continue;
    const st = new Array(k).fill(0);
    const agg = choices.findIndex((x) => x.kind === "bet" || x.kind === "raise");
    const pass = choices.findIndex((x) => x.kind === "check" || x.kind === "call");
    const v = Math.max(0, Math.min(1, (s - 0.5) * 2.2));
    if (agg >= 0) st[agg] = v * 0.8;
    if (pass >= 0) st[pass] = 1 - v * 0.8 - (choices[0].kind === "fold" ? Math.max(0, 0.6 - s) : 0);
    if (choices[0].kind === "fold") st[0] = Math.max(0, 1 - st.reduce((a, b) => a + b, 0));
    const tot = st.reduce((a, b) => a + b, 0) || 1;
    classes.push({ class: i, label: classLabel(i), combos: classCombos(i) * Math.min(1, s + 0.2), strategy: st.map((x) => x / tot), equity: s * 0.8, ev: s * pot * 0.6 });
  }
  const heroClass = comboClass(hero[0], hero[1]);
  return {
    path: [],
    kind: "action",
    street,
    board,
    pot,
    bets: [0, 0],
    stacks: [36, 36],
    street_bets: [facing, 0],
    player: "ip",
    actions: choices.map((x, i) => ({ kind: x.kind === "limp" ? "call" : x.kind, amount: x.to_bb, frequency: strategy[i] * 0.9 + 0.1 / k, combos: 10 })),
    cards: [],
    hands: [{ combo: hero.join(""), class: heroClass, weight: 1, strategy, equity, ev: Math.max(...evs), evs }],
    classes,
    players: [
      { combos: 300, equity: 0.45, ev: pot * 0.4 },
      { combos: 280, equity: 0.55, ev: pot * 0.6 },
    ],
  } as SolverNode;
}

const HERO_A = ["As", "Js"];
const FLOP = ["6h", "9h", "Jd"];

/**
 * A hand like one a player reported: you open from the HJ, the CO 3-bets, everyone else
 * folds; on the flop you check, the CO bets, and the hand goes on to the river. With `live`
 * the prepared spot was another line, so the flop is solved while you wait.
 */
function handA(live: boolean): { seat: number; cards: string[][]; stack: number; ante: number[]; script: Step[] } {
  const post: Source = live ? "live" : "solver";
  const showdown: Step = { kind: "end", winners: [1], shown: [2], made: [null, "Paar", "Paar", null, null, null], reason: "showdown" };
  const river: Decide = {
    street: "river",
    choices: [c("check", 0, 0), c("bet", 12.5, 12.5, 50), c("all_in", 24.3, 24.3, 97)],
    strategy: [0.55, 0.4, 0.05],
    evs: [11.2, 11.15, 9.9],
    equity: 0.66,
    source: post,
    after: (k) => (k === 0 ? [{ kind: "act", seat: 2, choice: c("check", 0, 0) }, showdown] : [{ kind: "act", seat: 2, choice: c("call", k === 1 ? 12.5 : 24.3, k === 1 ? 12.5 : 24.3) }, showdown]),
  };
  const turn: Decide = {
    street: "turn",
    choices: [c("check", 0, 0), c("bet", 16.3, 16.3, 66), c("all_in", 24.3, 24.3, 98)],
    strategy: [0.7, 0.3, 0],
    evs: [10.4, 10.1, 7.6],
    equity: 0.68,
    source: post,
    after: (k) => [
      ...(k === 0 ? [{ kind: "act", seat: 2, choice: c("check", 0, 0) } as Step] : [{ kind: "act", seat: 2, choice: c("call", 16.3, 16.3) } as Step]),
      { kind: "board", street: "river", cards: ["3s"] },
      { kind: "decide", d: river },
    ],
  };
  const facing: Decide = {
    street: "flop",
    choices: [c("fold", 0, 0), c("call", 4.82, 4.82), c("all_in", 33.95, 33.95, 140)],
    strategy: [0, 0.001, 0.999],
    evs: [0, 9.54, 10.01],
    equity: 0.71,
    source: post,
    after: (k) =>
      k === 0
        ? [{ kind: "end", winners: [2], shown: [], made: [], reason: "folded" }]
        : k === 2
          ? [{ kind: "act", seat: 2, choice: c("fold", 4.82, 0) }, { kind: "end", winners: [1], shown: [], made: [], reason: "folded" }]
          : [{ kind: "board", street: "turn", cards: ["2c"] }, { kind: "decide", d: turn }],
  };
  const flop: Decide = {
    street: "flop",
    choices: [c("check", 0, 0), c("bet", 4.82, 4.82, 33), c("all_in", 33.95, 33.95, 233)],
    strategy: [0.78, 0.22, 0],
    evs: [9.3, 9.25, 6.1],
    equity: 0.71,
    source: post,
    after: (k) =>
      k === 0
        ? [{ kind: "act", seat: 2, choice: c("bet", 4.82, 4.82, 33) }, { kind: "decide", d: facing }]
        : [{ kind: "act", seat: 2, choice: c("call", 4.82, 4.82) }, { kind: "board", street: "turn", cards: ["2c"] }, { kind: "decide", d: turn }],
  };
  const threeBet: Decide = {
    street: "preflop",
    choices: [c("fold", 2.2, 0), c("call", 6.05, 3.85), c("raise", 13.3, 11.1, 92), c("all_in", 40, 37.8, 300)],
    strategy: [0, 0.62, 0.3, 0.08],
    evs: [0, 2.11, 2.1, 1.6],
    equity: null,
    source: "model",
    after: (k) =>
      k === 0
        ? [{ kind: "end", winners: [2], shown: [], made: [], reason: "folded" }]
        : k === 1
          ? [{ kind: "board", street: "flop", cards: FLOP }, ...(live ? [{ kind: "solve", why: "new_line" } as Step] : []), { kind: "decide", d: flop }]
          : [{ kind: "act", seat: 2, choice: c("fold", 6.05, 0) }, { kind: "end", winners: [1], shown: [], made: [], reason: "folded" }],
  };
  const pre: Decide = {
    street: "preflop",
    choices: [c("fold", 0, 0), c("raise", 2.2, 2.2, 34), c("all_in", 40, 40, 1114)],
    strategy: [0, 1, 0],
    evs: [0, 1.35, 0.6],
    equity: null,
    source: "model",
    after: (k) =>
      k === 0
        ? [{ kind: "end", winners: [5], shown: [], made: [], reason: "folded" }]
        : [
            { kind: "act", seat: 2, choice: c("raise", 6.05, 6.05, 81) },
            { kind: "act", seat: 3, choice: c("fold", 0, 0) },
            { kind: "act", seat: 4, choice: c("fold", 0.5, 0) },
            { kind: "act", seat: 5, choice: c("fold", 1, 0) },
            { kind: "decide", d: threeBet },
          ],
  };
  return {
    seat: 1,
    stack: 40,
    ante: [0, 0, 0, 0, 0, 1],
    cards: [["8s", "3d"], HERO_A, ["Td", "Th"], ["9c", "4c"], ["7s", "2d"], ["Kc", "5h"]],
    script: [{ kind: "act", seat: 0, choice: c("fold", 0, 0) }, { kind: "decide", d: pre }],
  };
}

function handB(): { seat: number; cards: string[][]; stack: number; ante: number[]; script: Step[] } {
  const pre: Decide = {
    street: "preflop",
    choices: [c("fold", 0.5, 0), c("all_in", 10, 9.5, 350)],
    strategy: [0.12, 0.88],
    evs: [-0.5, 0.82],
    equity: null,
    source: "equilibrium",
    after: (k) =>
      k === 0
        ? [{ kind: "end", winners: [5], shown: [], made: [], reason: "folded" }]
        : [
            { kind: "act", seat: 5, choice: c("call", 9, 8) },
            { kind: "board", street: "flop", cards: ["Ks", "8d", "3c"] },
            { kind: "board", street: "turn", cards: ["5h"] },
            { kind: "board", street: "river", cards: ["Tc"] },
            { kind: "end", winners: [4], shown: [5], made: [null, null, null, null, "Paar", "Paar"], reason: "showdown" },
          ],
  };
  return {
    seat: 4,
    stack: 10,
    ante: [0, 0, 0, 0, 0, 1],
    cards: [["9d", "2s"], ["Jh", "4d"], ["Qc", "3s"], ["6d", "6s"], ["Ad", "5s"], ["Kd", "Jd"]],
    script: [
      { kind: "act", seat: 0, choice: c("fold", 0, 0) },
      { kind: "act", seat: 1, choice: c("fold", 0, 0) },
      { kind: "act", seat: 2, choice: c("fold", 0, 0) },
      { kind: "act", seat: 3, choice: c("fold", 0, 0) },
      { kind: "decide", d: pre },
    ],
  };
}

// --- A tiny engine that plays a script into hand views ---------------------------------------

type State = {
  config: PlayConfig;
  seed: number;
  hand: ReturnType<typeof handA>;
  queue: Step[];
  view: HandView;
  put: number[];
  streetPut: number[];
  folded: boolean[];
  board: string[];
  street: Street;
  pending: Decide | null;
  hinted: boolean;
  /** Played on without a solution from here. */
  unrated: Unrated | null;
};

let state: State | null = null;
let count = 0;

const POS6 = ["LJ", "HJ", "CO", "BTN", "SB", "BB"];

function snapshot(s: State, toAct: number | null): number {
  const snap: Snapshot = {
    street: s.street,
    board: [...s.board],
    bets: [...s.streetPut],
    stacks: s.put.map((p, i) => s.hand.stack - s.hand.ante[i] - p),
    pot: s.hand.ante.reduce((a, b) => a + b, 0) + s.put.reduce((a, b) => a + b, 0),
    folded: [...s.folded],
    to_act: toAct,
  };
  s.view.snapshots.push(snap);
  return s.view.snapshots.length - 1;
}

function lessonOf(s: State, d: Decide): Lesson {
  const snap = s.view.snapshots[s.view.snapshots.length - 1];
  const top = Math.max(...s.streetPut);
  const toCall = Math.max(0, top - s.streetPut[s.hand.seat]);
  const cls = comboClass(s.hand.cards[s.hand.seat][0], s.hand.cards[s.hand.seat][1]);
  if (s.unrated) {
    const none = { street: d.street, choices: d.choices, source: null, strategy: null, unrated: s.unrated, evs: null, pot: snap.pot, to_call: toCall, equity: null };
    return { ...none, class: classLabel(cls), node: null, grid: null, path: null, exploitability_pct: null };
  }
  const isPost = d.street !== "preflop";
  return {
    street: d.street,
    choices: d.choices,
    source: d.source,
    strategy: d.strategy,
    unrated: null,
    evs: d.evs,
    pot: snap.pot,
    to_call: toCall,
    equity: d.equity,
    class: classLabel(cls),
    node: isPost ? postNode(d.street as "flop", s.board, snap.pot, d.choices, d.strategy, d.evs, d.equity ?? 0.5, s.hand.cards[s.hand.seat], toCall) : null,
    grid: isPost ? null : d.source === "equilibrium" ? grid(2, 1, 0.42) : grid(d.choices.length, 1, 0.5),
    path: isPost ? [] : null,
    exploitability_pct: isPost ? (d.source === "live" ? 1.8 : 0.8) : null,
  };
}

function judge(l: Lesson, chosen: number): Verdict {
  if (!l.strategy) return { ...l, chosen, judgement: null, hinted: false, points: null };
  const evs = l.evs ?? [];
  const best = evs.indexOf(Math.max(...evs));
  const loss = Math.max(0, evs[best] - evs[chosen]);
  const pct = (loss / l.pot) * 100;
  const s = l.strategy ?? [];
  const f = s[chosen] ?? 0;
  const main = s.indexOf(Math.max(...s));
  const isMain = chosen === main || f >= 0.5;
  // Mirrors crates/ringside-play/src/grade.rs.
  const grade: Grade =
    pct <= 0.5 && isMain && f > 0 ? "best" : pct <= 0.5 && f >= 0.1 ? "mix" : pct <= 2 ? "good" : pct <= 6 ? "inaccurate" : pct <= 15 ? "mistake" : "blunder";
  const points = { best: 100, mix: 100, good: 70, inaccurate: 30, mistake: 0, blunder: 0 }[grade];
  return { ...l, chosen, judgement: { grade, best, main, frequency: f, loss_bb: loss, loss_pct: pct }, hinted: false, points };
}

function apply(s: State, choice: Choice, seat: number, verdict: number | null) {
  if (choice.kind === "fold") s.folded[seat] = true;
  else if (choice.kind !== "check") {
    s.put[seat] += choice.add_bb;
    s.streetPut[seat] = choice.to_bb;
  }
  const next = s.queue[0];
  const snap = snapshot(s, next?.kind === "act" ? next.seat : next?.kind === "decide" ? s.hand.seat : null);
  s.view.events.push({ type: "action", street: s.street, seat, choice, verdict, snapshot: snap });
}

function run(s: State) {
  while (s.queue.length) {
    const step = s.queue.shift()!;
    if (step.kind === "act") apply(s, step.choice, step.seat, null);
    else if (step.kind === "board") {
      s.board.push(...step.cards);
      s.street = step.street;
      s.streetPut = s.streetPut.map(() => 0);
      const snap = snapshot(s, null);
      s.view.events.push({ type: "board", street: step.street, cards: step.cards, snapshot: snap });
    } else if (step.kind === "decide") {
      s.pending = step.d;
      s.view.snapshots[s.view.snapshots.length - 1].to_act = s.hand.seat;
      const top = Math.max(...s.streetPut);
      s.view.decision = {
        street: step.d.street,
        choices: step.d.choices,
        pot: s.view.snapshots[s.view.snapshots.length - 1].pot,
        to_call: Math.max(0, top - s.streetPut[s.hand.seat]),
        hinted: s.hinted,
        graded: !s.unrated,
      };
      break;
    } else if (step.kind === "solve") {
      const pot = s.view.snapshots[s.view.snapshots.length - 1].pot;
      s.view.solving = { why: step.why, street: s.street, oop: 1, ip: 2, pot, stack: 33.95, target_pct: 2 };
      solveLive(s);
      break;
    } else {
      const pot = s.hand.ante.reduce((a, b) => a + b, 0) + s.put.reduce((a, b) => a + b, 0);
      s.view.outcome = {
        reason: step.reason,
        winners: step.winners,
        net: s.put.map((p, i) => (step.winners.includes(i) ? pot / step.winners.length : 0) - p - s.hand.ante[i]),
        shown: s.hand.cards.map((cards, i) => (step.shown.includes(i) ? cards : null)),
        made: step.made.length ? step.made : s.hand.cards.map(() => null),
      };
      s.view.board = [...s.board];
      s.view.decision = null;
    }
  }
  s.view.board = [...s.board];
}

// --- A live solve that takes a few seconds -------------------------------------------------

const RULE =
  "Die Gegner spielen hier nach einer einfachen Regel: Sie setzen und erhöhen mit starken Händen, callen, wenn ihre Gewinnchance gegen zufällige Hände den Preis deckt, und folden sonst.";
const SOLVE_STEPS = 10;
const SOLVE_STEP_MS = 280;

let timer: ReturnType<typeof setInterval> | null = null;
const liveHandlers = new Set<(s: LiveStatus) => void>();
const handHandlers = new Set<(v: HandView) => void>();

const liveStatusOf = (status: LiveStatus["status"], fraction: number): LiveStatus => ({
  urgency: "now",
  status,
  fraction,
  secs_left: status === "running" ? Math.max(1, ((1 - fraction) * SOLVE_STEPS * SOLVE_STEP_MS) / 1000) : null,
  target_pct: 2,
  error: null,
});
let lastLive: LiveStatus | null = null;

function emitLive(status: LiveStatus) {
  lastLive = status;
  for (const h of liveHandlers) h(status);
}

function stopSolve() {
  if (timer) clearInterval(timer);
  timer = null;
}

function solveLive(s: State) {
  stopSolve();
  let k = 0;
  emitLive(liveStatusOf("running", 0));
  timer = setInterval(() => {
    k += 1;
    if (k < SOLVE_STEPS) return emitLive(liveStatusOf("running", k / SOLVE_STEPS));
    stopSolve();
    s.view.solving = null;
    s.view.spot = { id: 2, source: "live", complete: true, exploitability_pct: 1.8, oop: 1, ip: 2, pot: 14.6, stack: 33.95 };
    emitLive(liveStatusOf("done", 1));
    run(s);
    const v = structuredClone(s.view);
    for (const h of handHandlers) h(v);
  }, SOLVE_STEP_MS);
}

export function onLive(handler: (s: LiveStatus) => void) {
  liveHandlers.add(handler);
  return Promise.resolve(() => void liveHandlers.delete(handler));
}

export function onHand(handler: (v: HandView) => void) {
  handHandlers.add(handler);
  return Promise.resolve(() => void handHandlers.delete(handler));
}

export function liveStatus(): LiveStatus | null {
  return state?.view.solving ? lastLive : null;
}

export function liveCancel(): HandView | null {
  const s = state;
  if (!s?.view.solving) return s ? structuredClone(s.view) : null;
  stopSolve();
  s.view.solving = null;
  s.unrated = "no_solve";
  const free: FreeInfo = { why: "no_solve", street: s.street, rule: RULE, note: "Abgebrochen" };
  s.view.free = free;
  emitLive(liveStatusOf("cancelled", 0));
  run(s);
  return structuredClone(s.view);
}

function start(config: PlayConfig, seed: number): HandView {
  stopSolve();
  const hand = config.mode === "push_fold" ? handB() : handA(config.mode === "full");
  const n = 6;
  const put = new Array(n).fill(0);
  put[n - 2] = 0.5;
  put[n - 1] = 1;
  const view: HandView = {
    seed,
    config,
    players: n,
    positions: POS6,
    stack_bb: hand.stack,
    ante: hand.ante,
    hero: hand.seat,
    hero_cards: hand.cards[hand.seat],
    board: [],
    events: [],
    snapshots: [],
    verdicts: [],
    decision: null,
    outcome: null,
    preflop_source: config.mode === "push_fold" ? "equilibrium" : "model",
    spot: config.mode === "postflop" ? { id: 1, source: "solver", complete: true, exploitability_pct: 0.8, oop: 1, ip: 2, pot: 14.6, stack: 33.95 } : null,
    waiting: false,
    solving: null,
    free: null,
  };
  state = {
    config,
    seed,
    hand,
    queue: [...hand.script],
    view,
    put,
    streetPut: [...put],
    folded: new Array(n).fill(false),
    board: [],
    street: "preflop",
    pending: null,
    hinted: false,
    unrated: null,
  };
  snapshot(state, 0);
  if (config.mode === "postflop") {
    // Postflop only: the hero's preflop raise is played automatically.
    run(state);
    const pre = state.pending!;
    state.pending = null;
    state.queue = pre.after(1);
    apply(state, pre.choices[1], hand.seat, null);
  }
  run(state);
  return structuredClone(state.view);
}

export function prepare(_config: PlayConfig): PoolStatus {
  return poolStatus();
}

export function next(config: PlayConfig, seed: number | null): HandView {
  count += 1;
  const alt = { ...config, mode: count % 3 === 0 && config.mode === "full" ? ("push_fold" as const) : config.mode };
  return start(alt, seed ?? 1000 + count);
}

export function repeat(): HandView {
  if (!state) throw new Error("Noch keine Hand gespielt.");
  return start(state.config, state.seed);
}

export function current(): HandView | null {
  return state ? structuredClone(state.view) : null;
}

export function act(choice: number): HandView {
  const s = state;
  if (!s || !s.pending) throw new Error("Du bist gerade nicht am Zug.");
  const d = s.pending;
  const v = judge(lessonOf(s, d), choice);
  if (s.hinted) {
    v.hinted = true;
    v.points = v.points === null ? null : v.points / 2;
  }
  s.hinted = false;
  s.view.verdicts.push(v);
  s.pending = null;
  s.view.decision = null;
  s.queue = d.after(choice);
  apply(s, d.choices[choice], s.hand.seat, s.view.verdicts.length - 1);
  run(s);
  return structuredClone(s.view);
}

export function hint(): Lesson {
  const s = state;
  if (!s || !s.pending) throw new Error("Du bist gerade nicht am Zug.");
  s.hinted = true;
  if (s.view.decision) s.view.decision.hinted = true;
  return lessonOf(s, s.pending);
}

export function poolStatus(): PoolStatus {
  return { status: "running", title: "Bereite 2 Spots vor …", ready: 1, target: 3, fraction: 0.46, eta_secs: 48, next_secs: 17, solve_secs: 24, solved: 3, error: null };
}

export function poolCancel(): void {}

export function openSolver(): OpenedSpot {
  throw new Error("Im Browser-Demo gibt es keinen Solver.");
}

export function history(_limit: number): PastHand[] {
  return [];
}

export type { PlayEvent };
