/** Browser stand-ins for the hand analysis, shaped like the real commands. `?analysis=0` starts unanalysed. */
import { demoChange } from "../skill/demo";
import type { HandQuery, HandSummary } from "../types";
import { DEMO_DECISIONS } from "./demo-hands";
import { RATINGS } from "./labels";
import type { AnalysisStatus, AnalysisSummary, AutoDone, Decision, DecisionBrief, JobProgress } from "./types";

const fresh = typeof window !== "undefined" && new URLSearchParams(window.location.search).get("analysis") === "0";
const pokerStars = typeof window !== "undefined" && ["1", "gg"].includes(new URLSearchParams(window.location.search).get("locked") ?? "");
let enabled = !fresh;
let analyzed = fresh ? 0 : 6431;
const done = new Set<number>();
let solvingHand: number | null = null;
const listeners = new Set<(s: AnalysisStatus) => void>();
const handListeners = new Set<(id: number) => void>();

const MAIN = 250000000002;

/** The demo's GGPoker hand: a 12-BB shove with A♥Q♥ on the button (see `demoGgHand`). */
function ggShove(id: number): Decision {
  return {
    action_index: 13,
    best: "All-in",
    engine: "demo",
    ev_loss_bb: 0,
    hand_id: id,
    note: "Push/Fold-Gleichgewicht (Chip-EV): 7 Spieler, 12 BB, Ante 0,13 BB. Bounties stehen in GGPoker-Händen nicht – sie zählen hier nicht mit.",
    played: "All-in",
    played_freq: 1,
    pot_bb: 1.43,
    rating: "best",
    reason: null,
    reliability: null,
    solver: null,
    source: "equilibrium",
    spot: "shove",
    status: "rated",
    strategy: [
      { ev_bb: 0, freq: 0, kind: "fold", label: "Fold", played: false },
      { ev_bb: 1.9, freq: 1, kind: "all_in", label: "All-in", played: true },
    ],
    street: "preflop",
    why: "Am Button mit 12 BB gehört AQs klar zu den Händen, die all-in gehen: Fold bringt nichts, All-in im Schnitt 1,9 BB.",
  };
}

function demoHandDecisions(id: number): Decision[] {
  if (id >= 2 ** 52) return [ggShove(id)];
  return (DEMO_DECISIONS[id] ?? DEMO_DECISIONS[MAIN]).map((d) => ({ ...d, hand_id: id }));
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
  if (s.state === "locked") return { ...base, title: "Pausiert, solange ein Poker-Client läuft", state: "paused_pokerstars" };
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
    state: pokerStars ? "locked" : enabled ? (analyzed < 6431 ? "preflop" : "postflop") : "idle",
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
    auto_run: false,
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
  // Everything but the solver's spots is there at once; those wait for the solve.
  return demoHandDecisions(id).map((d) =>
    d.source !== "solver" ? d : { ...d, status: "pending", rating: null, ev_loss_bb: null, why: "", reason: "Wartet auf den Postflop-Solver." },
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

/** The worst decision of a demo hand, as the hand list shows it. */
function worstOf(list: Decision[]): DecisionBrief | null {
  const order = (d: Decision) => (d.status === "rated" ? 0 : d.status === "pending" ? 1 : d.status === "unrated" ? 2 : 3);
  const sorted = [...list].sort((a, b) => order(a) - order(b) || (b.ev_loss_bb ?? 0) - (a.ev_loss_bb ?? 0));
  return sorted[0] ?? null;
}

const brief = (b: Omit<DecisionBrief, "pot_bb" | "reliability" | "reason" | "why"> & Partial<DecisionBrief>): DecisionBrief => ({
  pot_bb: 4,
  reliability: null,
  reason: null,
  why: "",
  ...b,
});

const WORST: (DecisionBrief | null)[] = [
  ...[250000000002, 250000000001, 250000000000, 249999999999].map((id) => worstOf(DEMO_DECISIONS[id] ?? [])),
  brief({ action_index: 6, street: "preflop", spot: "vs_open", status: "rated", played: "Call 2 BB", best: "Call", ev_loss_bb: 0, rating: "best", source: "model" }),
  brief({ action_index: 8, street: "flop", spot: "c_bet", status: "pending", played: "Bet 1,5 BB", best: null, ev_loss_bb: null, rating: null, source: "solver" }),
  brief({ action_index: 6, street: "preflop", spot: "call_shove", status: "rated", played: "Call 12 BB", best: "Fold", ev_loss_bb: 1.2, rating: "blunder", source: "equilibrium", pot_bb: 14.5 }),
];

/** The demo hand list with the worst decision per row, narrowed and sorted like the real query. */
export function withWorst(list: HandSummary[], q: HandQuery): HandSummary[] {
  // Every GGPoker row opens the demo's one GGPoker hand; the PokerStars rows keep their verdicts.
  let i = 0;
  let rows = list.map((h) => {
    const worst = h.site === "ggpoker" ? worstOf([ggShove(h.id)]) : WORST[i++ % WORST.length];
    return { ...h, worst: isAnalyzed(h.id) ? worst : null };
  });
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
  if (typeof window !== "undefined" && new URLSearchParams(window.location.search).get("data") === "empty")
    return { hands: 0, analyzed: 0, decisions: 0, rated: 0, loss_bb: 0, ratings: RATINGS.map(() => 0), by_source: [], by_spot: [], costliest: [] };
  if (!analyzed) return { hands: 6431, analyzed: 0, decisions: 0, rated: 0, loss_bb: 0, ratings: RATINGS.map(() => 0), by_source: [], by_spot: [], costliest: [] };
  const full = fullSummary();
  if (analyzed >= 6431) return full;
  // While the demo run is under way, the snapshot grows with it.
  const f = analyzed / 6431;
  const part = (n: number) => Math.round(n * f);
  return {
    ...full,
    rated: part(full.rated),
    loss_bb: Math.round(full.loss_bb * f * 10) / 10,
    ratings: full.ratings.map(part),
    by_source: full.by_source.map(([s, n]) => [s, part(n)]),
    by_spot: full.by_spot.map((s) => ({ ...s, rated: part(s.rated), flawed: part(s.flawed), loss_bb: Math.round(s.loss_bb * f * 10) / 10 })),
  };
}

function fullSummary(): AnalysisSummary {
  return {
    hands: 6431,
    analyzed,
    decisions: 9120,
    rated: 9120,
    loss_bb: 412.6,
    ratings: [6890, 610, 902, 488, 231, 71],
    by_source: [
      ["model", 5980],
      ["equilibrium", 1530],
      ["solver", 652],
      ["estimate", 958],
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
      { hand_id: 249999999996, played_at: "2026/11/16 21:12:10", hero_cards: "As 5s", position: "HJ", ...WORST[6]! },
      { hand_id: 250000000001, played_at: "2026/11/16 22:20:10", hero_cards: "Ah 7c", position: "BTN", ...WORST[1]! },
    ],
  };
}

export function onAutoDone(handler: (d: AutoDone) => void) {
  const shown = typeof window !== "undefined" && new URLSearchParams(window.location.search).get("autodone") === "1";
  const timer = shown ? setTimeout(() => handler({ hands: 42, mistakes: 3, job: null, skill: demoChange() }), 600) : undefined;
  return () => clearTimeout(timer);
}
