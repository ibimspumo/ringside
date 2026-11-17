// Browser stand-in for the practice: two scripted hands that look like real ones (6-max, 40 BB to
// a showdown, and a 10 BB push/fold spot), so every state of the page can be seen without Tauri.
import { CLASSES, classCombos, classLabel, comboClass } from "../solver/range";
import type { SolverNode } from "../solver/types";
import type {
  Choice,
  Grade,
  Grid,
  HandView,
  Lesson,
  OpenedSpot,
  PastHand,
  PlayConfig,
  PlayEvent,
  PoolStatus,
  Snapshot,
  Source,
  Street,
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

const HERO_A = ["Kh", "Qh"];
const FLOP = ["Qd", "7c", "2h"];

function handA(): { seat: number; cards: string[][]; stack: number; ante: number[]; script: Step[] } {
  const river: Decide = {
    street: "river",
    choices: [c("fold", 0, 0), c("call", 7.35, 7.35), c("all_in", 26.5, 26.5, 85)],
    strategy: [0.04, 0.9, 0.06],
    evs: [0, 5.1, 3.4],
    equity: 0.71,
    source: "solver",
    after: (k) =>
      k === 0
        ? [{ kind: "end", winners: [5], shown: [], made: [], reason: "folded" }]
        : [
            ...(k === 2 ? [{ kind: "act", seat: 5, choice: c("call", 26.5, 19.15) } as Step] : []),
            { kind: "end", winners: [3], shown: [5], made: [null, null, null, "Paar", null, "Paar"], reason: "showdown" },
          ],
  };
  const turn: Decide = {
    street: "turn",
    choices: [c("check", 0, 0), c("bet", 7.35, 7.35, 75), c("all_in", 35.85, 35.85, 366)],
    strategy: [0.21, 0.79, 0],
    evs: [5.9, 6.45, 4.8],
    equity: 0.74,
    source: "solver",
    after: (k) => [
      ...(k === 0 ? [] : [{ kind: "act", seat: 5, choice: c("call", 7.35, 7.35) } as Step]),
      { kind: "board", street: "river", cards: ["4c"] },
      { kind: "act", seat: 5, choice: c("bet", 7.35, 7.35, 75) },
      { kind: "decide", d: river },
    ],
  };
  const flop: Decide = {
    street: "flop",
    choices: [c("check", 0, 0), c("bet", 1.95, 1.95, 33), c("all_in", 37.8, 37.8, 641)],
    strategy: [0.38, 0.62, 0],
    evs: [3.85, 3.92, 2.1],
    equity: 0.78,
    source: "solver",
    after: (k) => [
      ...(k === 1 ? [{ kind: "act", seat: 5, choice: c("call", 1.95, 1.95) } as Step] : []),
      { kind: "board", street: "turn", cards: ["9s"] },
      { kind: "act", seat: 5, choice: c("check", 0, 0) },
      { kind: "decide", d: turn },
    ],
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
            { kind: "act", seat: 4, choice: c("fold", 0.5, 0) },
            { kind: "act", seat: 5, choice: c("call", k === 2 ? 39 : 2.2, k === 2 ? 38 : 1.2) },
            { kind: "board", street: "flop", cards: FLOP },
            ...(k === 2
              ? ([{ kind: "board", street: "turn", cards: ["9s"] }, { kind: "board", street: "river", cards: ["4c"] }, { kind: "end", winners: [3], shown: [5], made: [null, null, null, "Paar", null, "Paar"], reason: "showdown" }] as Step[])
              : ([{ kind: "act", seat: 5, choice: c("check", 0, 0) }, { kind: "decide", d: flop }] as Step[])),
          ],
  };
  return {
    seat: 3,
    stack: 40,
    ante: [0, 0, 0, 0, 0, 1],
    cards: [["8s", "3d"], ["Td", "6c"], ["9c", "4h"], HERO_A, ["7s", "2d"], ["Jc", "Qs"]],
    script: [
      { kind: "act", seat: 0, choice: c("fold", 0, 0) },
      { kind: "act", seat: 1, choice: c("fold", 0, 0) },
      { kind: "act", seat: 2, choice: c("fold", 0, 0) },
      { kind: "decide", d: pre },
    ],
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
  const isPost = d.street !== "preflop";
  return {
    street: d.street,
    choices: d.choices,
    source: d.source,
    strategy: d.strategy,
    evs: d.evs,
    pot: snap.pot,
    to_call: toCall,
    equity: d.equity,
    class: classLabel(cls),
    node: isPost ? postNode(d.street as "flop", s.board, snap.pot, d.choices, d.strategy, d.evs, d.equity ?? 0.5, s.hand.cards[s.hand.seat], toCall) : null,
    grid: isPost ? null : d.source === "equilibrium" ? grid(2, 1, 0.42) : grid(d.choices.length, 1, 0.5),
    path: isPost ? [] : null,
    exploitability_pct: isPost ? 0.8 : null,
  };
}

function judge(l: Lesson, chosen: number): Verdict {
  const evs = l.evs ?? [];
  const best = evs.indexOf(Math.max(...evs));
  const loss = Math.max(0, evs[best] - evs[chosen]);
  const pct = (loss / l.pot) * 100;
  const f = l.strategy?.[chosen] ?? 0;
  const grade: Grade = pct <= 0.5 && f >= 0.1 ? "best" : pct <= 2 ? "good" : pct <= 6 ? "inaccurate" : pct <= 15 ? "mistake" : "blunder";
  const points = { best: 100, good: 70, inaccurate: 30, mistake: 0, blunder: 0 }[grade];
  return { ...l, chosen, judgement: { grade, best, frequency: f, loss_bb: loss, loss_pct: pct }, hinted: false, points };
}

function apply(s: State, choice: Choice, seat: number, verdict: number | null) {
  if (choice.kind === "fold") s.folded[seat] = true;
  else if (choice.kind !== "check") {
    s.put[seat] += choice.add_bb;
    s.streetPut[seat] = choice.to_bb;
  }
  const snap = snapshot(s, null);
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
      };
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

function start(config: PlayConfig, seed: number): HandView {
  const hand = config.mode === "push_fold" ? handB() : handA();
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
    spot: config.mode === "push_fold" ? null : { id: 1, source: "solver", complete: true, exploitability_pct: 0.8, oop: 5, ip: 3, pot: 5.9, stack: 36.8 },
    waiting: false,
  };
  state = { config, seed, hand, queue: [...hand.script], view, put, streetPut: [...put], folded: new Array(n).fill(false), board: [], street: "preflop", pending: null, hinted: false };
  snapshot(state, 0);
  if (config.mode === "postflop") {
    // Postflop only: the hero's preflop raise is played automatically.
    run(state);
    const pre = state.pending!;
    state.pending = null;
    apply(state, pre.choices[1], hand.seat, null);
    state.queue = pre.after(1);
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
  apply(s, d.choices[choice], s.hand.seat, s.view.verdicts.length - 1);
  s.queue = d.after(choice);
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
