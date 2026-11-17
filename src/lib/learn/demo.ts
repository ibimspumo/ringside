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
import { parseRange } from "./catalog";
import { combosOf, deckWithout, matrixHand, score, categoryOf, CATEGORY } from "./wissen/poker";

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
      const used = new Set<string>();
      const board = r.deal(used, 5);
      const players = Array.from({ length: n }, () => r.deal(used, 2));
      return {
        question: "Wer gewinnt den Pot?",
        prompt: { type: "showdown", board, players },
        answer: { type: "choice", options: [...players.map((_, i) => ({ id: `p${i}`, label: `Spieler ${i + 1}` })), { id: "split", label: "Split" }] },
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
  return { correct, score: Math.max(0, Math.min(1, s)), truth, truth_text, facts, highlight: [], ev_loss_bb: null, grading: ex.grading, model_note: ex.model_note };
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
      const facts = p.players.map((h, i) => fact(`Spieler ${i + 1}`, CATEGORY[categoryOf([...h, ...p.board])]));
      const ok = "choice" in a && a.choice === truth;
      return base(ok, ok ? 1 : 0, { choice: truth }, truth === "split" ? "Split" : `Spieler ${winners[0] + 1}`, facts, ex);
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

export function demoExercise(kind: LearnKind, level: number, seed?: number | null): Exercise {
  const s = seed ?? Math.floor(Math.random() * 2 ** 52);
  return { kind, level, seed: s, hand_id: null, ...genTask(kind, level, rng(s)) };
}

export function demoCheck(exercise: Exercise, answer: LearnAnswer): Feedback {
  const ex = exercise.hand_id === null ? demoExercise(exercise.kind, exercise.level, exercise.seed) : exercise;
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
  const fb = demoCheck(spot.exercise, answer);
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
