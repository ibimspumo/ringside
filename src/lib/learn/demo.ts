// Browser-only stand-ins for the learn_* commands. They mimic crates/ringside-learn closely
// enough to design and click through the pages; the real grading lives in Rust.
import type {
  Action,
  Exercise,
  Fact,
  Feedback,
  Hand,
  Highlight,
  LearnAnswer,
  LearnKind,
  LearnProgress,
  OwnSpot,
  SkillProgress,
} from "../types";
import type { Decision, Rating } from "../analysis/types";
import { DEMO_DECISIONS } from "../analysis/demo-hands";
import { demoHandById } from "../demo";
import { parseRange } from "./catalog";
import type { DrillResult, DrillSpot, PlayProfile, SpotGroup } from "./profile";
import { bestFive, combosOf, deckWithout, matrixHand, score, categoryOf, CATEGORY } from "./wissen/poker";

const NAMES: Record<LearnKind, string> = {
  outs: "Outs zählen",
  pot_odds: "Pot Odds",
  probability: "Trefferwahrscheinlichkeit",
  combos: "Kombinatorik",
  position: "Positionen",
  showdown: "Showdown",
  equity_hand: "Equity Hand gegen Hand",
  equity_range: "Equity gegen eine Range",
  push_fold_sb: "Push/Fold im Small Blind",
  push_fold_bb: "Callen gegen einen Shove im Big Blind",
  push_fold_open: "Shove oder Fold, wenn alle zu dir gefoldet haben",
  push_fold_call: "Call gegen einen Shove",
  all_in: "All-ins eigener Hände",
  full_hands: "Ganze Hände",
  bounty_odds: "Pot Odds mit Kopfgeld",
  icm_call: "ICM und Bubble-Calls",
  open_fold: "Öffnen oder folden",
  open_fold_short: "Öffnen am kurzen Tisch",
  river_bet: "Bet oder Check am River",
  stack_depth: "Stacktiefe und M-Zahl",
  hud_read: "Gegner lesen mit HUD-Werten",
  own_decision: "Aus deinem Spiel",
};
const KINDS = Object.keys(NAMES) as LearnKind[];
const LEVEL_NAMES = ["neu", "kennengelernt", "mit Hilfe", "selbstständig", "bestätigt"] as const;

/* Formatting like crates/ringside-learn/src/fmt.rs ------------------------------------------------ */

const nf = (d: number) => new Intl.NumberFormat("de-DE", { minimumFractionDigits: d, maximumFractionDigits: d });
const pct = (x: number) => `${nf(1).format(x)} %`;
const bb = (x: number) => `${new Intl.NumberFormat("de-DE", { maximumFractionDigits: 2 }).format(x)} bb`;
const sbb = (x: number) => `${x > 0 ? "+" : x < 0 ? "−" : "±"}${bb(Math.abs(x))}`;
const fact = (label: string, value: string, formula: string | null = null): Fact => ({ label, value, formula });

/* Seeded random numbers --------------------------------------------------------------------------- */

function rng(seed: number) {
  let s = (seed % 2 ** 32) ^ 0x9e3779b9;
  const next = () => {
    s = (s + 0x6d2b79f5) | 0;
    let t = Math.imul(s ^ (s >>> 15), 1 | s);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
  return {
    next,
    int: (lo: number, hi: number) => lo + Math.floor(next() * (hi - lo + 1)),
    pick: <T>(xs: T[]): T => xs[Math.floor(next() * xs.length)],
    deal(used: Set<string>, n: number): string[] {
      const out: string[] = [];
      while (out.length < n) {
        const c = deckWithout([...used])[Math.floor(next() * (52 - used.size))];
        used.add(c);
        out.push(c);
      }
      return out;
    },
  };
}
type Rng = ReturnType<typeof rng>;

/* Poker helpers ----------------------------------------------------------------------------------- */

/** Monte Carlo equity in percent of `hero` against hands drawn by `villain`. */
function mcEquity(hero: string[], villain: (r: Rng, dead: Set<string>) => string[] | null, board: string[], seed: number, n = 1500): number {
  const r = rng(seed);
  let won = 0;
  let runs = 0;
  for (let i = 0; i < n * 3 && runs < n; i++) {
    const used = new Set([...hero, ...board]);
    const v = villain(r, used);
    if (!v) continue;
    v.forEach((c) => used.add(c));
    const b = [...board, ...r.deal(used, 5 - board.length)];
    const h = score([...hero, ...b]);
    const o = score([...v, ...b]);
    won += h > o ? 1 : h === o ? 0.5 : 0;
    runs++;
  }
  return runs ? (won / runs) * 100 : 0;
}

function classOf(cards: string[]): string {
  const R = "23456789TJQKA";
  const [a, b] = [...cards].sort((x, y) => R.indexOf(y[0]) - R.indexOf(x[0]));
  if (a[0] === b[0]) return a[0] + b[0];
  return a[0] + b[0] + (a[1] === b[1] ? "s" : "o");
}

/** Chen formula, a cheap stand-in for the Nash ordering of the 169 classes. */
function chen(cls: string): number {
  const V: Record<string, number> = { A: 10, K: 8, Q: 7, J: 6, T: 5 };
  const R = "23456789TJQKA";
  const val = (c: string) => V[c] ?? (R.indexOf(c) + 2) / 2;
  const [a, b] = [cls[0], cls[1]];
  let s = val(a);
  if (a === b) return Math.max(5, s * 2);
  if (cls[2] === "s") s += 2;
  const gap = R.indexOf(a) - R.indexOf(b) - 1;
  s -= [0, 1, 2, 4][Math.min(gap, 3)] + (gap >= 4 ? 1 : 0);
  if (gap <= 1 && R.indexOf(a) < R.indexOf("Q")) s += 1;
  return s;
}

/** Share of all combos (0–100) at least as strong as each class by the Chen ordering. */
const PERCENTILE: Map<string, number> = (() => {
  const all: string[] = [];
  for (let i = 0; i < 13; i++) for (let j = 0; j < 13; j++) all.push(matrixHand(i, j));
  all.sort((x, y) => chen(y) - chen(x));
  const out = new Map<string, number>();
  let acc = 0;
  for (const c of all) {
    acc += combosOf(c).length;
    out.set(c, (acc / 1326) * 100);
  }
  return out;
})();

/* Generators -------------------------------------------------------------------------------------- */

type Task = Omit<Exercise, "kind" | "level" | "seed" | "hand_id">;

function outsOf(hero: string[], villain: string[], board: string[]) {
  const outs: string[] = [];
  const half: string[] = [];
  const dirty: string[] = [];
  const before = categoryOf([...hero, ...board]);
  for (const c of deckWithout([...hero, ...villain, ...board])) {
    const h = score([...hero, ...board, c]);
    const v = score([...villain, ...board, c]);
    if (h > v) outs.push(c);
    else if (h === v) half.push(c);
    else if (categoryOf([...hero, ...board, c]) > before) dirty.push(c);
  }
  return { outs, half, dirty };
}

function genOuts(level: number, r: Rng): Task {
  for (;;) {
    const used = new Set<string>();
    const hero = r.deal(used, 2);
    const villain = r.deal(used, 2);
    const board = r.deal(used, level === 1 ? 3 : r.next() < 0.5 ? 3 : 4);
    if (score([...hero, ...board]) >= score([...villain, ...board])) continue;
    const o = outsOf(hero, villain, board);
    const n = o.outs.length;
    const ok = level === 1 ? n >= 4 && n <= 15 && !o.half.length : n >= 2 && n <= 20 && (level < 3 || o.dirty.length > 0);
    if (!ok) continue;
    const street = board.length === 3 ? "Turn" : "River";
    let q = `Du liegst hinten. Welche Karten bringen dich am ${street} nach vorn? Tippe alle Outs an.`;
    if (level >= 2) q += " Karten, die zu einem Split führen, sind halbe Outs.";
    if (level === 3) q += " Achtung: Nicht jede Karte, die deine Hand verbessert, gewinnt.";
    return {
      question: q,
      prompt: { type: "outs", hero, villain, board },
      answer: { type: "cards", from: deckWithout([...hero, ...villain, ...board]) },
      grading: "exakt",
      model_note: null,
    };
  }
}

function genTask(kind: LearnKind, level: number, r: Rng): Task {
  switch (kind) {
    case "outs":
      return genOuts(level, r);
    case "pot_odds": {
      const pot = r.int(4, 30) + r.pick([0, 0.5]);
      const bet = Math.round(pot * r.pick([0.33, 0.5, 0.66, 0.75, 1]) * 2) / 2;
      const total = pot + bet;
      if (level === 3) {
        const need = (bet / (total + bet)) * 100;
        const eq = Math.max(5, Math.round(need + r.int(-12, 12)));
        return {
          question: `Im Pot liegen ${bb(total)}, du musst ${bb(bet)} callen. Deine Equity beträgt ${pct(eq)}. Callst du?`,
          prompt: { type: "pot_odds", pot: total, to_call: bet, equity_pct: eq },
          answer: { type: "choice", options: [{ id: "call", label: "Call" }, { id: "fold", label: "Fold" }] },
          grading: "exakt",
          model_note: null,
        };
      }
      return {
        question: `Im Pot liegen ${bb(total)}, du musst ${bb(bet)} callen. Wie viel Equity brauchst du mindestens für den Call?`,
        prompt: { type: "pot_odds", pot: total, to_call: bet, equity_pct: null },
        answer: { type: "number", unit: "%", min: 0, max: 100, step: 0.1 },
        grading: "exakt",
        model_note: null,
      };
    }
    case "probability": {
      const outs = r.int(level === 1 ? 4 : 2, level === 1 ? 12 : 17);
      const come = r.pick([1, 2]);
      const method = level === 1 ? "rule" : "exact";
      const unseen = come === 2 ? 47 : r.pick([47, 46]);
      return {
        question:
          method === "rule"
            ? `Du hast ${outs} Outs und siehst noch ${come === 2 ? "Turn und River" : "eine Karte"}. Schätze mit der Regel 2/4.`
            : `Du hast ${outs} Outs, ${unseen} Karten sind unbekannt, noch ${come === 2 ? "zwei Karten" : "eine Karte"}. Wie groß ist die Trefferchance genau?`,
        prompt: { type: "probability", outs, unseen, cards_to_come: come, method },
        answer: { type: "number", unit: "%", min: 0, max: 100, step: 0.1 },
        grading: method === "rule" ? "formel" : "exakt",
        model_note: null,
      };
    }
    case "combos": {
      if (level === 3) {
        const range = r.pick(["TT+, AQs+, AQo+", "QQ+, AKs, AKo", "99+, AJs+, KQs"]);
        const dead = r.deal(new Set(), 1);
        return {
          question: `Wie viele Kombos enthält die Range ${range}, wenn ${dead[0]} liegt?`,
          prompt: { type: "combos", class: null, range, dead },
          answer: { type: "number", unit: "Kombos", min: 0, max: 200, step: 1 },
          grading: "exakt",
          model_note: null,
        };
      }
      const cls = r.pick(["AKs", "AKo", "QQ", "JTs", "KQo", "77", "A5s"]);
      const dead = level === 2 ? [cls[0] + r.pick(["s", "h", "d", "c"])] : [];
      return {
        question: dead.length ? `Wie viele Kombos von ${cls} gibt es, wenn ${dead[0]} auf dem Board liegt?` : `Wie viele Kombos gibt es von ${cls}?`,
        prompt: { type: "combos", class: cls, range: null, dead },
        answer: { type: "number", unit: "Kombos", min: 0, max: 50, step: 1 },
        grading: "exakt",
        model_note: null,
      };
    }
    case "position": {
      const size = r.pick([6, 8, 9]);
      const occupied = Array.from({ length: size }, (_, i) => i + 1);
      const button = r.int(1, size);
      const labels = positionLabels(size);
      const ask = r.next() < 0.5;
      const seat = r.int(1, size);
      const label = labelOf(seat, button, size);
      return ask
        ? {
            question: `Welcher Sitz ist ${label}?`,
            prompt: { type: "position", table_size: size, occupied, button, dead_button: false, small_blind: null, big_blind: null, ask: { type: "seat_of", label } },
            answer: { type: "seat", seats: occupied },
            grading: "exakt",
            model_note: null,
          }
        : {
            question: `Welche Position hat Sitz ${seat}?`,
            prompt: { type: "position", table_size: size, occupied, button, dead_button: false, small_blind: null, big_blind: null, ask: { type: "label_of", seat } },
            answer: { type: "choice", options: labels.map((l) => ({ id: l, label: l })) },
            grading: "exakt",
            model_note: null,
          };
    }
    case "showdown": {
      const n = level === 1 ? 2 : r.pick([2, 3]);
      let used = new Set<string>();
      let board = r.deal(used, 5);
      let players = Array.from({ length: n }, () => r.deal(used, 2));
      // Like the app: at level 1 the two hands are of different kinds, so the answer never hangs on a kicker.
      while (level === 1 && categoryOf([...players[0], ...board]) === categoryOf([...players[1], ...board])) {
        used = new Set<string>();
        board = r.deal(used, 5);
        players = Array.from({ length: n }, () => r.deal(used, 2));
      }
      return {
        question: "Wer gewinnt den Pot?",
        prompt: { type: "showdown", board, players },
        answer: { type: "choice", options: [...players.map((_, i) => ({ id: `p${i}`, label: `Spieler ${i + 1}` })), { id: "split", label: "Pot teilen" }] },
        grading: "exakt",
        model_note: null,
      };
    }
    case "equity_hand": {
      const used = new Set<string>();
      const hero = r.deal(used, 2);
      const villain = r.deal(used, 2);
      const board = level === 1 ? [] : r.deal(used, 3);
      return {
        question: `Beide sind ${board.length ? "am Flop" : "preflop"} all-in. Wie viel Equity hat deine Hand?`,
        prompt: { type: "equity_hand", hero, villain, board },
        answer: { type: "number", unit: "%", min: 0, max: 100, step: 0.1 },
        grading: "exakt",
        model_note: null,
      };
    }
    case "equity_range": {
      const [name, range, share] = r.pick([
        ["Premium", "QQ+, AKs, AKo", 2.6],
        ["Stärkste 10 %", "77+, ATs+, KQs, AJo+", 9.5],
        ["Stärkste 20 %", "55+, A8s+, KTs+, QTs+, JTs, ATo+, KJo+", 19.8],
      ] as [string, string, number][]);
      const used = new Set<string>();
      const hero = r.deal(used, 2);
      const board = level === 3 ? r.deal(used, 3) : [];
      return {
        question: `Dein Gegner hält eine Hand aus der Range „${name}“. Wie viel Equity hast du ${board.length ? "am Flop" : "preflop"}?`,
        prompt: { type: "equity_range", hero, range_name: name, range, range_pct: share, board },
        answer: { type: "number", unit: "%", min: 0, max: 100, step: 0.1 },
        grading: "modell",
        model_note: "Die Range ist gleich verteilt angenommen: jede Kombo darin ist gleich wahrscheinlich.",
      };
    }
    case "push_fold_sb":
    case "push_fold_bb": {
      const sb = kind === "push_fold_sb";
      const hand = r.deal(new Set(), 2);
      const cls = classOf(hand);
      const stack = level === 1 ? r.pick([5, 8, 10, 12]) : r.int(3, 20) + r.pick([0, 0.5]);
      const ante = level >= 2 ? r.pick([0, 0.1, 0.125]) : 0;
      const dead = level === 3 ? Math.round(ante * r.int(4, 7) * 100) / 100 : 0;
      let setup = `Effektiver Stack ${bb(stack)}`;
      if (ante) setup += `, Ante ${bb(ante)}`;
      if (dead) setup += `, totes Geld im Pot ${bb(dead)}`;
      return {
        question: sb
          ? `Alle haben zu dir in den Small Blind gefoldet. ${setup}. Schiebst du mit ${cls} all-in?`
          : `Der Small Blind geht gegen dich all-in, du bist im Big Blind. ${setup}. Callst du mit ${cls}?`,
        prompt: { type: "push_fold", hero_position: sb ? "sb" : "bb", hand, class: cls, stack_bb: stack, small_blind_bb: 0.5, ante_bb: ante, dead_bb: dead, players: level === 3 ? 8 : 2 },
        answer: { type: "choice", options: sb ? [{ id: "shove", label: "All-in" }, { id: "fold", label: "Fold" }] : [{ id: "call", label: "Call" }, { id: "fold", label: "Fold" }] },
        grading: "modell",
        model_note: "Heads-up-Nash zwischen Small und Big Blind, Chip-EV, ohne ICM. Spieler, die schon gefoldet haben, zählen nur als totes Geld.",
      };
    }
    case "push_fold_open":
    case "push_fold_call": {
      const open = kind === "push_fold_open";
      const profiles = [
        [12, 25, 7, 18, 9, 30, 14, 5, 11],
        [20, 8, 15, 6, 22, 10, 16, 13, 9],
      ];
      const n = level === 1 ? r.int(2, 6) : level === 2 ? r.int(6, 9) : r.int(3, 9);
      const [ante, bbAnte] = level === 1 ? [0, 0] : r.pick([[0.125, 0], [0, 1]]);
      const stacks = level === 3 ? r.pick(profiles).slice(9 - n) : Array(n).fill(level === 1 ? r.pick([8, 10, 12, 15]) : r.pick([6, 8, 10, 12, 15, 20]));
      const shover = open ? null : r.int(0, n - 2);
      const hero = open ? r.int(0, n - 2) : r.int((shover ?? 0) + 1, n - 1);
      const callers = !open && level === 3 && hero > (shover ?? 0) + 1 && r.next() < 0.4 ? [r.int((shover ?? 0) + 1, hero - 1)] : [];
      return tableTask(r.deal(new Set(), 2), stacks, hero, shover, callers, ante, bbAnte, 0);
    }
    case "all_in":
      throw "Diese Übungsart gibt es nur aus eigenen Händen.";
    case "full_hands":
      throw "Ganze Hände gibt es im Übungsmodus „Hände spielen“.";
    default:
      throw "Diese Übung kommt aus drills.json.";
  }
}

const TABLE_NOTE =
  "Gleichgewicht für reines Push/Fold in Chips (ohne ICM): schieben oder folden, nach einem Shove nur callen oder folden. Höchstens 3 Spieler all-in, 3-Wege-Equity geschätzt.";

function tableTask(hand: string[], stacks: number[], hero: number, shover: number | null, callers: number[], ante: number, bbAnte: number, dead: number): Task {
  const n = stacks.length;
  const positions = positionLabels(n);
  const cls = classOf(hand);
  const others = shover === null ? stacks.slice(hero + 1) : [stacks[shover], ...callers.map((c) => stacks[c])];
  const effective = Math.min(stacks[hero], Math.max(...others));
  const at: Record<string, string> = { BB: "im Big Blind", SB: "im Small Blind", BTN: "am Button", CO: "im Cutoff", HJ: "im Hijack", LJ: "im Lojack" };
  let setup = `Dein Stack ${bb(stacks[hero])}`;
  if (effective < stacks[hero]) setup += `, effektiv ${bb(effective)}`;
  if (ante) setup += `, Ante ${bb(ante)}`;
  if (bbAnte) setup += `, Big-Blind-Ante ${bb(bbAnte)}`;
  const table = `${n} Spieler, du sitzt ${at[positions[hero]] ?? `auf ${positions[hero]}`}`;
  const action = shover === null ? "" : `${positions[shover]} geht mit ${bb(stacks[shover])} all-in${callers.map((c) => `, ${positions[c]} callt`).join("")}`;
  return {
    question:
      shover === null
        ? `${table}. Alle vor dir haben gefoldet. ${setup}. Schiebst du mit ${cls} all-in?`
        : `${table}. ${action}. ${setup}. Callst du mit ${cls}?`,
    prompt: {
      type: "push_fold_table",
      hand,
      class: cls,
      positions,
      stacks_bb: stacks,
      hero,
      shover,
      callers,
      effective_bb: effective,
      small_blind_bb: 0.5,
      ante_bb: ante,
      bb_ante_bb: bbAnte,
      dead_bb: dead,
    },
    answer: {
      type: "choice",
      options: shover === null ? [{ id: "shove", label: "All-in" }, { id: "fold", label: "Fold" }] : [{ id: "call", label: "Call" }, { id: "fold", label: "Fold" }],
    },
    grading: "modell",
    model_note: TABLE_NOTE,
  };
}

function positionLabels(n: number): string[] {
  const all = ["UTG", "UTG+1", "UTG+2", "LJ", "HJ", "CO", "BTN", "SB", "BB"];
  const tail = all.slice(-3);
  const front = all.slice(0, -3);
  const rest = n - 3;
  const late = ["LJ", "HJ", "CO"].slice(Math.max(0, 3 - rest));
  const early = front.slice(0, Math.max(0, rest - late.length));
  return [...early, ...late, ...tail].slice(-n);
}

function labelOf(seat: number, button: number, n: number): string {
  const labels = positionLabels(n);
  // Button is labels[n-3]; seats go clockwise.
  const offset = (((seat - button) % n) + n) % n;
  return labels[(n - 3 + offset) % n];
}

/* Grading ----------------------------------------------------------------------------------------- */

function base(correct: boolean, s: number, truth: LearnAnswer, truth_text: string, facts: Fact[], ex: Exercise): Feedback {
  return { correct, score: Math.max(0, Math.min(1, s)), truth, truth_text, facts, highlight: [], ev_loss_bb: null, grading: ex.grading, model_note: ex.model_note, grid: null };
}

/** Linear score: 1 inside `tol` percentage points, 0 at `zero`. */
function band(diff: number, tol: number, zero: number) {
  const d = Math.abs(diff);
  return d <= tol ? 1 : Math.max(0, 1 - (d - tol) / (zero - tol));
}

function numberOf(a: LearnAnswer): number | null {
  return "number" in a ? a.number : null;
}

function gradeEstimate(truth: number, a: LearnAnswer, ex: Exercise, facts: Fact[], tol = 2, zero = 15): Feedback {
  const got = numberOf(a) ?? NaN;
  const s = band(got - truth, tol, zero);
  const fb = base(s === 1, s, { number: Math.round(truth * 10) / 10 }, pct(truth), facts, ex);
  if (Number.isFinite(got)) fb.facts.push(fact("Deine Antwort", pct(got)), fact("Abweichung", `${nf(1).format(Math.abs(got - truth))} Prozentpunkte`));
  return fb;
}

function grade(ex: Exercise, a: LearnAnswer): Feedback {
  const p = ex.prompt;
  switch (p.type) {
    case "outs": {
      const o = outsOf(p.hero, p.villain ?? [], p.board);
      const value = o.outs.length + o.half.length / 2;
      const facts = [fact("Outs", nf(o.half.length ? 1 : 0).format(value), o.half.length ? `${o.outs.length} + ${o.half.length} ÷ 2 halbe` : null)];
      if (o.dirty.length) facts.push(fact("Schmutzige Outs", `${o.dirty.length}`, "verbessern dich, gewinnen aber nicht"));
      let fb: Feedback;
      if ("cards" in a) {
        const picked = new Set(a.cards);
        const hits = o.outs.filter((c) => picked.has(c)).length;
        const wrong = a.cards.filter((c) => !o.outs.includes(c) && !o.half.includes(c)).length;
        const s = hits / Math.max(1, o.outs.length + wrong);
        fb = base(hits === o.outs.length && wrong === 0, s, { cards: o.outs }, `${nf(o.half.length ? 1 : 0).format(value)} Outs`, facts, ex);
        fb.highlight = [
          ...o.outs.map((c): Highlight => ({ card: c, tag: picked.has(c) ? "out" : "missed" })),
          ...o.half.map((c): Highlight => ({ card: c, tag: "half_out" })),
          ...a.cards.filter((c) => !o.outs.includes(c) && !o.half.includes(c)).map((c): Highlight => ({ card: c, tag: o.dirty.includes(c) ? "dirty" : "wrong" })),
          ...o.dirty.filter((c) => !picked.has(c)).map((c): Highlight => ({ card: c, tag: "dirty" })),
        ];
      } else {
        const got = numberOf(a) ?? -1;
        const s = got === value ? 1 : Math.max(0, 0.5 - Math.abs(got - value) / 8);
        fb = base(got === value, s, { number: value }, `${value} Outs`, facts, ex);
        fb.highlight = [...o.outs.map((c): Highlight => ({ card: c, tag: "out" })), ...o.half.map((c): Highlight => ({ card: c, tag: "half_out" }))];
      }
      return fb;
    }
    case "pot_odds": {
      const need = (p.to_call / (p.pot + p.to_call)) * 100;
      const facts = [fact("Benötigte Equity", pct(need), `${nf(2).format(p.to_call)} ÷ (${nf(2).format(p.pot)} + ${nf(2).format(p.to_call)})`)];
      if (p.equity_pct !== null) {
        const call = p.equity_pct >= need;
        const choice = "choice" in a ? a.choice : "";
        facts.push(fact("Deine Equity", pct(p.equity_pct)));
        const evCall = (p.equity_pct / 100) * (p.pot + p.to_call) - p.to_call;
        facts.push(fact("EV Call", sbb(Math.round(evCall * 100) / 100), "Equity × (Pot + Call) − Call"));
        const ok = (choice === "call") === call;
        const fb = base(ok, ok ? 1 : 0.2, { choice: call ? "call" : "fold" }, call ? "Call" : "Fold", facts, ex);
        fb.ev_loss_bb = ok ? 0 : Math.round(Math.abs(evCall) * 100) / 100;
        return fb;
      }
      return gradeEstimate(need, a, ex, facts);
    }
    case "probability": {
      const miss = p.cards_to_come === 2 ? ((p.unseen - p.outs) / p.unseen) * ((p.unseen - 1 - p.outs) / (p.unseen - 1)) : (p.unseen - p.outs) / p.unseen;
      const exact = (1 - miss) * 100;
      const rule = p.outs * (p.cards_to_come === 2 ? 4 : 2);
      const facts = [fact("Regel 2/4", pct(rule), `${p.outs} × ${p.cards_to_come === 2 ? 4 : 2}`), fact("Exakt", pct(exact), p.cards_to_come === 2 ? `1 − (${p.unseen - p.outs}/${p.unseen} × ${p.unseen - 1 - p.outs}/${p.unseen - 1})` : `${p.outs} ÷ ${p.unseen}`)];
      return gradeEstimate(p.method === "rule" ? rule : exact, a, ex, facts, p.method === "rule" ? 1 : 2);
    }
    case "combos": {
      const classes = p.class ? [p.class] : parseRange(p.range ?? "");
      const dead = new Set(p.dead);
      const total = classes.flatMap(combosOf).filter(([x, y]) => !dead.has(x) && !dead.has(y)).length;
      const facts = classes.slice(0, 6).map((c) => fact(c, `${combosOf(c).filter(([x, y]) => !dead.has(x) && !dead.has(y)).length} Kombos`));
      if (classes.length > 6) facts.push(fact("Weitere Klassen", `${classes.length - 6}`));
      const got = numberOf(a) ?? -1;
      return base(got === total, got === total ? 1 : Math.max(0, 0.5 - Math.abs(got - total) / 20), { number: total }, `${total} Kombos`, facts, ex);
    }
    case "position": {
      const facts = [fact("Button", `Sitz ${p.button}`), fact("Reihenfolge", positionLabels(p.table_size).join(" · "))];
      if (p.ask.type === "seat_of") {
        const label = p.ask.label;
        const seat = p.occupied.find((s) => labelOf(s, p.button, p.table_size) === label) ?? 1;
        const ok = "seat" in a && a.seat === seat;
        return base(ok, ok ? 1 : 0, { seat }, `Sitz ${seat}`, facts, ex);
      }
      const label = labelOf(p.ask.seat, p.button, p.table_size);
      const ok = "choice" in a && a.choice === label;
      return base(ok, ok ? 1 : 0, { choice: label }, label, facts, ex);
    }
    case "showdown": {
      const scores = p.players.map((h) => score([...h, ...p.board]));
      const best = Math.max(...scores);
      const winners = scores.flatMap((s, i) => (s === best ? [i] : []));
      const truth = winners.length > 1 ? "split" : `p${winners[0]}`;
      const facts = p.players.map((h, i) => fact(`Spieler ${i + 1}`, `${CATEGORY[categoryOf([...h, ...p.board])]} (${bestFive([...h, ...p.board]).join(" ")})`));
      const ok = "choice" in a && a.choice === truth;
      return base(ok, ok ? 1 : 0, { choice: truth }, truth === "split" ? `Pot geteilt zwischen ${winners.map((w) => `Spieler ${w + 1}`).join(" und ")}` : `Spieler ${winners[0] + 1}`, facts, ex);
    }
    case "equity_hand": {
      const eq = mcEquity(p.hero, () => p.villain, p.board, ex.seed, 2500);
      return gradeEstimate(eq, a, ex, [fact("Deine Equity", pct(eq)), fact("Gegner", pct(100 - eq))], 3, 20);
    }
    case "equity_range": {
      const combos = parseRange(p.range).flatMap(combosOf);
      const eq = mcEquity(p.hero, (r, dead) => {
        const c = r.pick(combos);
        return dead.has(c[0]) || dead.has(c[1]) ? null : [...c];
      }, p.board, ex.seed, 2000);
      return gradeEstimate(eq, a, ex, [fact("Deine Equity", pct(eq)), fact("Range", `${p.range} (${pct(p.range_pct)} aller Hände)`)], 3, 20);
    }
    case "push_fold": {
      const sb = p.hero_position === "sb";
      const at = PERCENTILE.get(p.class) ?? 100;
      const s = p.stack_bb;
      const extra = (p.ante_bb * 2 + p.dead_bb) * 8;
      const range = Math.min(100, (sb ? 1100 / Math.max(s, 1.5) - 5 : 600 / Math.max(s, 1.5) - 3) + extra);
      const act = sb ? "shove" : "call";
      const play = at <= range;
      const choice = "choice" in a ? a.choice : "";
      const ok = (choice === act) === play;
      const margin = Math.abs(at - range);
      // Like the engine: the loss is the gap between the two EVs, the better one is the solution.
      const evFold = sb ? -0.5 - p.ante_bb : -1 - p.ante_bb;
      const gap = Math.round(Math.min(1.4, 0.006 * margin * Math.sqrt(s)) * 100) / 100;
      const evAct = evFold + (play ? gap : -gap);
      const loss = ok ? 0 : gap;
      const facts = [
        fact("Nash-Strategie", play ? (sb ? "All-in" : "Call") : "Fold"),
        fact(`EV ${sb ? "All-in" : "Call"}`, sbb(evAct)),
        fact("EV Fold", sbb(evFold)),
        fact(sb ? "Shove-Range" : "Call-Range", `${pct(range)} aller Hände`),
        fact("Effektiver Stack", bb(s)),
      ];
      const fb = base(ok, ok ? 1 : Math.max(0, 0.5 - loss / 2), { choice: play ? act : "fold" }, play ? (sb ? "All-in" : "Call") : "Fold", facts, ex);
      fb.ev_loss_bb = loss;
      return fb;
    }
    case "push_fold_table": {
      const open = p.shover === null;
      const at = PERCENTILE.get(p.class) ?? 100;
      const s = p.effective_bb;
      const behind = p.positions.length - 1 - p.hero;
      const dead = p.ante_bb * p.positions.length + p.bb_ante_bb + p.dead_bb;
      const base0 = open ? 1100 / Math.max(s, 1.5) - 5 : 600 / Math.max(s, 1.5) - 3;
      const range = Math.max(2, Math.min(100, base0 / (1 + 0.3 * Math.max(0, behind - 1)) / (1 + p.callers.length) + dead * 6));
      const act = open ? "shove" : "call";
      const name = open ? "All-in" : "Call";
      const play = at <= range;
      const choice = "choice" in a ? a.choice : "";
      const ok = (choice === act) === play;
      const margin = Math.abs(at - range);
      const pos = p.positions[p.hero];
      const evFold = pos === "SB" ? -0.5 - p.ante_bb : pos === "BB" ? -1 - p.ante_bb - p.bb_ante_bb : -p.ante_bb;
      const gap = Math.round(Math.min(1.4, 0.006 * margin * Math.sqrt(s)) * 100) / 100;
      const evAct = evFold + (play ? gap : -gap);
      const loss = ok ? 0 : gap;
      const facts = [
        fact("Nash-Strategie", play ? `${name} (immer)` : "Fold (immer)"),
        fact(`EV ${name}`, sbb(evAct)),
        fact("EV Fold", sbb(evFold)),
        fact(`${open ? "Shove" : "Call"}-Range ${pos}`, `${pct(range)} aller Hände`),
        fact("Effektiver Stack", bb(s)),
      ];
      const fb = base(ok, ok ? 1 : Math.max(0, 0.5 - loss / 2), { choice: play ? act : "fold" }, play ? name : "Fold", facts, ex);
      fb.ev_loss_bb = loss;
      return fb;
    }
    case "all_in": {
      const spot = SPOTS.find((x) => x.exercise.hand_id === ex.hand_id);
      const r = spot?.all_in;
      if (!r) throw "Spot nicht gefunden";
      const fb = gradeEstimate(r.equity_pct, a, ex, [], 3, 20);
      fb.facts.unshift(fact("Equity", pct(r.equity_pct)));
      fb.facts.push(fact("Pot", bb(r.pot_bb)), fact("Erwartet (EV)", sbb(r.ev_bb), "Equity × (Pot − Rake) − eigener Einsatz"), fact("Tatsächlich", sbb(r.net_bb)), fact("Glück", sbb(r.luck_bb), "Tatsächlich − EV"));
      return fb;
    }
    default:
      throw "Diese Übung wird mit drills.json bewertet.";
  }
}

/* Attempts and progress --------------------------------------------------------------------------- */

type Attempt = { kind: LearnKind; level: number; score: number; at: number };
const DAY = 86_400;
const now = () => Math.floor(Date.now() / 1000);

/** A plausible history so the demo shows every mastery stage. */
const attempts: Attempt[] = (() => {
  const t = now();
  const out: Attempt[] = [];
  const add = (kind: LearnKind, level: number, n: number, s: number, daysAgo: number) => {
    for (let i = 0; i < n; i++) out.push({ kind, level, score: s, at: t - daysAgo * DAY - i * 60 });
  };
  add("outs", 1, 4, 1, 9);
  add("outs", 1, 4, 0.95, 7);
  add("outs", 1, 8, 0.9, 2);
  add("pot_odds", 1, 4, 0.9, 3);
  add("pot_odds", 1, 4, 0.85, 1);
  add("probability", 1, 5, 0.7, 1);
  add("combos", 1, 2, 1, 4);
  add("position", 1, 3, 0.8, 0);
  add("push_fold_sb", 1, 3, 0.55, 6);
  add("showdown", 1, 1, 1, 12);
  return out.sort((a, b) => a.at - b.at);
})();

function skill(kind: LearnKind): SkillProgress {
  const mine = attempts.filter((x) => x.kind === kind);
  const max = mine.reduce((m, x) => Math.max(m, x.level), 0);
  const window = mine.filter((x) => x.level === max).slice(-8);
  const mean = window.length ? window.reduce((s, x) => s + x.score, 0) / window.length : null;
  const days = new Set(window.map((x) => Math.floor(x.at / DAY))).size;
  let level: SkillProgress["level"] = mine.length ? 1 : 0;
  if (mean !== null && window.length >= 3 && mean >= 0.6) level = 2;
  if (mean !== null && window.length === 8 && mean >= 0.8 && days >= 2) level = 3;
  if (level === 3 && mine.length >= 16 && (mine.at(-1)?.at ?? 0) - (mine[0]?.at ?? 0) > 3 * DAY) level = 4;
  return {
    id: kind,
    name_de: NAMES[kind],
    level,
    level_name: LEVEL_NAMES[level],
    max_level: max,
    attempts: mine.length,
    mean_score: mean,
    last_at: mine.at(-1)?.at ?? null,
  };
}

function record(ex: Exercise, fb: Feedback) {
  attempts.push({ kind: ex.kind, level: ex.level, score: fb.score, at: now() });
}

/* Own spots --------------------------------------------------------------------------------------- */

type SpotSeed = { kind: LearnKind; id: number; when: string; hero: string[]; stack: number; villainCards: string[]; board: string[]; heroAct: "shove" | "call" | "fold" };

const SPOT_SEEDS: SpotSeed[] = [
  { kind: "all_in", id: 900000000101, when: "2026/11/16 21:48:12", hero: ["Ah", "Qh"], stack: 18, villainCards: ["Kc", "Kd"], board: ["Qs", "7h", "2c", "Ad", "9s"], heroAct: "call" },
  { kind: "push_fold_open", id: 900000000102, when: "2026/11/16 21:31:40", hero: ["Kd", "7s"], stack: 9.5, villainCards: [], board: [], heroAct: "fold" },
  { kind: "push_fold_call", id: 900000000103, when: "2026/11/16 20:57:03", hero: ["Qs", "9d"], stack: 11, villainCards: ["Ac", "4c"], board: ["8h", "6c", "2d", "Kc", "Ts"], heroAct: "call" },
  { kind: "all_in", id: 900000000104, when: "2026/11/15 23:12:55", hero: ["Js", "Jc"], stack: 24, villainCards: ["Ad", "Kh"], board: ["9d", "4s", "3h", "Kd", "2s"], heroAct: "shove" },
  { kind: "push_fold_open", id: 900000000105, when: "2026/11/15 22:40:18", hero: ["As", "3d"], stack: 13, villainCards: ["Jh", "Jd"], board: ["Td", "8c", "5s", "2h", "Qd"], heroAct: "shove" },
  { kind: "push_fold_call", id: 900000000106, when: "2026/11/15 22:02:44", hero: ["Th", "8h"], stack: 7, villainCards: [], board: [], heroAct: "fold" },
];

/** Hand amounts are in hundredths, like imported hands: 400 chips. */
const BB = 40_000;
const ANTE = 5_000;
const SEATS = ["Hero", "maverick_77", "Lumi.K", "Saltbae", "RiverRat", "nordlicht", "ChipLeader9", "Koenigsweg"];

/** A tournament hand that leads to the spot: everybody folds to the blinds, then the decision. */
function spotHandOf(s: SpotSeed): { hand: Hand; stop: number } {
  const heroSb = s.kind === "push_fold_open" || s.kind === "all_in";
  const sbName = heroSb ? "Hero" : "maverick_77";
  const bbName = heroSb ? "maverick_77" : "Hero";
  const stack = s.stack * BB;
  const seats = SEATS.map((name, i) => ({ seat: i + 1, name, stack: name === "Hero" || name === "maverick_77" ? stack + (name === "maverick_77" ? 2 * BB : 0) : (20 + i * 3) * BB, bounty: null, sitting_out: false }));
  const order = ["Lumi.K", "Saltbae", "RiverRat", "nordlicht", "ChipLeader9", "Koenigsweg"];
  const a = (player: string, kind: Action["kind"], amount: number, all_in = false): Action => ({ street: "preflop", player, kind, amount, to: null, all_in });
  const actions: Action[] = [
    ...SEATS.map((p) => a(p, "post_ante", ANTE)),
    a(sbName, "post_small_blind", BB / 2),
    a(bbName, "post_big_blind", BB),
    ...order.map((p) => a(p, "fold", 0)),
  ];
  const heroAll = stack - ANTE;
  let stop: number;
  if (heroSb) {
    stop = actions.length;
    if (s.heroAct === "fold") actions.push(a("Hero", "fold", 0));
    else {
      actions.push(a("Hero", "raise", heroAll - BB / 2, true));
      actions.push(s.villainCards.length ? a("maverick_77", "call", heroAll - BB, false) : a("maverick_77", "fold", 0));
    }
  } else {
    actions.push(a("maverick_77", "raise", heroAll + 2 * BB - BB / 2, true));
    stop = actions.length;
    actions.push(s.heroAct === "fold" ? a("Hero", "fold", 0) : a("Hero", "call", heroAll - BB, true));
  }
  if (s.kind === "all_in") stop = actions.length;
  const showdown = s.heroAct !== "fold" && s.villainCards.length > 0;
  const pot = showdown ? 2 * (stack - ANTE) + ANTE * 8 : BB * 1.5 + ANTE * 8;
  const heroWins = showdown && score([...s.hero, ...s.board]) > score([...s.villainCards, ...s.board]);
  const hand: Hand = {
    id: s.id,
    game: "tournament",
    tournament_id: 4046789300,
    variant: "Hold'em No Limit",
    table_name: "4046789300 10",
    max_seats: 8,
    button_seat: heroSb ? 8 : 1,
    small_blind: BB / 2,
    big_blind: BB,
    currency: null,
    played_at: s.when,
    seats,
    hero: "Hero",
    hero_cards: s.hero,
    board: showdown ? s.board : [],
    actions,
    returned: [],
    shown: showdown ? [["Hero", s.hero], ["maverick_77", s.villainCards]] : [],
    collected: [[heroWins || (!showdown && s.heroAct !== "fold") ? "Hero" : "maverick_77", pot]],
    showdown,
    total_pot: pot,
    rake: 0,
    bounties_won: [],
    bounty_currency: null,
  };
  if (!heroSb) {
    // Hero sits in the big blind: the button moves one seat back.
    hand.button_seat = 8;
    hand.seats = seats.map((x) => (x.name === "Hero" ? { ...x, seat: 2 } : x.name === "maverick_77" ? { ...x, seat: 1 } : x));
  }
  return { hand, stop };
}

function makeSpot(s: SpotSeed): OwnSpot {
  const { stop } = spotHandOf(s);
  const base = { kind: s.kind, level: 1, seed: 0, hand_id: s.id };
  if (s.kind === "all_in") {
    const eq = mcEquity(s.hero, () => s.villainCards, [], s.id % 1000, 3000);
    const pot = 2 * s.stack + (ANTE / BB) * 8;
    const ev = (eq / 100) * pot - s.stack;
    const net = score([...s.hero, ...s.board]) > score([...s.villainCards, ...s.board]) ? pot - s.stack : -s.stack;
    const round = (x: number) => Math.round(x * 100) / 100;
    const exercise: Exercise = {
      ...base,
      question: "Du warst in dieser Hand preflop all-in, alle Karten liegen offen. Wie viel Equity hattest du?",
      prompt: { type: "all_in", hero: s.hero, opponents: [s.villainCards], board: [], pots: [{ amount_bb: round(pot), players: [0, 1] }] },
      answer: { type: "number", unit: "%", min: 0, max: 100, step: 0.1 },
      grading: "exakt",
      model_note: null,
    };
    return {
      id: `all_in:${s.id}`,
      kind: s.kind,
      hand_id: s.id,
      played_at: s.when,
      stop_before_action: stop,
      exercise,
      played: null,
      all_in: { equity_pct: round(eq), pot_bb: round(pot), ev_bb: round(ev), net_bb: round(net), luck_bb: round(net - ev), rake_bb: 0 },
    };
  }
  // Seats in action order: everyone after the big blind, the blinds last.
  const { hand } = spotHandOf(s);
  const bySeat = [...hand.seats].sort((x, y) => x.seat - y.seat);
  const bbAt = bySeat.findIndex((x) => x.name === (s.kind === "push_fold_open" ? "maverick_77" : "Hero"));
  const order = bySeat.map((_, i) => bySeat[(bbAt + 1 + i) % bySeat.length]);
  const stacks = order.map((x) => x.stack / BB);
  const heroAt = order.findIndex((x) => x.name === "Hero");
  const shover = s.kind === "push_fold_call" ? order.length - 2 : null;
  const t = tableTask(s.hero, stacks, heroAt, shover, [], ANTE / BB, 0, 0);
  const exercise: Exercise = { ...base, level: 3, ...t };
  return { id: `${s.kind}:${s.id}`, kind: s.kind, hand_id: s.id, played_at: s.when, stop_before_action: stop, exercise, played: { choice: s.heroAct }, all_in: null };
}

const SPOTS: OwnSpot[] = SPOT_SEEDS.map(makeSpot);

/* Public stand-ins -------------------------------------------------------------------------------- */

/* Kinds solved by the engine: real exercises written by crates/ringside-learn/tests/demo.rs. --------- */

type Drill = { exercise: Exercise; feedback: Record<string, Feedback> };
const DRILLS: ReadonlySet<LearnKind> = new Set(["bounty_odds", "icm_call", "open_fold", "open_fold_short", "river_bet", "stack_depth", "hud_read"]);
let drills: Promise<Record<string, Drill[][]>> | null = null;

async function drill(kind: LearnKind, level: number, seed: number): Promise<Drill> {
  drills ??= import("./drills.json").then((m) => m.default as unknown as Record<string, Drill[][]>);
  const list = (await drills)[kind][level - 1];
  return list[seed % list.length];
}

/** Score bands of the number answers, as the Rust graders use them: [full, zero, relative to the truth]. */
function tolerance(ex: Exercise): [number, number, boolean] {
  const p = ex.prompt;
  if (p.type === "icm") {
    const pool = p.payouts.reduce((a, b) => a + b, 0);
    return [0.025 * pool, 0.12 * pool, false];
  }
  if (p.type === "stack_depth") return p.ask === "bb" ? [0.03, 0.25, true] : p.ask === "m" ? [0.05, 0.3, true] : [1.5, 8, false];
  return [2, 10, false];
}

function checkDrill(d: Drill, answer: LearnAnswer): Feedback {
  if ("choice" in answer) {
    const fb = d.feedback[answer.choice];
    if (!fb) throw `Unbekannte Antwort '${answer.choice}'`;
    return structuredClone(fb);
  }
  if (!("number" in answer)) throw "Hier wird eine Zahl erwartet";
  const fb = structuredClone(d.feedback.number);
  const truth = "number" in fb.truth ? fb.truth.number : 0;
  const [full, zero, relative] = tolerance(d.exercise);
  const err = relative ? (answer.number - truth) / truth : answer.number - truth;
  fb.score = band(err, full, zero);
  fb.correct = Math.abs(err) <= full;
  return fb;
}

export async function demoExercise(kind: LearnKind, level: number, seed?: number | null): Promise<Exercise> {
  // Pictures of a round show the same hands before and after the answer.
  const pictured = typeof window !== "undefined" && new URLSearchParams(window.location.search).has("session");
  const s = seed ?? (pictured ? 1000 + kind.length * 97 + level : Math.floor(Math.random() * 2 ** 52));
  if (DRILLS.has(kind)) return { ...(await drill(kind, level, s)).exercise, seed: s };
  return exerciseNow(kind, level, s);
}

function exerciseNow(kind: LearnKind, level: number, seed: number): Exercise {
  return { kind, level, seed, hand_id: null, ...genTask(kind, level, rng(seed)) };
}

export async function demoCheck(exercise: Exercise, answer: LearnAnswer): Promise<Feedback> {
  if (DRILLS.has(exercise.kind)) {
    const fb = checkDrill(await drill(exercise.kind, exercise.level, exercise.seed), answer);
    record(exercise, fb);
    return fb;
  }
  return checkNow(exercise, answer);
}

function checkNow(exercise: Exercise, answer: LearnAnswer): Feedback {
  const ex = exercise.hand_id === null ? exerciseNow(exercise.kind, exercise.level, exercise.seed) : exercise;
  const fb = grade(ex, answer);
  record(ex, fb);
  return fb;
}

export function demoOwnSpots(kind: LearnKind | null, limit: number): OwnSpot[] {
  return SPOTS.filter((s) => !kind || s.kind === kind).slice(0, limit);
}

export function demoCheckOwn(spotId: string, answer: LearnAnswer): Feedback {
  const spot = SPOTS.find((s) => s.id === spotId);
  if (!spot) throw "Spot nicht gefunden";
  const fb = checkNow(spot.exercise, answer);
  if (spot.played && "choice" in spot.played) {
    const loss = grade(spot.exercise, spot.played).ev_loss_bb ?? 0;
    const did = { shove: "All-in", call: "Call" }[spot.played.choice] ?? "Fold";
    fb.facts.push(fact("Damals gespielt", `${did}, ${loss > 0.005 ? `kostete ${bb(loss)}` : "im Modell richtig"}`));
  }
  return fb;
}

export function demoProgress(): LearnProgress {
  const t = now();
  const d = new Date(t * 1000);
  const monday = Date.UTC(d.getUTCFullYear(), d.getUTCMonth(), d.getUTCDate() - ((d.getUTCDay() + 6) % 7)) / 1000;
  return {
    skills: KINDS.map(skill),
    xp_week: attempts.filter((x) => x.at >= monday).length,
    xp_total: attempts.length,
  };
}

export function demoSpotHand(id: number): Hand | null {
  const s = SPOT_SEEDS.find((x) => x.id === id);
  return s ? spotHandOf(s).hand : null;
}

/* Playing profile and practice from it ------------------------------------------------------------ */

type DemoGroup = Omit<SpotGroup, "error_pct" | "mean_loss_bb" | "few" | "unsure" | "computed_pct" | "estimate_loss_pct"> & {
  computed: number;
  estimate_loss: number;
};

const g = (x: DemoGroup): SpotGroup => {
  const { computed, estimate_loss, ...rest } = x;
  return {
    ...rest,
    error_pct: (100 * x.flawed) / x.decisions,
    mean_loss_bb: x.loss_bb / x.decisions,
    few: x.decisions < 10,
    computed_pct: (100 * computed) / x.decisions,
    estimate_loss_pct: (100 * estimate_loss) / x.loss_bb,
    unsure: estimate_loss / x.loss_bb > 0.5,
  };
};

const DEMO_GROUPS: SpotGroup[] = [
  g({
    key: "river_vs_bet|-|3|6", lage: "river_vs_bet", seat: null, band: 3, table: "short", title: "River: auf einen Einsatz reagieren", detail: "ab 40 BB · 6 Spieler",
    decisions: 23, flawed: 9, loss_bb: 14.6, open: 2, sources: [["estimate", 15], ["solver", 8]], computed: 8, estimate_loss: 9.9,
    estimate_reliability: 0.73, rank: 1, last_played_at: "2026/11/17 22:41:05", change: null,
  }),
  g({
    key: "vs_limp|SB|3|6", lage: "vs_limp", seat: "SB", band: 3, table: "short", title: "Wenn einer nur mitgeht (Limp) im Small Blind", detail: "ab 40 BB · 6 Spieler",
    decisions: 31, flawed: 9, loss_bb: 11.2, open: 0, sources: [["model", 31]], computed: 31, estimate_loss: 0,
    estimate_reliability: null, rank: 2, last_played_at: "2026/11/17 21:12:44",
    change: { before: 21, before_error_pct: 38, after: 10, after_error_pct: 20 },
  }),
  g({
    key: "push_fold|BTN|1|6", lage: "push_fold", seat: "BTN", band: 1, table: "short", title: "All-in oder Fold als Erster am Button", detail: "10–20 BB · 6 Spieler",
    decisions: 44, flawed: 6, loss_bb: 8.4, open: 0, sources: [["equilibrium", 44]], computed: 44, estimate_loss: 0,
    estimate_reliability: null, rank: 3, last_played_at: "2026/11/16 23:02:10", change: null,
  }),
  g({
    key: "turn_bet|-|3|6", lage: "turn_bet", seat: null, band: 3, table: "short", title: "Turn: setzen oder checken", detail: "ab 40 BB · 6 Spieler",
    decisions: 19, flawed: 5, loss_bb: 6.1, open: 3, sources: [["solver", 12], ["estimate", 7]], computed: 12, estimate_loss: 2.5,
    estimate_reliability: 0.71, rank: 4, last_played_at: "2026/11/17 20:55:31", change: null,
  }),
  g({
    key: "first_in|BTN|3|6", lage: "first_in", seat: "BTN", band: 3, table: "short", title: "Eröffnen am Button", detail: "ab 40 BB · 6 Spieler",
    decisions: 52, flawed: 3, loss_bb: 2.3, open: 0, sources: [["model", 52]], computed: 52, estimate_loss: 0,
    estimate_reliability: null, rank: 5, last_played_at: "2026/11/17 22:10:00", change: null,
  }),
  g({
    key: "first_in|BTN|3|5", lage: "first_in", seat: "BTN", band: 3, table: "few", title: "Eröffnen am Button", detail: "ab 40 BB · 3–5 Spieler",
    decisions: 14, flawed: 4, loss_bb: 2.8, open: 0, sources: [["model", 14]], computed: 14, estimate_loss: 0,
    estimate_reliability: null, rank: null, last_played_at: "2026/11/17 23:05:00", change: null,
  }),
  g({
    key: "vs_three_bet|BTN|3|6", lage: "vs_three_bet", seat: "BTN", band: 3, table: "short", title: "Gegen 3-Bet am Button", detail: "ab 40 BB · 6 Spieler",
    decisions: 6, flawed: 2, loss_bb: 1.9, open: 0, sources: [["model", 6]], computed: 6, estimate_loss: 0,
    estimate_reliability: null, rank: null, last_played_at: "2026/11/15 19:40:00", change: null,
  }),
];

/** The demo hands' decisions behind each group: hand id and action index. */
const DEMO_DRILLS: Record<string, [number, number][]> = {
  "river_vs_bet|-|3|6": [[250000000000, 14], [250000000002, 18]],
  "vs_limp|SB|3|6": [[250000000002, 6]],
  "push_fold|BTN|1|6": [[250000000001, 11]],
  "turn_bet|-|3|6": [[250000000000, 12]],
  "first_in|BTN|3|6": [[250000000000, 5]],
};

export function demoProfile(): PlayProfile {
  if (previewEmptyProfile()) return { decisions: 0, loss_bb: 0, open: 0, min_decisions: 10, groups: [] };
  const decisions = DEMO_GROUPS.reduce((a, x) => a + x.decisions, 0) + 412;
  return { decisions, loss_bb: DEMO_GROUPS.reduce((a, x) => a + x.loss_bb, 0) + 9.7, open: 5, min_decisions: 10, groups: DEMO_GROUPS };
}

function previewEmptyProfile(): boolean {
  return typeof window !== "undefined" && new URLSearchParams(window.location.search).get("profile") === "0";
}

function drillDecision(hand_id: number, action_index: number): Decision | null {
  return DEMO_DECISIONS[hand_id]?.find((d) => d.action_index === action_index) ?? null;
}

function drillSpot(key: string, hand_id: number, action_index: number, seed: number | null): DrillSpot | null {
  const group = DEMO_GROUPS.find((x) => x.key === key);
  const d = drillDecision(hand_id, action_index);
  if (!group || !d) return null;
  const base = demoHandById(hand_id);
  const hand: Hand = structuredClone(base);
  hand.actions = hand.actions.slice(0, action_index + 1);
  hand.shown = [];
  if (seed !== null) {
    hand.hero_cards = rng(seed).deal(new Set([...base.board, ...base.hero_cards]), 2);
  }
  return {
    key,
    title: group.title,
    detail: group.detail,
    hand_id,
    action_index,
    seed,
    played_at: base.played_at,
    hand,
    hero_cards: hand.hero_cards,
    position: group.seat === "EP" || group.seat === "MP" ? null : group.seat,
    choices: d.strategy.map((c) => ({ label: c.label, kind: c.kind })),
    source: d.source,
    reliability: d.reliability,
    pot_bb: d.pot_bb,
  };
}

export function demoDrillReal(key: string, limit: number): DrillSpot[] {
  return (DEMO_DRILLS[key] ?? [])
    .map(([h, a]) => drillSpot(key, h, a, null))
    .filter((s): s is DrillSpot => !!s)
    .slice(0, limit);
}

export function demoDrillVariant(key: string, seed: number): DrillSpot {
  const list = DEMO_DRILLS[key] ?? [];
  if (!list.length) throw "Für diese Lage gibt es noch keine bewerteten Hände";
  const [h, a] = list[seed % list.length];
  const s = drillSpot(key, h, a, seed);
  if (!s) throw "Für diese Lage konnte keine neue Hand bewertet werden";
  return s;
}

/** The practice's grades (crates/ringside-play/src/grade.rs), in percent of the pot. */
function demoRating(strategy: Decision["strategy"], chosen: number, pot: number): { rating: Rating; loss: number; points: number } {
  const freq = strategy[chosen].freq;
  const main = strategy.reduce((m, c, i) => (c.freq > strategy[m].freq ? i : m), 0);
  const isMain = chosen === main || freq >= 0.5;
  const evs = strategy.map((c) => c.ev_bb ?? 0);
  const best = Math.max(...evs);
  const mixed = freq >= 0.1;
  const loss = mixed ? 0 : Math.max(0, best - evs[chosen]);
  const pct = (loss / Math.max(pot, 1e-9)) * 100;
  const rating: Rating =
    (pct <= 0.5 || mixed) && isMain && freq > 0 ? "best" : mixed ? "mix" : pct <= 2 ? "good" : pct <= 6 ? "inaccuracy" : pct <= 15 ? "mistake" : "blunder";
  const points = { best: 1, mix: 1, good: 0.7, inaccuracy: 0.3, mistake: 0, blunder: 0 }[rating];
  return { rating, loss, points };
}

export function demoDrillCheck(key: string, hand_id: number, action_index: number, seed: number | null, choice: number): DrillResult {
  const d = drillDecision(hand_id, action_index);
  if (!d || !DEMO_GROUPS.some((x) => x.key === key)) throw "Spot nicht gefunden";
  const { rating, loss, points } = demoRating(d.strategy, choice, d.pot_bb ?? 1);
  const decision: Decision = {
    ...d,
    strategy: d.strategy.map((c, i) => ({ ...c, played: i === choice })),
    played: d.strategy[choice].label,
    played_freq: d.strategy[choice].freq,
    rating,
    ev_loss_bb: loss,
    // The stored reason names the real hand's cards; a dealt variation has others.
    why: seed === null ? d.why : "",
  };
  attempts.push({ kind: "own_decision", level: 1, score: points, at: now() });
  return { decision, score: points, correct: ["best", "mix", "good"].includes(rating), then: seed === null ? d : null };
}
