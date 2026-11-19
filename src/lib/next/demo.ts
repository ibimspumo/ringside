// Browser preview of „Was als Nächstes?“: the list the backend ranks from the demo data
// (learn/demo.ts profile, demo.ts leaks, analysis/demo.ts costliest hands). `?data=empty` and
// `?profile=0` show it empty.
import type { SpotType } from "../analysis/types";
import { bbText } from "../learn/profile";
import type { KindLoss } from "../report/types";
import type { NextStep } from "./types";

const base = {
  term: null,
  per_time_bb: null,
  times: null,
  total_bb: null,
  trust: 1,
  fresh: 1,
  estimate: false,
  spot_key: null,
  spots: [],
  hand_id: null,
  practice: null,
  hands: null,
  read: null,
} satisfies Partial<NextStep>;

function spot(key: string, spots: SpotType[], title: string, detail: string, decisions: number, flawed: number, loss: number, trust: number, read: string, fresh = 1): NextStep {
  const mean = loss / decisions;
  return {
    ...base,
    id: `spot:${key}`,
    kind: "spot",
    title,
    why: `${flawed} von ${decisions} deiner Entscheidungen hier waren ungenau oder schlechter (${detail}).`,
    number: `kostet dich im Schnitt etwa ${bbText(mean)} pro Mal, ${decisions}-mal gespielt`,
    per_time_bb: mean,
    times: decisions,
    total_bb: loss,
    trust,
    fresh,
    estimate: trust < 0.99,
    score: loss * trust * fresh,
    spot_key: key,
    spots,
    practice: { spot: key },
    hands: { by: "spot", key, label: `${title}, ${detail.split(" · ")[0]}` },
    read,
  };
}

const STEPS: NextStep[] = [
  spot("river_vs_bet|-|3|6", ["river"], "River: auf einen Einsatz reagieren", "ab 40 BB · 6 Spieler", 23, 9, 14.6, 0.817, "value-bluff"),
  spot("vs_limp|SB|3|6", ["vs_limp"], "Wenn einer nur mitgeht (Limp) im Small Blind", "ab 40 BB · 6 Spieler", 31, 9, 11.2, 1, "range"),
  {
    ...base,
    id: "leak:fold_to_steal|tournament|SB,BB|1",
    kind: "leak",
    title: "Im Small Blind und im Big Blind: Du gibst deine Blinds zu oft her",
    why: "Du liegst bei 77 %, üblich sind 50–70 % (10–20 BB, Turnier).",
    number: "kostet dich im Schnitt etwa 0,1 BB pro Hand, 1240 Hände in dieser Lage",
    term: "fold_to_steal",
    per_time_bb: 0.116,
    times: 1240,
    total_bb: 143.8,
    trust: 0.249,
    estimate: true,
    score: 10.8,
    practice: { kind: "push_fold_call", level: 1 },
    hands: { by: "stat", stat: "fold_to_steal", made: true, positions: ["SB", "BB"], band: 1, game: "tournament", label: "Fold auf Steal · SB, BB · 10–20 BB" },
    read: "fold-steal",
  },
  spot("push_fold|BTN|1|6", ["open", "steal", "shove"], "All-in oder Fold als Erster am Button", "10–20 BB · 6 Spieler", 44, 6, 8.4, 1, "push-fold", 0.977),
  spot("turn_bet|-|3|6", ["turn"], "Turn: setzen oder checken", "ab 40 BB · 6 Spieler", 19, 5, 6.1, 0.881, "equity"),
  spot("first_in|BTN|3|6", ["open", "steal", "shove"], "Eröffnen am Button", "ab 40 BB · 6 Spieler", 52, 3, 2.3, 1, "steal"),
  {
    ...base,
    id: "decision:249999999996:6",
    kind: "decision",
    title: "Teure Hand: Call 12 BB statt Fold",
    why: "Gegen diesen All-in liegt deine Hand zu oft hinten.",
    number: "hat dich etwa 1,7 BB gekostet, eine einzelne Hand",
    per_time_bb: 1.7,
    times: 1,
    total_bb: 1.7,
    trust: 0.35,
    score: 0.58,
    spots: ["call_shove"],
    hand_id: 249999999996,
    hands: { by: "decision", hand_id: 249999999996, action_index: 6 },
    read: "push-fold",
  },
  {
    ...base,
    id: "review:outs",
    kind: "review",
    title: "Wiederholen: Outs zählen",
    why: "Zuletzt vor 4 Tagen geübt – eine kurze Runde, damit es sitzt.",
    number: "80 % richtig in den letzten Übungen",
    score: 0.4,
    practice: { kind: "outs", level: 2 },
  },
];

const params = () => (typeof window === "undefined" ? null : new URLSearchParams(window.location.search));

export function demoNextSteps(): NextStep[] {
  const p = params();
  if (p?.get("data") === "empty" || p?.get("profile") === "0") return [];
  return structuredClone(STEPS);
}

/**
 * As `within` in crates/ringside-core/src/next.rs: the demo list, kept where it touched a part of
 * the history (`kinds`: its loss per kind of spot), then that part's rarer kinds.
 */
export function demoWithin(kinds: KindLoss[], part: string): NextStep[] {
  const keys = new Set(kinds.filter((k) => k.loss_bb > 0).map((k) => k.key));
  const kept = demoNextSteps().filter((s) => s.spot_key !== null && keys.has(s.spot_key));
  const rare = kinds
    .filter((k) => k.loss_bb > 0 && !kept.some((s) => s.spot_key === k.key))
    .map((k): NextStep => ({ ...spot(k.key, [], k.title, k.detail, k.decisions, k.flawed, k.loss_bb, 1, "range"), number: `kostet dich im Schnitt etwa ${bbText(k.loss_bb / k.decisions)} pro Mal, ${k.decisions}-mal ${part}` }));
  return [...kept, ...rare];
}
