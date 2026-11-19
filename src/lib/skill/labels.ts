import type { LearnTarget } from "../app/Leaks.svelte";
import type { SkillArea } from "./types";

/** A rating as players know it from chess: whole points, no thousands dot ("1287"). */
export const points = (r: number) => String(Math.round(r));

/** "+14", "−6", "±0". */
export function delta(d: number): string {
  const n = Math.round(d);
  return n === 0 ? "±0" : `${n > 0 ? "+" : "−"}${Math.abs(n)}`;
}

/** The uncertainty as players read it: "± 80", rounded to tens. */
export const spreadOf = (sigma: number) => Math.max(10, Math.round(sigma / 10) * 10);
export const spread = (sigma: number) => `± ${spreadOf(sigma)}`;

/** What to practise for an area, where the practice has it. */
export const PRACTICE: Partial<Record<SkillArea, LearnTarget>> = {
  push_fold: { kind: "push_fold_open", level: 1 },
  vs_all_in: { kind: "push_fold_call", level: 1 },
};

/** "4 Hände" style counts. */
export const decisions = (n: number) => (n === 1 ? "1 Entscheidung" : `${n.toLocaleString("de-DE")} Entscheidungen`);

/** The hero's cards of a stored hand, "AhKd" or "Ah Kd". */
export const cardsOf = (s: string) => s.replace(/\s+/g, "").match(/.{2}/g) ?? [];

/** What an area means, in the words of a casual player. */
export const AREA_HELP: Record<SkillArea, string> = {
  push_fold: "Alle vor dir haben gefoldet und du hast unter 20 Big Blinds: alles setzen oder weglegen.",
  vs_all_in: "Ein Gegner hat alles gesetzt: mitgehen oder weglegen.",
  open: "Alle vor dir haben gefoldet und du hast 20 Big Blinds oder mehr: als Erster erhöhen oder weglegen.",
  blind_vs_blind: "Nur noch die beiden Blinds sind übrig: Small Blind gegen Big Blind.",
  big_blind: "Du sitzt im Big Blind und jemand hat erhöht oder nur mitgezahlt.",
  vs_open: "Jemand vor dir hat erhöht: mitgehen, selbst erhöhen oder weglegen.",
  vs_three_bet: "Du hast erhöht und ein Gegner hat noch einmal erhöht (3-Bet).",
  postflop: "Alle Entscheidungen auf Flop und Turn, nachdem die ersten Gemeinschaftskarten liegen.",
  river: "Die letzte Entscheidung, wenn alle fünf Karten liegen.",
};
