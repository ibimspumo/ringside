import type { LearnTarget } from "../app/Leaks.svelte";
import { THRESHOLD_TEXT } from "../play/labels";
import { GRADE, type Grade, type Source as KitSource } from "../ui/poker";
import type { Rating, Source, SpotType } from "./types";

export const RATINGS: Rating[] = ["best", "mix", "good", "inaccuracy", "mistake", "blunder"];

/** The kit's grade of a stored rating: the same six steps as the practice. */
export const GRADE_OF: Record<Rating, Grade> = {
  best: "best",
  mix: "mix",
  good: "good",
  inaccuracy: "inaccurate",
  mistake: "mistake",
  blunder: "blunder",
};

export const RATING: Record<Rating, { label: string; color: string }> = Object.fromEntries(
  RATINGS.map((r) => [r, { label: GRADE[GRADE_OF[r]].label, color: `var(--grade-${GRADE_OF[r]})` }]),
) as Record<Rating, { label: string; color: string }>;

/** How the ratings are cut, in words: the practice's thresholds. */
export const RATING_RULE = `Bewertet wird in Prozent des Pots, wie in „Hände spielen“. ${RATINGS.map((r) => `${RATING[r].label}: ${THRESHOLD_TEXT[GRADE_OF[r]]}`).join(". ")}. Gemischte Strategien sind nie ein Fehler.`;

export const SPOT: Record<SpotType, string> = {
  open: "Eröffnen",
  steal: "Steal",
  three_bet: "3-Bet",
  vs_open: "Gegen einen Raise",
  vs_limp: "Gegen einen Limp",
  vs_three_bet: "Gegen eine 3-Bet",
  shove: "All-in oder Fold",
  call_shove: "Call gegen All-in",
  c_bet: "C-Bet am Flop",
  vs_c_bet: "Gegen eine C-Bet",
  flop: "Flop",
  turn: "Turn",
  river: "River",
};

export const SPOT_HINT: Record<SpotType, string> = {
  open: "Du bist als Erster drin, aus früher oder mittlerer Position.",
  steal: "Du bist als Erster drin, aus Cutoff, Button oder Small Blind.",
  three_bet: "Du erhöhst einen Raise noch einmal.",
  vs_open: "Vor dir hat jemand erhöht: Fold, Call oder 3-Bet.",
  vs_limp: "Vor dir hat jemand nur gecallt (gelimpt): isolieren, mitlimpen, im Big Blind checken oder all-in.",
  vs_three_bet: "Dein Raise wurde erneut erhöht.",
  shove: "Kurzer Stack: alle haben gefoldet, du gehst all-in oder foldest.",
  call_shove: "Jemand ist all-in gegangen, du callst oder foldest.",
  c_bet: "Du hast vor dem Flop erhöht und setzt am Flop nach – oder checkst.",
  vs_c_bet: "Der Raiser von vor dem Flop setzt am Flop, du reagierst.",
  flop: "Andere Entscheidungen am Flop.",
  turn: "Entscheidungen am Turn.",
  river: "Entscheidungen am River.",
};

export const SPOT_GROUPS: { label: string; spots: SpotType[] }[] = [
  { label: "Vor dem Flop", spots: ["open", "steal", "three_bet", "vs_open", "vs_limp", "vs_three_bet"] },
  { label: "Kurzer Stack", spots: ["shove", "call_shove"] },
  { label: "Nach dem Flop", spots: ["c_bet", "vs_c_bet", "flop", "turn", "river"] },
];

export const SOURCE: Record<Source, { label: string; explain: string }> = {
  equilibrium: {
    label: "Gleichgewicht",
    explain:
      "Die Push/Fold-Strategie, die kein Spieler mehr verbessern kann, wenn nur All-in oder Fold erlaubt ist. Gerechnet in Chips, ohne Preisgeld-Druck (ICM), aus Ringsides Tabellen.",
  },
  model: {
    label: "Modell",
    explain:
      "Ringsides Preflop-Modell: Es rechnet die Aktionen vor dem Flop genau und schätzt den Rest der Hand mit einer Formel. Eine gute Richtschnur, aber kein GTO des echten Spiels.",
  },
  solver: {
    label: "Solver",
    explain:
      "Eine echte Berechnung der Hand ab dem Flop, heads-up, mit Ranges aus dem Preflop-Modell und Standard-Einsatzgrößen.",
  },
  estimate: {
    label: "Einschätzung",
    explain:
      "Für diesen Spot gibt es weder Solver noch Modell, etwa weil drei Spieler den Flop sehen. Ringside schätzt: Wie oft gewinnt deine Hand gegen die wahrscheinlichen Hände der Gegner, was kostet der Call (Pot Odds), wie oft folden sie auf einen Bet? Gut als Richtung, nicht exakt.",
  },
  forced: {
    label: "Keine Entscheidung",
    explain: "Hier gab es nichts zu entscheiden: Alle anderen waren schon all-in, oder der Call der letzten Chips war mit jeder Hand richtig. Zählt nicht in deine Bewertung.",
  },
};

/** Sources a filter offers: forced moves are no decisions. */
export const FILTER_SOURCES: Source[] = ["solver", "model", "equilibrium", "estimate"];

/** The kit's source mark of a stored source; forced moves have none. */
export const KIT_SOURCE: Record<Source, KitSource | null> = {
  equilibrium: "equilibrium",
  model: "model",
  solver: "solver",
  estimate: "estimate",
  forced: null,
};

/** The estimate's reliability in words: how often it was at most one step off the reference. */
export function reliabilityText(share: number, street: "preflop" | "flop" | "turn" | "river"): string {
  const ref = street === "preflop" ? "dem Preflop-Modell" : "dem Solver";
  return `Verlässlichkeit ${Math.round(share * 100)} %: So oft lag die Einschätzung in Prüf-Spots höchstens eine Stufe neben ${ref} – bei guten Zügen wie bei Fehlern.`;
}

export const EV_LOSS_EXPLAIN =
  "EV-Verlust: wie viele Big Blinds du mit deiner Aktion im Schnitt weniger gewinnst als mit der besten – auf lange Sicht, nicht in dieser einen Hand.";

/**
 * What an action's EV counts from. The solver and the estimate count from the decision on; the
 * preflop model and the push/fold equilibrium against the stack before the hand, so there a fold
 * loses the blind already posted. Within one list the gaps are the same either way.
 */
export const EV_FROM: Record<Source, "now" | "stack"> = { solver: "now", estimate: "now", forced: "now", model: "stack", equilibrium: "stack" };

/** The "bringt" column's legend, one sentence per way of counting. */
export const EV_LEGEND: Record<"now" | "stack", string> = {
  now: "„bringt“: was die Aktion ab hier im Schnitt einbringt, Folden ist 0.",
  stack: "„bringt“: was die Aktion im Schnitt einbringt, gerechnet ab deinem Stack vor der Hand – darum zählt beim Folden ein schon gesetzter Blind als Verlust.",
};

export const EV_EXPLAIN =
  "EV (Erwartungswert): was diese Aktion auf lange Sicht im Schnitt bringt, in Big Blinds. Die Zahlen einer Liste lassen sich direkt vergleichen; zwischen den Quellen nur, was eine Aktion weniger bringt als die beste.";

/** Where to practise or read up on a spot type. */
export const LEARN: Record<SpotType, { label: string; target: LearnTarget }[]> = {
  open: [{ label: "Wissen: Eröffnen", target: { article: "pfr" } }],
  steal: [{ label: "Wissen: Steal", target: { article: "steal" } }],
  three_bet: [{ label: "Wissen: 3-Bet", target: { article: "three-bet" } }],
  vs_open: [{ label: "Wissen: Ranges", target: { article: "range" } }],
  vs_limp: [{ label: "Wissen: Ranges", target: { article: "range" } }],
  vs_three_bet: [{ label: "Wissen: Fold gegen 3-Bet", target: { article: "fold-three-bet" } }],
  shove: [
    { label: "Übung: All-in oder Fold", target: { kind: "push_fold_open", level: 1 } },
    { label: "Wissen: Push/Fold", target: { article: "push-fold" } },
  ],
  call_shove: [
    { label: "Übung: Call gegen All-in", target: { kind: "push_fold_call", level: 1 } },
    { label: "Wissen: Push/Fold", target: { article: "push-fold" } },
  ],
  c_bet: [{ label: "Wissen: C-Bet", target: { article: "cbet" } }],
  vs_c_bet: [{ label: "Wissen: Gegen eine C-Bet", target: { article: "fold-cbet" } }],
  flop: [{ label: "Wissen: Pot Odds", target: { article: "pot-odds" } }],
  turn: [{ label: "Wissen: Equity", target: { article: "equity" } }],
  river: [{ label: "Wissen: Value und Bluff", target: { article: "value-bluff" } }],
};

export function bb(x: number, digits = 1): string {
  return `${x.toLocaleString("de-DE", { maximumFractionDigits: x < 1 ? 2 : digits })} BB`;
}

export function pct(share: number): string {
  return `${Math.round(share * 100)} %`;
}

export function duration(secs: number): string {
  if (secs < 90) return `${Math.max(1, Math.round(secs))} s`;
  if (secs < 5400) return `${Math.round(secs / 60)} min`;
  return `${(secs / 3600).toLocaleString("de-DE", { maximumFractionDigits: 1 })} h`;
}
