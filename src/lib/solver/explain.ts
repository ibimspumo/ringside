// Plain-language answers for players who are getting good, not for solver experts: what the
// solution does in one line, why in one sentence, and the words it uses.
import { isAggressive, sizePct, toCall } from "./play";
import type { SolverAction, SolverHand, SolverNode } from "./types";

export const GLOSSARY = {
  oop: "OOP („out of position“): handelt nach dem Flop auf jeder Straße zuerst und sieht vorher nicht, was der Gegner tut – ein Nachteil.",
  ip: "IP („in position“): handelt nach dem Flop als Letzter und weiß schon, was der Gegner getan hat – ein Vorteil.",
  spr: "SPR (Stack-to-Pot-Ratio): effektiver Stack geteilt durch den Pot. Unter 3 ist mit ein, zwei Einsätzen alles drin; über 8 bleibt viel Spielraum für Turn und River.",
  effective: "Effektiver Stack: der kleinere der beiden Stacks. Mehr kann in dieser Hand keiner gewinnen oder verlieren.",
  equity: "Equity: wie oft eine Hand den Pot gewinnen würde, wenn jetzt alle Karten ohne weitere Einsätze aufgedeckt würden – gegen alle Hände, die der Gegner hier haben kann.",
  ev: "EV (Erwartungswert): was eine Hand oder Aktion von hier an im Schnitt einbringt, in Big Blinds. Höher ist besser; Folden hat EV 0.",
  range: "Range: alle Hände, die ein Spieler an dieser Stelle haben kann, gewichtet danach, wie oft er bis hierher so spielt.",
  exploit:
    "Genauigkeit der Lösung: So viel (in % des Pots) könnte ein perfekter Gegner gegen sie im Schnitt mehr gewinnen. Je kleiner, desto genauer; unter 1 % ist zum Lernen mehr als genug.",
  evloss: "EV-Verlust: wie viel deine Wahl im Schnitt weniger einbringt als die beste Aktion mit genau deiner Hand.",
  potodds: "Pot Odds: wie viel Equity ein Call mindestens braucht – der Call geteilt durch den Pot nach dem Call.",
  threebet:
    "3-Bet: die zweite Erhöhung vor dem Flop – jemand erhöht gegen einen Raise. Darauf folgt die 4-Bet. Wer 3-bettet, hat meist eine starke Hand oder blufft gezielt mit Händen, die gut blocken.",
  mixed:
    "Gemischt: Die Lösung spielt dieselbe Hand mal so, mal so. Beide Aktionen sind dann etwa gleich viel wert; das Mischen macht dich für den Gegner schwer lesbar.",
} as const;

export type Term = keyof typeof GLOSSARY;

/** How often, in words. */
export function often(f: number): string {
  if (f >= 0.97) return "immer";
  if (f >= 0.85) return "fast immer";
  if (f >= 0.6) return "meistens";
  if (f >= 0.35) return "oft";
  if (f >= 0.12) return "manchmal";
  return "selten";
}

/** Bet sizes in words, from the share of the pot. */
export function sizeWord(pct: number): string {
  if (pct < 45) return "klein";
  if (pct < 85) return "mittel";
  if (pct < 125) return "groß";
  return "sehr groß";
}

/** The action as a verb, e.g. "klein setzen", "callen". */
export function verb(a: SolverAction, node: SolverNode): string {
  switch (a.kind) {
    case "fold":
      return "folden";
    case "check":
      return "checken";
    case "call":
      return "callen";
    case "bet":
      return `${sizeWord(sizePct(a, node))} setzen`;
    case "raise":
      return `${sizeWord(sizePct(a, node))} erhöhen`;
    case "all_in":
      return "all-in gehen";
  }
}

/** "Hier meistens checken, manchmal klein setzen." from the range's frequencies. */
export function headline(node: SolverNode): string {
  const ranked = node.actions
    .map((a, k) => ({ a, f: a.frequency, k }))
    .filter((x) => x.f >= 0.03)
    .sort((x, y) => y.f - x.f)
    .slice(0, 3);
  if (!ranked.length) return "";
  return `Hier ${ranked.map((x) => `${often(x.f)} ${verb(x.a, node)}`).join(", ")}.`;
}

type Weighted = { equity: number; w: number; strategy: number[] };

function members(node: SolverNode): Weighted[] {
  if (node.hands.length) return node.hands.map((h) => ({ equity: h.equity, w: h.weight, strategy: h.strategy }));
  return node.classes.map((c) => ({ equity: c.equity, w: c.combos, strategy: c.strategy }));
}

/** Share of the combos taking `pick` that are strong (≥ 60 % equity) and weak (< 35 %). */
function split(list: Weighted[], pick: (k: number) => boolean): { strong: number; weak: number; total: number } {
  let strong = 0;
  let weak = 0;
  let total = 0;
  for (const h of list) {
    const w = h.w * h.strategy.reduce((s, x, k) => s + (pick(k) ? x : 0), 0);
    total += w;
    if (h.equity >= 0.6) strong += w;
    else if (h.equity < 0.35) weak += w;
  }
  return total > 0 ? { strong: strong / total, weak: weak / total, total } : { strong: 0, weak: 0, total: 0 };
}

/** One sentence on why the range splits the way it does. */
export function why(node: SolverNode): string {
  if (node.kind !== "action") return "";
  const list = members(node);
  const aggressive = (k: number) => isAggressive(node.actions[k].kind);
  const share = node.actions.reduce((s, a) => s + (isAggressive(a.kind) ? a.frequency : 0), 0);
  const call = toCall(node);
  if (call > 0) {
    const need = Math.round((call / (node.pot + call)) * 100);
    const raise = share >= 0.05 ? " Erhöht wird mit den stärksten Händen und ein paar Bluffs." : "";
    return `Ein Call muss rund ${need} % der Zeit gewinnen, damit er sich lohnt (Pot Odds) – deshalb callen Hände mit genug Equity, die schwächsten folden.${raise}`;
  }
  if (share < 0.1) return "Fast die ganze Range checkt: Mit kaum einer Hand bringt Setzen hier mehr als Checken.";
  const s = split(list, aggressive);
  const value = s.strong >= s.weak;
  const main = value ? "starken Händen, die von schlechteren bezahlt werden wollen" : "schwachen Händen als Bluff, die beim Showdown selten gewinnen";
  const also = Math.min(s.strong, s.weak) >= 0.15 ? (value ? " und einigen Bluffs" : " und einigen starken Händen") : "";
  return `Gesetzt wird vor allem mit ${main}${also}; mittelstarke Hände checken lieber und kommen günstig zum Showdown.`;
}

/** What the solution does with one hand class, and whether the choice is close. */
export function handAdvice(node: SolverNode, strategy: number[], hands: SolverHand[]): { line: string; close: string | null } {
  const ranked = strategy
    .map((f, k) => ({ f, k }))
    .filter((x) => x.f >= 0.03)
    .sort((x, y) => y.f - x.f)
    .slice(0, 2);
  const line = ranked.map((x) => `${often(x.f)} ${verb(node.actions[x.k], node)}`).join(", sonst ");
  if (ranked.length < 2 || !hands.length || hands[0].evs.length !== strategy.length) return { line, close: null };
  const avg = (k: number) => {
    const w = hands.reduce((s, h) => s + h.weight, 0) || 1;
    return hands.reduce((s, h) => s + h.weight * h.evs[k], 0) / w;
  };
  const [a, b] = ranked;
  const diff = avg(a.k) - avg(b.k);
  const close =
    Math.abs(diff) <= node.pot * 0.01
      ? "Beides ist etwa gleich viel wert – deshalb mischt die Lösung."
      : `${cap(verb(node.actions[diff > 0 ? a.k : b.k], node))} bringt im Schnitt ${fmt(Math.abs(diff))} BB mehr als ${verb(node.actions[diff > 0 ? b.k : a.k], node)}.`;
  return { line, close };
}

/** Why the best action beats the one chosen, for the practice feedback. */
export function lesson(node: SolverNode, hand: SolverHand, best: number): string {
  const eq = Math.round(hand.equity * 100);
  const a = node.actions[best];
  const call = toCall(node);
  const need = Math.round((call / (node.pot + call)) * 100);
  switch (a.kind) {
    case "fold":
      return `Für einen Call bräuchtest du rund ${need} % Equity, deine Hand hat gegen die Range des Gegners nur etwa ${eq} % – Folden spart Chips.`;
    case "call":
      return `Ein Call braucht rund ${need} % Equity, deine Hand hat etwa ${eq} % – genug, um weiterzuspielen.`;
    case "check":
      return hand.equity >= 0.6
        ? `Mit ${eq} % Equity bist du vorne, aber Checken holt hier mehr: Schwächere Hände des Gegners setzen dann oft selbst oder bleiben im Spiel.`
        : hand.equity >= 0.35
          ? `Mit ${eq} % Equity ist deine Hand mittelstark: Sie gewinnt oft beim Showdown, will aber keinen großen Pot gegen bessere Hände.`
          : `Mit ${eq} % Equity gewinnst du selten beim Showdown, und ein Bluff bringt hier zu wenige Folds – Checken kostet am wenigsten.`;
    default:
      return hand.equity >= 0.6
        ? `Mit ${eq} % Equity bist du meist vorne – ${verb(a, node)} holt Chips von schlechteren Händen, die mitgehen.`
        : hand.equity < 0.35
          ? `Mit ${eq} % Equity gewinnst du selten beim Showdown – ${verb(a, node)} als Bluff kann den Gegner zum Folden bringen.`
          : `Mit ${eq} % Equity ist ${verb(a, node)} hier etwas mehr wert: Du schützt deine Hand und baust einen Pot, den du oft gewinnst.`;
  }
}

const fmt = (x: number) => x.toLocaleString("de-DE", { maximumFractionDigits: 2 });
export const cap = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);

/** The solve's precision in words, from its exploitability in % of the pot. */
export function precision(pct: number): string {
  if (pct <= 0.6) return "sehr genau";
  if (pct <= 1.5) return "genau";
  if (pct <= 4) return "grob";
  return "ungenau";
}
