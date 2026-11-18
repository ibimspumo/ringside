// Sentences of the session review: luck in one line, and why a decision was smart or not.
import { bb, int, num } from "../ui/format";
import type { SessionReport, Swing } from "./types";

export const cardsOf = (text: string) => text.split(/\s+/).filter(Boolean);

/** The session's all-in luck in one sentence. */
export function luckLine(l: SessionReport["luck"]): string {
  if (!l.all_ins) return "Kein All-in mit offenen Karten – Glück hat dein Ergebnis kaum verschoben.";
  const n = l.all_ins === 1 ? "einem All-in" : `${int(l.all_ins)} All-ins`;
  if (Math.abs(l.luck_bb) < 1) return `In ${n} hielten sich Glück und Pech die Waage.`;
  const what = l.luck_bb < 0 ? "Pech" : "Glück";
  return `Insgesamt hattest du diese Session etwa ${bb(Math.abs(l.luck_bb))} ${what} in ${n}. Mit normalem Glück stündest du bei ${bb(l.ev_bb, { sign: true })}.`;
}

/** Why the decision was right or wrong, judged when it was made (against the range). */
export function swingLine(s: Swing, right: boolean): string {
  if (right) {
    const gain = s.margin_bb !== null && s.margin_bb > 0.05 && s.alternative ? ` – auf Dauer bringt das ${bb(s.margin_bb, { sign: true })} mehr als ${s.alternative}` : "";
    return `Gegen seine Range richtig${gain}.`;
  }
  return `Gewonnen, aber auf Dauer kostet das etwa ${bb(s.loss_bb)}${s.best ? ` – besser ${s.best}` : ""}.`;
}

/** What happened afterwards, with the cards shown: extra information, never the judgement. */
export function outcomeLine(s: Swing, right: boolean): string {
  const eq = s.equity_pct;
  if (right) {
    if (eq === null) return `Diesmal verloren (${bb(s.net_bb, { sign: true })}) – das passiert, die Entscheidung bleibt richtig.`;
    if (eq < 50) return `Gegen seine gezeigten Karten hattest du nur ${num(eq, 0)} % – er hatte diesmal eine der wenigen starken Hände.`;
    return `Mit ${num(eq, 0)} % Gewinnchance vorn und trotzdem verloren (${bb(s.net_bb, { sign: true })}).`;
  }
  if (eq !== null && eq < 50) return `Gegen seine gezeigten Karten hattest du nur ${num(eq, 0)} % – hier hattest du Glück.`;
  return `Diesmal ging es gut (${bb(s.net_bb, { sign: true })}) – hier hattest du Glück.`;
}
