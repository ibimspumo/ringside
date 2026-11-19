import type { LearnTarget } from "../app/Leaks.svelte";
import { THRESHOLD_TEXT } from "../play/labels";
import { GRADE, type Grade, type Source as KitSource } from "../ui/poker";
import { WORDS, explain, type WordId } from "../ui/words";
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
  steal: "Blinds angreifen",
  three_bet: "Nochmal erhöhen",
  vs_open: "Gegen einen Raise",
  vs_limp: "Gegen einen Limp",
  vs_three_bet: "Gegen ein Re-Raise",
  shove: "All-in oder Fold",
  call_shove: "Call gegen All-in",
  c_bet: "Am Flop nachsetzen",
  vs_c_bet: "Gegen Nachsetzen am Flop",
  flop: "Flop",
  turn: "Turn",
  river: "River",
};

export const SPOT_HINT: Record<SpotType, string> = {
  open: "Du bist als Erster drin, aus früher oder mittlerer Position.",
  steal: "Du bist als Erster drin, aus Cutoff, Button oder Small Blind – Profis nennen das Steal.",
  three_bet: "Du erhöhst einen Raise noch einmal – Profis nennen das 3-Bet.",
  vs_open: "Vor dir hat jemand erhöht: Fold, Call oder nochmal erhöhen.",
  vs_limp: "Vor dir hat jemand nur gecallt (gelimpt): isolieren, mitlimpen, im Big Blind checken oder all-in.",
  vs_three_bet: "Dein Raise wurde noch einmal erhöht (3-Bet).",
  shove: "Kurzer Stack: alle haben gefoldet, du gehst all-in oder foldest.",
  call_shove: "Jemand ist all-in gegangen, du callst oder foldest.",
  c_bet: "Du hast vor dem Flop erhöht und setzt am Flop nach (C-Bet) – oder checkst.",
  vs_c_bet: "Wer vor dem Flop erhöht hat, setzt am Flop nach (C-Bet); du reagierst.",
  flop: "Andere Entscheidungen am Flop.",
  turn: "Entscheidungen am Turn.",
  river: "Entscheidungen am River.",
};

export const SPOT_GROUPS: { label: string; spots: SpotType[] }[] = [
  { label: "Vor dem Flop", spots: ["open", "steal", "three_bet", "vs_open", "vs_limp", "vs_three_bet"] },
  { label: "Kurzer Stack", spots: ["shove", "call_shove"] },
  { label: "Nach dem Flop", spots: ["c_bet", "vs_c_bet", "flop", "turn", "river"] },
];

/** Where a rating comes from, in the glossary's words. */
const SOURCE_WORD: Record<Source, WordId> = { equilibrium: "equilibrium", model: "model", solver: "solved", estimate: "estimate", forced: "forced" };
export const SOURCE: Record<Source, { label: string; explain: string }> = Object.fromEntries(
  Object.entries(SOURCE_WORD).map(([s, id]) => [s, { label: WORDS[id].word, explain: explain(id) }]),
) as Record<Source, { label: string; explain: string }>;

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

/** The estimate's reliability in words: the lowest share of its grades at most one step off the reference over every check with enough cases. */
export function reliabilityText(share: number, street: "preflop" | "flop" | "turn" | "river"): string {
  const ref = street === "preflop" ? "dem Preflop-Modell" : "dem Solver";
  return `Verlässlichkeit ${Math.round(share * 100)} %: Mindestens so oft lag die Einschätzung in Prüf-Spots höchstens eine Stufe neben ${ref} – bei guten Zügen wie bei Fehlern.`;
}

export const EV_LOSS_EXPLAIN = explain("ev_loss");

/**
 * What an action's EV counts from. The solver and the estimate count from the decision on; the
 * preflop model and the push/fold equilibrium against the stack before the hand, so there a fold
 * loses the blind already posted. Within one list the gaps are the same either way.
 */
export const EV_FROM: Record<Source, "now" | "stack"> = { solver: "now", estimate: "now", forced: "now", model: "stack", equilibrium: "stack" };

/** What the "bringt" column counts, one sentence per way of counting. */
export const EV_LEGEND: Record<"now" | "stack", string> = {
  now: "Was die Aktion ab hier im Schnitt einbringt, in Big Blinds. Folden ist 0.",
  stack: "Was die Aktion im Schnitt einbringt, in Big Blinds, gerechnet ab deinem Stack vor der Hand – darum zählt beim Folden ein schon gesetzter Blind als Verlust.",
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

