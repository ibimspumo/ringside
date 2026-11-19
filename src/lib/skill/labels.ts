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
export const spread = (sigma: number) => `± ${Math.max(10, Math.round(sigma / 10) * 10)}`;

/** What to practise for an area, where the practice has it. */
export const PRACTICE: Partial<Record<SkillArea, LearnTarget>> = {
  push_fold: { kind: "push_fold_open", level: 1 },
  vs_all_in: { kind: "push_fold_call", level: 1 },
};

/** "4 Hände" style counts. */
export const decisions = (n: number) => (n === 1 ? "1 Entscheidung" : `${n.toLocaleString("de-DE")} Entscheidungen`);

/** The hero's cards of a stored hand, "AhKd" or "Ah Kd". */
export const cardsOf = (s: string) => s.replace(/\s+/g, "").match(/.{2}/g) ?? [];
