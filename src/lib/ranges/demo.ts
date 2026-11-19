// Browser preview of "Deine Ranges": a made-up but plausible tournament player who opens a
// little too tight from late position and too loose from early position.
import type { Filter } from "../types";
import { POSITIONS } from "./select";
import type { History, ModelRange, RangeChart, RangeCounts, RangeFilter, RangeGroup } from "./types";

function rng(seed: number) {
  let x = seed >>> 0 || 1;
  return () => {
    x ^= x << 13;
    x ^= x >>> 17;
    x ^= x << 5;
    return (x >>> 0) / 4294967296;
  };
}

const RANKS = 13;
/** A rough preflop strength per class (row * 13 + col, ace first, suited above). */
function strength(i: number): number {
  const r = Math.floor(i / RANKS);
  const c = i % RANKS;
  const hi = 12 - Math.min(r, c);
  const lo = 12 - Math.max(r, c);
  if (r === c) return 0.55 + hi * 0.037;
  const suited = r < c;
  const gap = hi - lo;
  return (hi * 0.034 + lo * 0.021 + (suited ? 0.07 : 0) - Math.min(gap, 5) * 0.018 + (hi === 12 ? 0.06 : 0)) * 1.08;
}
const combos = (i: number) => (Math.floor(i / 13) === i % 13 ? 6 : Math.floor(i / 13) < i % 13 ? 4 : 12);
const ORDER = Array.from({ length: 169 }, (_, i) => i).sort((a, b) => strength(b) - strength(a));

/** Play share per class for the top `pct` of combos, with a soft edge. */
function topRange(pct: number, soft = 0.04): number[] {
  const out = new Array(169).fill(0);
  let seen = 0;
  for (const i of ORDER) {
    const at = seen / 1326;
    out[i] = Math.max(0, Math.min(1, (pct - at) / soft + 0.5));
    seen += combos(i);
  }
  return out;
}

const OPEN: Record<string, number> = { UTG: 0.14, "UTG+1": 0.16, LJ: 0.2, HJ: 0.25, CO: 0.31, BTN: 0.47, SB: 0.42, BB: 0.3 };
const REPLY: Partial<Record<History, number>> = { limped: 0.34, vs_open: 0.16, squeeze: 0.1, vs_three_bet: 0.12, vs_shove: 0.11 };
const YOU_BIAS: Record<string, number> = { UTG: 1.25, "UTG+1": 1.2, LJ: 1.05, HJ: 0.92, CO: 0.8, BTN: 0.66, SB: 0.7, BB: 0.9 };

function base(h: History | null, position: string | null): number {
  const p = position ?? "CO";
  return h === "unopened" || h === null ? (OPEN[p] ?? 0.25) : (REPLY[h] ?? 0.15) * (p === "BB" ? 1.6 : 1);
}

export function demoGroups(filter: Filter): RangeGroup[] {
  const r = rng(97 + (filter.tournament_id ?? 0) + (filter.since ? 5 : 0) + (filter.game === "cash" ? 11 : 0));
  const scale = filter.since ? 0.35 : 1;
  const out: RangeGroup[] = [];
  const seats = ["UTG", "UTG+1", "LJ", "HJ", "CO", "BTN", "SB", "BB"];
  const weight: Record<History, number> = { unopened: 1, limped: 0.12, vs_open: 0.5, squeeze: 0.1, vs_three_bet: 0.12, vs_shove: 0.18 };
  for (const history of Object.keys(weight) as History[]) {
    for (const [si, position] of seats.entries()) {
      if (history === "unopened" && position === "BB") continue;
      const before = seats.slice(0, si);
      const opponents: (string | null)[] = history === "unopened" ? [null] : history === "vs_three_bet" ? seats.slice(si + 1) : before;
      if (!opponents.length) continue;
      for (const vs of opponents) {
        for (let k = 0; k < 9; k++) {
          const stack = Math.min(60, Math.round(6 * Math.exp(r() * 2.3)));
          const count = Math.round((r() * 160 + 30) * weight[history] * scale * (vs ? 1 / opponents.length + 0.2 : 1));
          if (count <= 0) continue;
          out.push({
            history,
            position,
            vs_position: vs,
            stack,
            players: r() < 0.6 ? 8 : r() < 0.6 ? 9 : 6 + Math.floor(r() * 2),
            behind: position === "BB" ? null : r() < 0.18 ? 0 : r() < 0.5 ? 1 : 2,
            count,
          });
        }
      }
    }
  }
  return out.filter((g) => POSITIONS.includes(g.position));
}

function demoModel(range: RangeFilter, stack: number): ModelRange {
  const pushFold = range.history === "unopened" && stack < 13;
  const play = topRange(base(range.history, range.position) * (pushFold ? 0.9 : 1));
  const raise = range.history === "vs_shove" ? 0 : 1;
  const cells = play.map((p, i) => {
    if (pushFold) return [1 - p, 0, 0, p] as [number, number, number, number];
    if (range.history === "vs_shove") return [1 - p, p, 0, 0] as [number, number, number, number];
    const call = range.history === "unopened" ? 0 : p * (0.55 - strength(i) * 0.4);
    return [1 - p, Math.max(0, call), Math.max(0, p - call) * raise, 0] as [number, number, number, number];
  });
  const stackBb = pushFold ? Math.round(stack) : [15, 20, 25, 30, 40, 50, 75, 100].reduce((a, b) => (Math.abs(b - stack) < Math.abs(a - stack) ? b : a));
  return {
    source: pushFold ? "push_fold" : "model",
    players: 8,
    stack_bb: stackBb,
    ante: "big_blind",
    cells,
    raise_to_bb: pushFold ? 0 : 2.2,
    all_in_bb: stackBb,
    notes: [
      pushFold
        ? `Push/Fold-Gleichgewicht (nur All-in oder Fold): 8 Spieler, alle mit ${stackBb} BB, Big-Blind-Ante`
        : `Preflop-Modell: 8 Spieler, alle mit ${stackBb} BB, Big-Blind-Ante`,
      `8 Spieler am Tisch: so war es bei 61 % deiner Hände hier`,
      `nächste vorgerechnete Tiefe ${stackBb} BB statt ${Math.round(stack)} BB (typische Tiefe deiner Hände)`,
    ],
  };
}

export function demoChart(filter: Filter, range: RangeFilter): RangeChart {
  const groups = demoGroups(filter).filter(
    (g) =>
      (!range.history || g.history === range.history) &&
      (!range.position || g.position === range.position) &&
      (!range.vs_position || g.vs_position === range.vs_position) &&
      (range.min_bb === null || g.stack >= range.min_bb) &&
      (range.max_bb === null || g.stack < range.max_bb),
  );
  const total = groups.reduce((n, g) => n + g.count, 0);
  const stacks = groups.flatMap((g) => Array(g.count).fill(g.stack)).sort((a, b) => a - b);
  const stack = stacks.length ? stacks[Math.floor(stacks.length / 2)] : 25;
  const r = rng(13 + total);
  const bias = YOU_BIAS[range.position ?? "CO"] ?? 1;
  const you = topRange(base(range.history, range.position) * bias, 0.07);
  const locked = new URLSearchParams(window.location.search).get("locked") === "1";
  const model = range.position && !locked ? demoModel(range, stack) : null;
  // Deal exactly `total` hands over the classes, in proportion to their combos.
  const dealt = new Array(169).fill(0);
  for (let k = 0; k < total; k++) {
    let x = r() * 1326;
    let i = 0;
    while (i < 168 && x >= combos(i)) x -= combos(i++);
    dealt[i]++;
  }
  const cells = Array.from({ length: 169 }, (_, i): [number, number, number, number] => {
    const n = dealt[i];
    let play = 0;
    for (let k = 0; k < n; k++) play += r() < you[i] ? 1 : 0;
    const shove = range.history === "unopened" && stack < 13 ? play : Math.round(play * (strength(i) > 0.9 ? 0.3 : 0));
    const call = range.history === "vs_shove" ? play : range.history === "unopened" ? 0 : Math.round((play - shove) * 0.4);
    return [n - play, call, play - shove - call, shove];
  });
  const counts: RangeCounts = {
    total: cells.reduce((s, c) => s + c[0] + c[1] + c[2] + c[3], 0),
    cells,
    stack_bb: stack,
    players: [8, Math.round(total * 0.61)],
    ante: ["big_blind", total],
    line: [range.vs_position ? `${range.vs_position}:r` : "", total],
  };
  if (!model) {
    const why = locked ? "Der Vergleich ist gesperrt, solange ein Poker-Client läuft." : "Wähle eine Position, dann steht das Modell daneben.";
    return { counts, model: null, comparison: null, model_missing: why, locked };
  }
  let w = 0;
  let mp = 0;
  const gaps: [number, number][] = [];
  cells.forEach((c, i) => {
    const m = model.cells[i];
    const played = m ? 1 - m[0] : 0;
    w += combos(i);
    mp += combos(i) * played;
    const n = c[0] + c[1] + c[2] + c[3];
    if (n) gaps.push([(played - (n - c[0]) / n) * n, i]);
  });
  const name = (i: number) => {
    const RANK = "AKQJT98765432";
    const a = Math.floor(i / 13);
    const b = i % 13;
    return a === b ? RANK[a] + RANK[b] : a < b ? `${RANK[a]}${RANK[b]}s` : `${RANK[b]}${RANK[a]}o`;
  };
  const missing = gaps.filter((g) => g[0] >= 0.5).sort((a, b) => b[0] - a[0]).slice(0, 5).map((g) => name(g[1]));
  const extra = gaps.filter((g) => g[0] <= -0.5).sort((a, b) => a[0] - b[0]).slice(0, 4).map((g) => name(g[1]));
  return {
    counts,
    model,
    comparison: {
      you_pct: (100 * (counts.total - cells.reduce((s, c) => s + c[0], 0))) / Math.max(1, counts.total),
      model_pct: (100 * mp) / w,
      missing,
      extra,
    },
    model_missing: null,
    locked,
  };
}
