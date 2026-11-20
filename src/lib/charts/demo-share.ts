// Browser demo only: the chart samples are one table at one depth. Seated at another table or
// depth, a spot's grid is reshaped to the share played there, so the sentence above the grid,
// the grid and the table-size comparison tell the same number.
import type { PreflopAnte, TableOpens } from "./preflop-types";

/** The real comparison across table sizes, written by `demo_table_opens_are_current` in src-tauri/src/charts.rs. */
export async function tableOpens(ante: PreflopAnte, stackBb: number, pushfold: boolean): Promise<TableOpens[]> {
  const all: Record<string, TableOpens[]> = (await import("./sizes-demo.json")).default;
  return all[`${ante}|${stackBb}|${pushfold ? "pushfold" : "model"}`] ?? [];
}

/** Share played by `position` when everyone before folded, from the real comparison; null off its grid. */
export async function realShare(ante: PreflopAnte, players: number, stackBb: number, position: string, pushfold: boolean): Promise<number | null> {
  const row = (await tableOpens(ante, stackBb, pushfold)).find((r) => r.players === players);
  const k = row ? row.positions.indexOf(position) : -1;
  return row && k >= 0 && k < row.pct.length ? row.pct[k] : null;
}

/** Combos of a hand class (row * 13 + col, ace first, suited above the diagonal). */
export const combos = (i: number) => (Math.floor(i / 13) === i % 13 ? 6 : Math.floor(i / 13) < i % 13 ? 4 : 12);

/** Percent of all 1326 combos a grid of 0–255 frequencies plays. */
export function shareOf(freq: number[]): number {
  return (100 * freq.reduce((t, f, i) => t + (combos(i) * f) / 255, 0)) / 1326;
}

/**
 * New play shares (0–1) per class for `target` percent of all combos: the classes keep their
 * order (most played first, ties by `key`), the edge between played and folded stays soft.
 */
export function reshape(played: number[], key: number[], target: number, soft = 40): number[] {
  const order = played.map((_, i) => i).sort((a, b) => played[b] - played[a] || key[b] - key[a]);
  const middle = new Array<number>(played.length);
  let seen = 0;
  for (const i of order) {
    middle[i] = seen + combos(i) / 2;
    seen += combos(i);
  }
  const at = (x: number) => middle.map((m) => Math.max(0, Math.min(1, (x - m) / soft + 0.5)));
  const want = (target / 100) * 1326;
  let lo = -soft;
  let hi = 1326 + soft;
  for (let k = 0; k < 60; k++) {
    const x = (lo + hi) / 2;
    if (at(x).reduce((t, p, i) => t + p * combos(i), 0) < want) lo = x;
    else hi = x;
  }
  return at((lo + hi) / 2);
}
