// Browser demo of the Spielwertung: a believable history over the demo tournaments.
import { demoTournaments } from "../demo";
import type { AreaScore, SkillArea, SkillChange, SkillEntry, SkillGrades, SkillProfile } from "./types";

const TIERS: [number, string][] = [
  [0, "Einsteiger"],
  [1000, "Auf dem Weg"],
  [1300, "Solide"],
  [1550, "Stark"],
  [1800, "Sehr stark"],
  [2000, "Profi-Niveau"],
];
const tierOf = (r: number) => [...TIERS].reverse().find(([from]) => r >= from)![1];

const AREAS: [SkillArea, string, AreaScore["spots"]][] = [
  ["push_fold", "All-in oder Fold", ["open", "steal", "shove"]],
  ["vs_all_in", "Gegen ein All-in", ["call_shove"]],
  ["open", "Eröffnen", ["open", "steal", "shove"]],
  ["blind_vs_blind", "Blind gegen Blind", ["open", "steal", "shove"]],
  ["big_blind", "Im Big Blind", ["vs_open", "three_bet", "vs_limp"]],
  ["vs_open", "Gegen eine Eröffnung", ["vs_open", "three_bet", "vs_limp"]],
  ["vs_three_bet", "Gegen eine 3-Bet", ["vs_three_bet"]],
  ["postflop", "Flop und Turn", ["c_bet", "vs_c_bet", "flop", "turn"]],
  ["river", "River", ["river"]],
];

function grades(n: number, rnd: () => number, err = 1): SkillGrades {
  const blunder = Math.round(n * 0.03 * err * (0.5 + rnd()));
  const mistake = Math.round(n * 0.03 * err * (0.5 + rnd()));
  const inaccurate = Math.round(n * 0.05 * (0.5 + rnd()));
  const good = Math.round(n * 0.09 * (0.5 + rnd()));
  return { perfect: Math.max(0, n - blunder - mistake - inaccurate - good), good, inaccurate, mistake, blunder };
}

const sum = (gs: SkillGrades[]): SkillGrades =>
  gs.reduce((a, g) => ({ perfect: a.perfect + g.perfect, good: a.good + g.good, inaccurate: a.inaccurate + g.inaccurate, mistake: a.mistake + g.mistake, blunder: a.blunder + g.blunder }), {
    perfect: 0,
    good: 0,
    inaccurate: 0,
    mistake: 0,
    blunder: 0,
  });

/** Every finished tournament (or the last `count`), oldest first; the demo's numbers move like the real rating (noisy measure, Glicko-like gain). */
export function demoSkill(count?: number): SkillProfile {
  const finished = demoTournaments()
    .rows.filter((t) => t.profit !== null)
    .sort((a, b) => a.started_at.localeCompare(b.started_at));
  const rows = count === undefined ? finished : finished.slice(Math.max(0, finished.length - count));
  let s = 11;
  const rnd = () => (s = (s * 16807) % 2147483647) / 2147483647;
  let rating = 1200;
  let v = 300 ** 2;
  const entries: SkillEntry[] = rows.map((t, i) => {
    v = Math.min(v + 15 ** 2, 300 ** 2);
    const decisions = Math.round(25 + rnd() * 140);
    const real = decisions * (0.25 + rnd() * 0.25);
    const truth = 1230 + i * 6;
    const performance = Math.max(100, Math.min(2400, truth + (rnd() - 0.5) * 2 * (1700 / Math.sqrt(real))));
    const noise = 1700 ** 2 / real;
    const k = v / (v + noise);
    const delta = Math.max(-150, Math.min(150, k * (performance - rating)));
    rating += delta;
    v = (v * noise) / (v + noise);
    const g = grades(decisions, rnd, performance < rating ? 1.6 : 0.7);
    return {
      key: `t:${t.id}`,
      tournament_id: t.id,
      started_at: t.started_at,
      rating,
      sigma: Math.sqrt(v),
      delta,
      performance,
      decisions,
      real,
      burned_pct: real * 0.009 * 100 * (performance < rating ? 1.5 : 0.6),
      grades: g,
    };
  });
  // The last tournament: a costly call off, so "Was hat dich zuletzt bewegt" has both sides.
  const last = entries[entries.length - 1];
  if (last) {
    last.delta = 14;
    last.rating = entries[entries.length - 2] ? entries[entries.length - 2].rating + 14 : last.rating;
  }
  const r = last?.rating ?? null;
  const verdicts: [AreaScore["verdict"], number][] = [
    ["strong", 120],
    ["weak", -140],
    ["even", 20],
    ["few", 0],
    ["even", -30],
    ["weak", -85],
    ["few", 0],
    ["strong", 95],
    ["even", 40],
  ];
  const shares = [9, 24, 14, 2, 11, 27, 3, 7, 3];
  const scale = Math.min(1, entries.length / 20);
  const counts = [48, 31, 286, 15, 104, 341, 4, 233, 51].map((n) => Math.max(1, Math.round(n * scale)));
  const areas: AreaScore[] =
    r === null
      ? []
      : AREAS.map(([area, name, spots], i) => ({
          area,
          name,
          spots,
          decisions: counts[i],
          real: counts[i] * 0.4,
          rating: r + (counts[i] * 0.4 < 4 ? 0 : verdicts[i][1] * scale),
          verdict: counts[i] * 0.4 < 4 ? "few" : verdicts[i][0],
          burn_share_pct: shares[i],
          grades: grades(counts[i], rnd, verdicts[i][1] < 0 ? 1.8 : 0.6),
        }));
  return {
    rating: r,
    sigma: last?.sigma ?? 300,
    provisional: (last?.sigma ?? 300) > 110,
    tier: r === null ? null : tierOf(r),
    tiers: TIERS,
    start: 1200,
    entries,
    grades: sum(entries.map((e) => e.grades)),
    areas,
    moments:
      r === null
        ? []
        : [
            {
              hand_id: 250000000002,
              action_index: 8,
              played_at: last!.started_at,
              hero_cards: "9s9d",
              played: "Call 18 BB",
              best: "Fold",
              rating: "blunder",
              source: "equilibrium",
              area: "vs_all_in",
              points: -11,
              lost_pct: 14.2,
              pot_pct: 125,
              bad_luck: false,
            },
            {
              hand_id: 249999999996,
              action_index: 6,
              played_at: last!.started_at,
              hero_cards: "As5s",
              played: "Fold",
              best: "Raise auf 4,5 BB",
              rating: "mistake",
              source: "model",
              area: "vs_open",
              points: -4,
              lost_pct: 3.1,
              pot_pct: 18,
              bad_luck: false,
            },
            {
              hand_id: 250000000001,
              action_index: 5,
              played_at: last!.started_at,
              hero_cards: "AhKd",
              played: "All-in 22 BB",
              best: "All-in 22 BB",
              rating: "best",
              source: "equilibrium",
              area: "push_fold",
              points: 6,
              lost_pct: 0,
              pot_pct: 100,
              bad_luck: true,
            },
            {
              hand_id: 250000000004,
              action_index: 12,
              played_at: last!.started_at,
              hero_cards: "QhJh",
              played: "Call 6 BB",
              best: "Call 6 BB",
              rating: "best",
              source: "solver",
              area: "river",
              points: 3,
              lost_pct: 0,
              pot_pct: 64,
              bad_luck: false,
            },
          ],
  };
}

export const emptySkill = (): SkillProfile => ({ ...demoSkill(0), rating: null, tier: null, entries: [], areas: [], moments: [] });

/** `?data=few`: three tournaments, still provisional. */
export const fewSkill = (): SkillProfile => demoSkill(3);

export function demoChange(): SkillChange | null {
  const p = demoSkill();
  const last = p.entries[p.entries.length - 1];
  return p.rating === null || !last ? null : { rating: p.rating, sigma: p.sigma, tier: p.tier!, delta: last.delta };
}
