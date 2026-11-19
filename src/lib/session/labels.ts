// Sentences of the session review: luck in one line, and why a decision was smart or not.
import { bb, int, num } from "../ui/format";
import type { SessionReport, Swing } from "./types";

export const cardsOf = (text: string) => text.split(/\s+/).filter(Boolean);

/** An example of a hand class as two cards: "K9s" → K♠ 9♠, "K9o" → K♠ 9♥, "88" → 8♠ 8♥. */
export function classCards(name: string): [string, string] {
  const [hi, lo] = [name[0], name[1]];
  return [`${hi}s`, `${lo}${name[2] === "s" ? "s" : "h"}`];
}

/** "K9 in gleicher Farbe", "K9 in verschiedenen Farben", "Paar 8". */
export function classWords(name: string): string {
  if (name.length === 2) return `Ein Paar ${name[0]} – jede Farbe`;
  return `${name.slice(0, 2)} in ${name[2] === "s" ? "gleicher Farbe" : "verschiedenen Farben"} – jede Farbe`;
}

/** The session's all-in luck in one sentence. */
export function luckLine(l: SessionReport["luck"]): string {
  if (!l.all_ins) return "Kein All-in mit offenen Karten – Glück hat dein Ergebnis kaum verschoben.";
  const n = l.all_ins === 1 ? "einem All-in" : `${int(l.all_ins)} All-ins`;
  if (Math.abs(l.luck_bb) < 1) return `In ${n} hielten sich Glück und Pech die Waage.`;
  const what = l.luck_bb < 0 ? "Pech" : "Glück";
  return `Insgesamt hattest du diese Session etwa ${bb(Math.abs(l.luck_bb))} ${what} in ${n}. Ohne dieses ${what} stündest du in Chips rechnerisch bei ${bb(l.ev_bb, { sign: true })}.`;
}

/**
 * Why the decision was right or wrong, judged when it was made (against the hands he can have).
 * `lead` starts with the verdict, for places that do not show it already.
 */
export function swingLine(s: Swing, right: boolean, lead = true): string {
  if (right) {
    const gain = s.margin_bb !== null && s.margin_bb > 0.05 && s.alternative ? ` – auf Dauer bringt das ${bb(s.margin_bb, { sign: true })} mehr als ${s.alternative}` : "";
    return `${lead ? "Schlau entschieden – g" : "G"}egen die Hände, die er hier haben kann, richtig${gain}.`;
  }
  return `${lead ? "Nicht schlau, auch wenn's gut ging: a" : "A"}uf Dauer kostet das etwa ${bb(s.loss_bb)}${s.best ? ` – besser ${s.best}` : ""}.`;
}

/** What happened afterwards, with the cards shown: extra information, never the judgement. */
export function outcomeLine(s: Swing, right: boolean): string {
  const eq = s.equity_pct;
  if (right) {
    if (eq === null) return "Diesmal verloren – das passiert, die Entscheidung bleibt richtig.";
    if (eq < 50) return `Gegen seine gezeigten Karten hattest du nur ${num(eq, 0)} % – er hatte diesmal eine der wenigen starken Hände.`;
    return `Mit ${num(eq, 0)} % Gewinnchance vorn und trotzdem verloren.`;
  }
  if (eq !== null && eq < 50) return `Gegen seine gezeigten Karten hattest du nur ${num(eq, 0)} % – hier hattest du Glück.`;
  return "Diesmal ging es gut – hier hattest du Glück.";
}
