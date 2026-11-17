import type { LearnTarget } from "../app/Leaks.svelte";
import type { Rating, Source, SpotType } from "./types";

export const RATINGS: Rating[] = ["best", "good", "inaccuracy", "mistake", "blunder"];

export const RATING: Record<Rating, { label: string; color: string }> = {
  best: { label: "Beste Wahl", color: "var(--pos)" },
  good: { label: "Gut", color: "color-mix(in srgb, var(--pos) 55%, var(--text-2))" },
  inaccuracy: { label: "Ungenau", color: "var(--amber)" },
  mistake: { label: "Fehler", color: "color-mix(in srgb, var(--amber) 45%, var(--neg))" },
  blunder: { label: "Grober Fehler", color: "var(--neg)" },
};

/** How the ratings are cut, in words. */
export const RATING_RULE =
  "Beste Wahl unter 0,05 BB Verlust, gut unter 0,15 BB, ungenau unter 0,5 BB, Fehler unter 1,5 BB, darüber grober Fehler. Spielt die Berechnung deine Aktion selbst in mindestens 10 % der Fälle, ist sie mindestens „gut“ und kostet nichts: gemischte Strategien sind kein Fehler.";

export const SPOT: Record<SpotType, string> = {
  open: "Eröffnen",
  steal: "Steal",
  three_bet: "3-Bet",
  vs_open: "Gegen einen Raise",
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
  vs_open: "Vor dir hat jemand erhöht oder gelimpt: Fold oder Call.",
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
  { label: "Vor dem Flop", spots: ["open", "steal", "three_bet", "vs_open", "vs_three_bet"] },
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
};

export const EV_LOSS_EXPLAIN =
  "EV-Verlust: wie viele Big Blinds du mit deiner Aktion im Schnitt weniger gewinnst als mit der besten – auf lange Sicht, nicht in dieser einen Hand.";

/** Where to practise or read up on a spot type. */
export const LEARN: Record<SpotType, { label: string; target: LearnTarget }[]> = {
  open: [{ label: "Wissen: Eröffnen", target: { article: "pfr" } }],
  steal: [{ label: "Wissen: Steal", target: { article: "steal" } }],
  three_bet: [{ label: "Wissen: 3-Bet", target: { article: "three-bet" } }],
  vs_open: [{ label: "Wissen: Ranges", target: { article: "range" } }],
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
