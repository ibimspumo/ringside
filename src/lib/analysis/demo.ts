/** Browser stand-ins for the hand analysis, shaped like the real commands. `?analysis=0` starts unanalysed. */
import type { HandQuery, HandSummary } from "../types";
import { RATINGS } from "./labels";
import type { AnalysisStatus, AnalysisSummary, Choice, Decision, DecisionBrief, JobProgress, Rating } from "./types";

const fresh = typeof window !== "undefined" && new URLSearchParams(window.location.search).get("analysis") === "0";
let enabled = !fresh;
let analyzed = fresh ? 0 : 6431;
const done = new Set<number>();
let solvingHand: number | null = null;
const listeners = new Set<(s: AnalysisStatus) => void>();
const handListeners = new Set<(id: number) => void>();

const MODEL_NOTE =
  "Preflop-Modell (kein Gleichgewicht des echten Spiels): 6 Spieler, alle mit 100 BB, Ante für alle; nächste vorgerechnete Tiefe: 100 BB statt 110,7 BB.";
const SOLVER_NOTE =
  "Postflop-Solver heads-up, Ranges aus dem Preflop-Modell (6 Spieler, 100 BB). Einsatzgrößen Flop 33 %, Raise 60 %, Turn 75 %, River 75 %, All-in ab 150 % Pot; Bet von 45 % Pot als 50 % gerechnet. Gelöst bis 0,9 % des Pots Ausbeutbarkeit (210 Iterationen).";

/** The line to each street's decision, checked down in the demo tree. */
const DEMO_PATH = {
  flop: [],
  turn: [{ action: 0 }, { action: 0 }, { card: "Kd" }],
  river: [{ action: 0 }, { action: 0 }, { card: "Kd" }, { action: 0 }, { action: 0 }, { card: "5c" }],
};

function choice(label: string, freq: number, ev: number, played = false): Choice {
  return { label, freq, ev_bb: ev, played };
}

function rated(
  index: number,
  street: Decision["street"],
  spot: Decision["spot"],
  source: Decision["source"],
  played: string,
  strategy: Choice[],
  rating: Rating,
  why: string,
): Decision {
  const best = strategy.reduce((a, b) => (b.freq > a.freq ? b : a));
  const top = Math.max(...strategy.map((c) => c.ev_bb ?? 0));
  const mine = strategy.find((c) => c.played)!;
  return {
    hand_id: 0,
    action_index: index,
    street,
    spot,
    status: "rated",
    played,
    best: best.label,
    strategy,
    played_freq: mine.freq,
    ev_loss_bb: Math.max(0, top - (mine.ev_bb ?? 0)),
    rating,
    source,
    why,
    note: source === "model" ? MODEL_NOTE : SOLVER_NOTE,
    reason: null,
    solver:
      source === "solver"
        ? JSON.stringify({
            board: ["9c", "4h", "2s"],
            pot: 6.5,
            stack: 104.2,
            oop: { position: "SB", range: "22+,A2s+,K9s+,QTs+,JTs,ATo+,KJo+", note: "SB: Limp" },
            ip: { position: "BB", range: "random", note: "BB: Check im Big Blind" },
            hero: { player: "oop", cards: ["9s", "9d"] },
            players: 6,
            source: "Hand #250000000002, Analyse",
            path: DEMO_PATH[street as keyof typeof DEMO_PATH] ?? [],
          })
        : null,
    engine: "demo",
  };
}

function demoHandDecisions(id: number): Decision[] {
  const list: Decision[] = [
    rated(
      6,
      "preflop",
      "vs_open",
      "model",
      "Call 0,5 BB",
      [choice("Fold", 0.04, 0), choice("Call", 0.22, 0.86, true), choice("Raise auf 4 BB", 0.74, 1.47)],
      "inaccuracy",
      "Hier wäre Raise auf 4 BB besser gewesen – dein Call kostet dich etwa 0,6 BB. Grund: Mit 99 im Small Blind spielt das Modell bei 100 BB Tiefe hier zu 74 % Raise auf 4 BB.",
    ),
    rated(
      8,
      "flop",
      "flop",
      "solver",
      "Check",
      [choice("Check", 0.81, 3.9, true), choice("Bet 2,2 BB", 0.19, 3.88)],
      "best",
      "Gut gespielt: dein Check ist hier die beste Wahl. Grund: Deine Hand hat hier gegen die Range des Gegners etwa 88 % Equity; der Solver spielt sie zu 81 % Check.",
    ),
    rated(
      11,
      "flop",
      "flop",
      "solver",
      "Raise auf 8 BB",
      [choice("Fold", 0, -2), choice("Call", 0.64, 6.1), choice("Raise auf 7,6 BB", 0.31, 6.02, true), choice("All-in", 0.05, 5.7)],
      "good",
      "Passt: der Solver spielt dein Raise mit deiner Hand zu 31 %, meist aber Call. Grund: Deine Hand hat hier gegen die Range des Gegners etwa 86 % Equity; der Solver spielt sie zu 64 % Call.",
    ),
    rated(
      14,
      "turn",
      "turn",
      "solver",
      "Bet 15 BB",
      [choice("Check", 0.12, 9.1), choice("Bet 13,5 BB", 0.71, 9.6, true), choice("All-in", 0.17, 9.4)],
      "best",
      "Gut gespielt: dein Bet ist hier die beste Wahl. Grund: Deine Hand hat hier gegen die Range des Gegners etwa 82 % Equity; der Solver spielt sie zu 71 % Bet 13,5 BB.",
    ),
    rated(
      16,
      "river",
      "river",
      "solver",
      "Bet 30 BB",
      [choice("Check", 0.68, 7.4), choice("Bet 27 BB", 0.29, 7.1, true), choice("All-in", 0.03, 5.9)],
      "good",
      "Passt: der Solver spielt dein Bet mit deiner Hand zu 29 %, meist aber Check. Grund: Deine Hand hat hier gegen die Range des Gegners etwa 61 % Equity; der Solver spielt sie zu 68 % Check.",
    ),
    rated(
      18,
      "river",
      "river",
      "solver",
      "Call 46 BB",
      [choice("Fold", 0.93, 0), choice("Call", 0.07, -2.3, true)],
      "blunder",
      "Hier wäre Fold besser gewesen – dein Call kostet dich etwa 2,3 BB. Grund: Deine Hand hat hier gegen die Range des Gegners etwa 24 % Equity; der Solver spielt sie zu 93 % Fold.",
    ),
  ];
  return list.map((d) => ({ ...d, hand_id: id }));
}

function isAnalyzed(id: number) {
  return analyzed > 0 || done.has(id);
}

export function demoDecisions(id: number): Decision[] {
  return isAnalyzed(id) ? demoHandDecisions(id) : [];
}

let cancelled = false;

export function demoCancel() {
  enabled = false;
  cancelled = true;
  solvingHand = null;
  emit();
}

export function demoCancelHand(id: number) {
  if (solvingHand === id) solvingHand = null;
  emit();
}

function progress(s: Omit<AnalysisStatus, "progress">): JobProgress {
  const c = s.counts;
  const detail = `${c.analyzed.toLocaleString("de-DE")} von ${c.hands.toLocaleString("de-DE")} Händen, ${c.rated.toLocaleString("de-DE")} Entscheidungen bewertet${c.queued ? `, ${c.queued} Flop-Spots warten` : ""}`;
  const base = { id: "analysis", detail, eta_secs: null, share: null };
  if (s.cancelled) return { ...base, title: "Hand-Analyse abgebrochen", state: "cancelled" };
  if (s.state === "preflop") return { ...base, title: "Analysiere deine Hände", state: "running", share: c.analyzed / c.hands, eta_secs: s.eta_secs };
  if (s.state === "postflop") {
    const mine = s.solving && s.solving.hand_id !== 250000000004;
    return {
      ...base,
      title: mine ? `Rechne Hand #${s.solving!.hand_id} nach dem Flop` : "Rechne Flop-Spots deiner Hände",
      state: "running",
      share: mine ? 0.4 : 1 - c.pending / Math.max(c.decisions, 1),
      eta_secs: s.eta_secs,
    };
  }
  return { ...base, title: "Hand-Analyse wartet", state: "waiting" };
}

export function demoStatus(): AnalysisStatus {
  const s: Omit<AnalysisStatus, "progress"> = {
    counts: { hands: 6431, analyzed, decisions: analyzed * 2, rated: Math.round(analyzed * 1.4), pending: enabled ? 412 : 0, queued: enabled ? 211 : 0 },
    enabled,
    state: enabled ? (analyzed < 6431 ? "preflop" : "postflop") : "idle",
    solving:
      solvingHand !== null
        ? { hand_id: solvingHand, iteration: 120, max_iterations: 300, exploitability_pct: 1.6, elapsed_ms: 21000 }
        : enabled
          ? { hand_id: 250000000004, iteration: 120, max_iterations: 300, exploitability_pct: 1.6, elapsed_ms: 21000 }
          : null,
    eta_secs: enabled ? 211 * 48 : null,
    marked: solvingHand !== null ? [solvingHand] : [],
    threads: 4,
    last_error: null,
    cancelled,
  };
  if (solvingHand !== null && !enabled) s.state = "postflop";
  return { ...s, progress: progress(s) };
}

function emit() {
  const s = demoStatus();
  listeners.forEach((l) => l(s));
}

export function demoRun(on: boolean) {
  enabled = on;
  if (on) cancelled = false;
  if (on && analyzed === 0) {
    const tick = setInterval(() => {
      analyzed = Math.min(6431, analyzed + 900);
      emit();
      if (analyzed >= 6431) clearInterval(tick);
    }, 400);
  }
  emit();
}

export function demoAnalyze(id: number): Decision[] {
  done.add(id);
  solvingHand = id;
  setTimeout(emit, 50);
  setTimeout(() => {
    solvingHand = null;
    emit();
    handListeners.forEach((l) => l(id));
  }, 2500);
  // Preflop is there at once; the postflop spots wait for the solver.
  return demoHandDecisions(id).map((d) =>
    d.street === "preflop" ? d : { ...d, status: "pending", rating: null, ev_loss_bb: null, why: "", reason: "Wartet auf den Postflop-Solver." },
  );
}

export function onStatus(handler: (s: AnalysisStatus) => void) {
  listeners.add(handler);
  return () => {
    listeners.delete(handler);
  };
}

export function onHand(handler: (id: number) => void) {
  handListeners.add(handler);
  return () => {
    handListeners.delete(handler);
  };
}

const WORST: (DecisionBrief | null)[] = [
  { action_index: 18, street: "river", spot: "river", status: "rated", played: "Call 46 BB", best: "Fold", ev_loss_bb: 2.3, rating: "blunder", source: "solver", reason: null, why: "" },
  { action_index: 6, street: "preflop", spot: "steal", status: "rated", played: "Fold", best: "Raise auf 2,2 BB", ev_loss_bb: 0.42, rating: "inaccuracy", source: "model", reason: null, why: "" },
  { action_index: 6, street: "preflop", spot: "shove", status: "rated", played: "Fold", best: "All-in", ev_loss_bb: 0.81, rating: "mistake", source: "equilibrium", reason: null, why: "" },
  { action_index: 6, street: "preflop", spot: "vs_open", status: "rated", played: "Call 2 BB", best: "Call", ev_loss_bb: 0, rating: "best", source: "model", reason: null, why: "" },
  { action_index: 8, street: "flop", spot: "c_bet", status: "pending", played: "Bet 1,5 BB", best: null, ev_loss_bb: null, rating: null, source: "solver", reason: "Wartet auf den Postflop-Solver.", why: "" },
  { action_index: 6, street: "preflop", spot: "vs_open", status: "unrated", played: "Call 1 BB", best: null, ev_loss_bb: null, rating: null, source: null, reason: "Limp: das Preflop-Modell kennt Limps nur aus dem Small Blind.", why: "" },
  { action_index: 6, street: "preflop", spot: "call_shove", status: "rated", played: "Call 12 BB", best: "Fold", ev_loss_bb: 1.7, rating: "blunder", source: "equilibrium", reason: null, why: "" },
];

/** The demo hand list with the worst decision per row, narrowed and sorted like the real query. */
export function withWorst(list: HandSummary[], q: HandQuery): HandSummary[] {
  let rows = list.map((h, i) => ({ ...h, worst: isAnalyzed(h.id) ? WORST[i % WORST.length] : null }));
  const f = q.decisions;
  if (f) {
    rows = rows.filter((h) => {
      const w = h.worst;
      if (f.unrated) return !w || w.status !== "rated";
      if (!w) return false;
      if (f.min_rating && (!w.rating || RATINGS.indexOf(w.rating) < RATINGS.indexOf(f.min_rating))) return false;
      if (f.min_loss_bb !== null && (w.ev_loss_bb ?? -1) < f.min_loss_bb) return false;
      if (f.spots.length && !f.spots.includes(w.spot)) return false;
      if (f.street && w.street !== f.street) return false;
      if (f.source && w.source !== f.source) return false;
      return true;
    });
  }
  if (q.sort_by_loss) rows.sort((a, b) => (b.worst?.ev_loss_bb ?? -1) - (a.worst?.ev_loss_bb ?? -1));
  return rows;
}

export function demoSummary(): AnalysisSummary {
  if (!analyzed) return { hands: 6431, analyzed: 0, decisions: 0, rated: 0, loss_bb: 0, ratings: [0, 0, 0, 0, 0], by_source: [], by_spot: [], costliest: [] };
  return {
    hands: 6431,
    analyzed,
    decisions: 9120,
    rated: 7302,
    loss_bb: 412.6,
    ratings: [5810, 702, 488, 231, 71],
    by_source: [
      ["model", 5120],
      ["equilibrium", 1530],
      ["solver", 652],
    ],
    by_spot: [
      { spot: "call_shove", decisions: 412, rated: 405, flawed: 61, loss_bb: 118.4 },
      { spot: "steal", decisions: 1880, rated: 1712, flawed: 233, loss_bb: 96.1 },
      { spot: "vs_c_bet", decisions: 540, rated: 210, flawed: 48, loss_bb: 61.7 },
      { spot: "shove", decisions: 1118, rated: 1118, flawed: 74, loss_bb: 44.9 },
      { spot: "vs_open", decisions: 2210, rated: 1630, flawed: 140, loss_bb: 38.2 },
      { spot: "river", decisions: 230, rated: 98, flawed: 17, loss_bb: 21.5 },
    ],
    costliest: [
      { hand_id: 250000000002, played_at: "2026/11/16 22:59:10", hero_cards: "9s 9d", position: "SB", ...WORST[0]! },
      { hand_id: 250000000010, played_at: "2026/11/16 21:12:10", hero_cards: "As 5s", position: "HJ", ...WORST[6]! },
      { hand_id: 250000000006, played_at: "2026/11/16 22:20:10", hero_cards: "8h 6h", position: "BTN", ...WORST[2]! },
    ],
  };
}
