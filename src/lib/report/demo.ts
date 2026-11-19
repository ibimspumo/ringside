// Browser demo of the tournament report: a made-up tournament around the demo hands, so
// every click (hand, practice, coach) leads somewhere. `?report=<id>&data=pending` shows a
// preliminary report, `?data=empty` one without rated decisions.
import { demoTournaments } from "../demo";
import { demoWithin } from "../next/demo";
import type { AskAbout, AskAnswer, AskEstimate, KindLoss, Moment, StackPoint, TournamentReport } from "./types";

/** The demo tournament with a stored coach review. */
export const DEMO_REVIEWED = 4046787838;

const preview = typeof window === "undefined" ? null : new URLSearchParams(window.location.search).get("data");

const WORST: Moment[] = [
  {
    hand_id: 250000000002,
    action_index: 6,
    n: 58,
    played_at: "2026/11/16 20:31:10",
    hero_cards: "9s 9d",
    position: "SB",
    street: "preflop",
    spot: "vs_limp",
    situation: "Wenn einer nur mitgeht (Limp)",
    stack_bb: 42,
    played: "Call 0,5 BB",
    best: "Raise auf 4,5 BB",
    why: "Als SB bei 100 BB Tiefe: Das Modell spielt mit 51 % der Hände weiter, die hier ankommen. 99 gehört zu den besten 3 % davon, also klar dazu. Raise auf 4,5 BB bringt 1,5 BB mehr als Call.",
    rating: "blunder",
    source: "model",
    loss_bb: 1.52,
    margin_bb: null,
    alternative: null,
  },
  {
    hand_id: 250000000001,
    action_index: 11,
    n: 164,
    played_at: "2026/11/16 21:44:10",
    hero_cards: "Ah 7c",
    position: "BTN",
    street: "preflop",
    spot: "shove",
    situation: "All-in oder Fold als Erster",
    stack_bb: 10,
    played: "Fold",
    best: "All-in",
    why: "Als BTN bei 10 BB effektiv: Das Gleichgewicht geht mit 40 % aller Hände all-in. A7o gehört zu den besten 16 % aller Hände, also klar dazu. All-in bringt 0,99 BB mehr als Fold.",
    rating: "blunder",
    source: "equilibrium",
    loss_bb: 0.985,
    margin_bb: null,
    alternative: null,
  },
  {
    hand_id: 250000000000,
    action_index: 12,
    n: 131,
    played_at: "2026/11/16 21:15:10",
    hero_cards: "Ah Qd",
    position: "BTN",
    street: "turn",
    spot: "turn",
    situation: "Turn",
    stack_bb: 34,
    played: "Check",
    best: "Bet 6,9 BB",
    why: "Deine Hand gehört zum stärksten Drittel deiner Range und hat gegen die Range des Gegners 93 % Equity. Damit ist der Einsatz Value: Er soll von schwächeren Händen bezahlt werden. Auf Bet 6,9 BB gibt der Gegner in 44 % der Fälle auf. Bet 6,9 BB bringt 0,93 BB mehr als Check.",
    rating: "mistake",
    source: "solver",
    loss_bb: 0.926,
    margin_bb: null,
    alternative: null,
  },
];

const BEST: Moment[] = [
  {
    hand_id: 250000000002,
    action_index: 11,
    n: 58,
    played_at: "2026/11/16 20:31:10",
    hero_cards: "9s 9d",
    position: "SB",
    street: "flop",
    spot: "flop",
    situation: "Flop: auf einen Einsatz reagieren",
    stack_bb: 42,
    played: "Raise auf 8 BB",
    best: "Raise auf 8 BB",
    why: "Gegen die wahrscheinlichen Ranges von 2 Gegnern hat deine Hand geschätzt 94 % Equity. Ein Call braucht nur 22 %, das reicht klar. Erhöhen bringt geschätzt 0,11 BB mehr als Callen.",
    rating: "best",
    source: "estimate",
    loss_bb: 0,
    margin_bb: 1.1,
    alternative: "Call",
  },
  {
    hand_id: 250000000000,
    action_index: 9,
    n: 131,
    played_at: "2026/11/16 21:15:10",
    hero_cards: "Ah Qd",
    position: "BTN",
    street: "flop",
    spot: "c_bet",
    situation: "C-Bet am Flop",
    stack_bb: 34,
    played: "Bet 1,8 BB",
    best: "Bet 1,8 BB",
    why: "Deine Hand gehört zum stärksten Drittel deiner Range und hat gegen die Range des Gegners 89 % Equity. Auf Bet 1,8 BB gibt der Gegner in 42 % der Fälle auf.",
    rating: "best",
    source: "solver",
    loss_bb: 0,
    margin_bb: 0.62,
    alternative: "Check",
  },
  {
    hand_id: 250000000000,
    action_index: 5,
    n: 131,
    played_at: "2026/11/16 21:15:10",
    hero_cards: "Ah Qd",
    position: "BTN",
    street: "preflop",
    spot: "steal",
    situation: "Eröffnen",
    stack_bb: 34,
    played: "Raise auf 2,5 BB",
    best: "Raise auf 2,5 BB",
    why: "Als BTN bei 100 BB Tiefe: Das Modell spielt mit 38 % der Hände weiter, die hier ankommen. AQo gehört zu den besten 6 % davon, also klar dazu.",
    rating: "best",
    source: "model",
    loss_bb: 0,
    margin_bb: 0.4,
    alternative: "Fold",
  },
];

/** A stack walk over the hands with the costliest mistakes on their hands. */
function timeline(hands: number, busted: boolean): StackPoint[] {
  let s = 11;
  const rnd = () => (s = (s * 16807) % 2147483647) / 2147483647;
  let stack = 1_500_00;
  const marks = new Map(WORST.map((m, i) => [m.n, i + 1]));
  return Array.from({ length: hands }, (_, i) => {
    const n = i + 1;
    const bb = n < 60 ? 2000 : n < 120 ? 6000 : 12000;
    const swing = rnd() < 0.08 ? (rnd() - 0.42) * stack * 0.9 : (rnd() - 0.55) * bb * 3;
    const last = n === hands && busted;
    const net = last ? -stack : Math.round(Math.max(-stack + bb, swing) / 100) * 100;
    const point: StackPoint = { n, hand_id: 250000000000 + n, stack, big_blind: bb, net, loss_bb: marks.has(n) ? WORST[marks.get(n)! - 1].loss_bb : 0, mistake: marks.get(n) ?? null };
    stack += net;
    return point;
  });
}

export function demoReport(id: number): TournamentReport {
  const all = demoTournaments().rows;
  const tournament = all.find((t) => t.id === id) ?? all[2];
  const hands = Math.max(tournament.hands, 170);
  const empty = preview === "empty";
  const pending = preview === "pending";
  const ratings = empty ? [0, 0, 0, 0, 0, 0] : [148, 21, 34, 14, 9, 4];
  const decisions = ratings.reduce((a, b) => a + b, 0);
  const loss = empty ? 0 : 18.6;
  const group = (key: string, title: string, d: number, flawed: number, l: number, unsure = false, spots: string[] = []) => ({
    key,
    title,
    decisions: d,
    good: d - flawed,
    flawed,
    loss_bb: l,
    estimate_loss_pct: unsure ? 72 : 18,
    unsure,
    spots: spots as never[],
  });
  return {
    tournament,
    hands,
    unanalyzed: pending ? 12 : 0,
    pending: pending ? 23 : 0,
    unrated: 0,
    luck: { net: -1_500_00, ev: 640_00, luck: -2_140_00, net_bb: -38.2, ev_bb: 9.6, luck_bb: -47.8, all_ins: 6 },
    score: {
      decisions,
      good: empty ? 0 : 203,
      inaccuracies: empty ? 0 : 14,
      mistakes: empty ? 0 : 13,
      ratings,
      loss_bb: loss,
      per100: empty ? null : (loss / decisions) * 100,
      good_pct: empty ? null : (203 / decisions) * 100,
      estimate_loss_pct: 31,
    },
    breakdowns: empty
      ? []
      : [
          {
            key: "situation",
            title: "Spot-Art",
            groups: [
              group("river_vs_bet", "River: auf einen Einsatz reagieren", 9, 3, 5.4, true, ["river"]),
              group("vs_limp", "Wenn einer nur mitgeht (Limp)", 14, 4, 4.1, false, ["vs_limp"]),
              group("push_fold", "All-in oder Fold als Erster", 31, 3, 3.2, false, ["open", "steal", "shove"]),
              group("vs_open", "Gegen eine Erhöhung", 48, 4, 2.6, false, ["vs_open", "three_bet"]),
              group("c_bet", "C-Bet am Flop", 12, 2, 1.9, false, ["c_bet"]),
              group("first_in", "Eröffnen", 96, 3, 1.4, false, ["open", "steal"]),
            ],
          },
          {
            key: "seat",
            title: "Position",
            groups: [
              group("BTN", "Button", 41, 7, 7.9),
              group("SB", "Small Blind", 38, 6, 4.8),
              group("BB", "Big Blind", 52, 6, 3.1),
              group("CO", "Cutoff", 33, 3, 1.5),
              group("MP", "Mittlere Position", 34, 3, 0.9),
              group("EP", "Frühe Position", 32, 2, 0.4),
            ],
          },
          {
            key: "street",
            title: "Straße",
            groups: [
              group("0", "Vor dem Flop", 189, 12, 9.1),
              group("3", "River", 14, 5, 6.2, true),
              group("1", "Flop", 25, 6, 2.4),
              group("2", "Turn", 2, 4, 0.9),
            ],
          },
          {
            key: "stack",
            title: "Stacktiefe",
            groups: [
              group("2", "20–40 BB", 82, 11, 9.8),
              group("1", "10–20 BB", 64, 9, 5.6),
              group("0", "unter 10 BB", 41, 4, 2.1),
              group("3", "ab 40 BB", 43, 3, 1.1),
            ],
          },
        ],
    worst: empty ? [] : WORST,
    best: empty ? [] : BEST,
    starting: empty
      ? []
      : [
          { class: "AQo", hands: 3, net_bb: -21.5, loss_bb: 3.0, flawed: 1, hand_ids: [250000000000] },
          { class: "99", hands: 2, net_bb: 38.4, loss_bb: 1.5, flawed: 1, hand_ids: [250000000002] },
          { class: "A7o", hands: 4, net_bb: -1.5, loss_bb: 1.0, flawed: 1, hand_ids: [250000000001] },
          { class: "AA", hands: 1, net_bb: 24.0, loss_bb: 0, flawed: 0, hand_ids: [250000000002] },
          { class: "KK", hands: 1, net_bb: -42.0, loss_bb: 0, flawed: 0, hand_ids: [250000000002] },
          { class: "AKs", hands: 2, net_bb: 11.2, loss_bb: 0.3, flawed: 0, hand_ids: [250000000000] },
          { class: "JTs", hands: 3, net_bb: 6.1, loss_bb: 0.5, flawed: 1, hand_ids: [250000000002] },
          { class: "T9s", hands: 2, net_bb: -6.4, loss_bb: 0.8, flawed: 1, hand_ids: [250000000002] },
          { class: "K9o", hands: 5, net_bb: -3.0, loss_bb: 1.2, flawed: 2, hand_ids: [250000000001] },
          { class: "72o", hands: 6, net_bb: -1.0, loss_bb: 0, flawed: 0, hand_ids: [250000000001] },
          { class: "55", hands: 3, net_bb: 9.0, loss_bb: 0.4, flawed: 1, hand_ids: [250000000002] },
          { class: "QJo", hands: 4, net_bb: -4.5, loss_bb: 0.9, flawed: 1, hand_ids: [250000000000] },
        ],
    timeline: timeline(hands, tournament.prize === 0),
    kinds: empty ? [] : KINDS,
    next: empty ? [] : demoWithin(KINDS, "in diesem Turnier"),
    history: empty
      ? []
      : all
          .filter((t) => t.started_at < tournament.started_at)
          .slice(0, 5)
          .map((t, i) => ({
            tournament_id: t.id,
            started_at: t.started_at,
            name: t.name,
            decisions: 160 + i * 23,
            per100: [6.1, 9.4, 7.2, 11.8, 5.6][i],
            good_pct: [91.5, 87.0, 90.2, 84.1, 92.3][i],
          })),
    average: empty ? null : { tournaments: 27, decisions: 5480, per100: 7.4, good_pct: 89.6 },
    coach_review: id === DEMO_REVIEWED,
  };
}

export async function demoAskPrepare(about: AskAbout, question: string): Promise<AskEstimate> {
  await new Promise((r) => setTimeout(r, 300));
  const sent =
    about.kind === "tournament"
      ? "Ergebnis: Platz 212 von 640, ohne Preisgeld.\nBewertete Entscheidungen: 230, davon gut 203 (88,3 %) …"
      : "Hand · Blinds 100/200, Ante 25 …\nBewertung von Ringside:\n- vor dem Flop: gespielt Call 0,5 BB; Empfehlung Raise auf 4,5 BB …";
  return {
    token: "demo",
    model: "openai/gpt-6.1-sol",
    question,
    sent: `Fakten von Ringside:\n${sent}\n\nFrage: ${question}`,
    prompt_tokens: 1450,
    completion_tokens: 900,
    max_completion_tokens: 3000,
    cost_usd: 0.0039,
    max_cost_usd: 0.0112,
  };
}

export async function demoAsk(about: AskAbout): Promise<AskAnswer> {
  await new Promise((r) => setTimeout(r, 900));
  const text =
    about.kind === "tournament"
      ? "Dein Score ist ordentlich: 88,3 % deiner Entscheidungen waren gut. Teuer wurde es vor allem am River gegen eine Bet.\n\nDort hast du mit starken Händen nur gecallt. Ein Raise holt mehr Chips aus Gegnern, die schwächere Hände bezahlen. Das nennt man Value: Du setzt, weil schlechtere Hände mitgehen.\n\nÜbe als Nächstes „River: Gegner setzt“ – dort hast du 5,4 BB verloren."
      : "Ein Raise ist hier besser, weil du mit 99 eine starke Hand hast und der Limper meist schwächer ist.\n\nMit dem Call lässt du ihn billig mitspielen und spielst später ohne Position. Ringside rechnet den Unterschied mit 1,52 BB.";
  return { text, unchecked: [], cost_usd: 0.0041, model: "openai/gpt-6.1-sol" };
}

/** Loss per kind of spot of the demo tournament, costliest first. */
const KINDS: KindLoss[] = [
  { key: "river_vs_bet|-|3|6", title: "River: auf einen Einsatz reagieren", detail: "ab 40 BB · 6 Spieler", lage: "river_vs_bet", decisions: 9, flawed: 3, loss_bb: 5.4 },
  { key: "vs_limp|SB|3|6", title: "Wenn einer nur mitgeht (Limp) im Small Blind", detail: "ab 40 BB · 6 Spieler", lage: "vs_limp", decisions: 6, flawed: 3, loss_bb: 4.1 },
  { key: "push_fold|BTN|1|6", title: "All-in oder Fold als Erster am Button", detail: "10–20 BB · 6 Spieler", lage: "push_fold", decisions: 31, flawed: 3, loss_bb: 3.2 },
  { key: "vs_open|BB|2|6", title: "Gegen eine Erhöhung im Big Blind", detail: "20–40 BB · 6 Spieler", lage: "vs_open", decisions: 7, flawed: 2, loss_bb: 2.6 },
];
