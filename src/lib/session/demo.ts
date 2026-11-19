// Browser demo of the session review: an evening with three tournaments side by side and
// about 150 hands. `?data=empty` has no hands yet, `?data=few` a session too short for
// patterns, `?data=pending` one whose analysis still runs; `&locked=1` is what the backend
// sends while PokerStars runs.
import type { KindLoss, Moment } from "../report/types";
import { demoTournaments } from "../demo";
import type { ClassGap, PreflopReview, SessionPage, SessionReview, SessionSpan, Swing } from "./types";

const preview = typeof window === "undefined" ? null : new URLSearchParams(window.location.search).get("data");
const RANKS = "AKQJT98765432";

/** "AKs" for the class at row/col of the grid, ace first, suited above the diagonal. */
function className(i: number): string {
  const r = Math.floor(i / 13);
  const c = i % 13;
  if (r === c) return RANKS[r] + RANKS[c];
  return r < c ? RANKS[r] + RANKS[c] + "s" : RANKS[c] + RANKS[r] + "o";
}

/** Rough strength, only to make the made-up decisions look like poker. */
function strength(i: number): number {
  const r = Math.floor(i / 13);
  const c = i % 13;
  const hi = 12 - Math.min(r, c);
  const lo = 12 - Math.max(r, c);
  if (r === c) return 0.55 + hi / 28;
  return (hi * 1.6 + lo) / 32 + (r < c ? 0.08 : 0) - (hi - lo) / 60;
}

function preflop(hands: number): PreflopReview {
  let s = 11;
  const rnd = () => (s = (s * 16807) % 2147483647) / 2147483647;
  const by = new Map<number, ClassGap>();
  let compared = 0;
  let played = 0;
  let model = 0;
  for (let h = 0; h < hands; h++) {
    if (rnd() < 0.09) continue; // not compared: no model here, or the analysis is missing
    const i = Math.floor(rnd() * 169);
    const st = strength(i);
    const p = Math.max(0, Math.min(1, (st - 0.62) * 6));
    // Plays a bit tighter than the model, with the odd loose call.
    const you = st > 0.8 || (st > 0.55 && rnd() < 0.08);
    compared++;
    played += you ? 1 : 0;
    model += p;
    const c = by.get(i) ?? { class: i, name: className(i), hands: 0, played: 0, model: 0, loss_bb: 0 };
    c.hands++;
    c.played += you ? 1 : 0;
    c.model += p;
    c.loss_bb += Math.abs((you ? 1 : 0) - p) * (0.4 + rnd());
    by.set(i, c);
  }
  const classes = [...by.values()].sort((a, b) => a.class - b.class);
  const gap = (c: ClassGap) => c.played - c.model;
  const ranked = (wide: boolean) =>
    classes
      .filter((c) => (wide ? gap(c) >= 0.5 : -gap(c) >= 0.5))
      .sort((a, b) => b.loss_bb - a.loss_bb)
      .slice(0, 5)
      .map((c) => c.class);
  const few = compared < 30;
  const diff = compared ? (100 * (played - model)) / compared : 0;
  const direction = diff >= 3 ? "Du spielst etwas zu viele Hände." : diff <= -3 ? "Du spielst etwas zu wenige Hände." : "Wie viele Hände du spielst, passt.";
  return {
    hands,
    compared,
    played,
    model_played: model,
    equilibrium: Math.round(compared * 0.3),
    you_pct: compared ? (100 * played) / compared : null,
    model_pct: compared ? (100 * model) / compared : null,
    classes,
    wide: ranked(true),
    tight: ranked(false),
    patterns: few
      ? []
      : [
          {
            text: "Wenn alle vor dir folden, spielst du am Button zu wenige Hände – dabei ist das die stärkste Position: 4 von 15 Mal, besser etwa 9.",
            wide: false,
            decisions: 15,
            played: 4,
            model: 8.9,
            loss_bb: 3.4,
            history: "unopened",
            position: "BTN",
            vs_position: null,
            practice: "first_in|BTN|2|9",
          },
          {
            text: "Gegen eine Erhöhung aus früher Position gehst du zu oft mit: 9 von 12 Mal, besser etwa 3.",
            wide: true,
            decisions: 12,
            played: 9,
            model: 3.2,
            loss_bb: 5.1,
            history: "vs_open",
            position: "BB",
            vs_position: "UTG",
            practice: "vs_open|BB|2|9",
          },
        ],
    sample: compared === 0 ? null : few ? "few" : compared < 300 ? "rough" : "solid",
    decisions: compared + 31,
    good: Math.round((compared + 31) * 0.83),
    loss_bb: 14.2,
    verdict: few
      ? null
      : { level: 2, title: "Ausbaufähig", line: `8 von 10 Entscheidungen vor dem Flop passen. ${direction}` },
  };
}

const WORST: Moment[] = [
  {
    hand_id: 250000000002,
    action_index: 6,
    n: 58,
    played_at: "2026/11/18 20:31:10",
    hero_cards: "9s 9d",
    position: "SB",
    street: "preflop",
    spot: "vs_limp",
    situation: "Wenn einer nur mitgeht (Limp)",
    stack_bb: 42,
    played: "Call 0,5 BB",
    best: "Raise auf 4,5 BB",
    why: "Als SB bei 42 BB: Das Modell spielt mit 51 % der Hände weiter, die hier ankommen. 99 gehört zu den besten 3 % davon. Raise auf 4,5 BB bringt 1,5 BB mehr als Call.",
    rating: "blunder",
    source: "model",
    loss_bb: 1.52,
    margin_bb: null,
    alternative: null,
  },
  {
    hand_id: 250000000001,
    action_index: 11,
    n: 104,
    played_at: "2026/11/18 21:44:10",
    hero_cards: "Ah 7c",
    position: "BTN",
    street: "preflop",
    spot: "shove",
    situation: "All-in oder Fold als Erster",
    stack_bb: 10,
    played: "Fold",
    best: "All-in",
    why: "Als BTN bei 10 BB: Das Gleichgewicht geht mit 40 % aller Hände all-in. A7o gehört zu den besten 16 %. All-in bringt 0,99 BB mehr als Fold.",
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
    played_at: "2026/11/18 22:15:10",
    hero_cards: "Ah Qd",
    position: "BTN",
    street: "turn",
    spot: "turn",
    situation: "Turn",
    stack_bb: 34,
    played: "Check",
    best: "Bet 6,9 BB",
    why: "Deine Hand gehört zum stärksten Drittel deiner Range. Auf Bet 6,9 BB gibt der Gegner in 44 % der Fälle auf. Bet 6,9 BB bringt 0,93 BB mehr als Check.",
    rating: "mistake",
    source: "solver",
    loss_bb: 0.926,
    margin_bb: null,
    alternative: null,
  },
];

function swing(s: Partial<Swing> & Pick<Swing, "hand_id" | "hero_cards" | "played" | "net_bb">): Swing {
  return {
    action_index: 8,
    tournament_id: null,
    position: "BTN",
    street: "preflop",
    best: null,
    rating: "best",
    source: "equilibrium",
    why: "",
    ev_bb: null,
    luck_bb: null,
    loss_bb: 0,
    margin_bb: null,
    alternative: null,
    equity_pct: null,
    opponents: [],
    board: [],
    ...s,
  };
}

const BAD_LUCK: Swing[] = [
  swing({
    hand_id: 250000000010,
    hero_cards: "Ah Qh",
    position: "CO",
    played: "All-in 14,5 BB",
    net_bb: -14.5,
    ev_bb: 6.1,
    luck_bb: -20.6,
    margin_bb: 2.3,
    alternative: "Fold",
    equity_pct: 70.4,
    opponents: [["Kc", "Jd"]],
    board: [],
    why: "Als CO bei 14 BB: Das Gleichgewicht geht mit 31 % aller Hände all-in. AQs gehört zu den besten 3 %.",
  }),
  swing({
    hand_id: 250000000011,
    hero_cards: "7s 7d",
    position: "BB",
    played: "Call 11 BB",
    net_bb: -11.5,
    ev_bb: 0.4,
    luck_bb: -11.9,
    margin_bb: 0.8,
    alternative: "Fold",
    equity_pct: 31.2,
    opponents: [["Ac", "Ad"]],
    why: "Gegen einen Shove vom Button callt das Gleichgewicht mit 77 und besser – gegen seine Range liegst du vorn.",
  }),
  swing({
    hand_id: 250000000012,
    hero_cards: "Jh Th",
    position: "BTN",
    street: "river",
    source: "solver",
    rating: "good",
    played: "Call 6 BB",
    net_bb: -9.2,
    margin_bb: 0.6,
    alternative: "Fold",
  }),
];

const GOOD_LUCK: Swing[] = [
  swing({
    hand_id: 250000000020,
    hero_cards: "Kd 9c",
    position: "BB",
    rating: "mistake",
    played: "Call 18 BB",
    best: "Fold",
    net_bb: 19.5,
    ev_bb: -4.3,
    luck_bb: 23.8,
    loss_bb: 2.6,
    equity_pct: 38.5,
    opponents: [["As", "Jh"]],
    board: [],
    why: "Gegen einen Shove aus früher Position callt das Gleichgewicht erst ab KJ.",
  }),
  swing({
    hand_id: 250000000021,
    hero_cards: "Ac 4d",
    position: "UTG",
    rating: "inaccuracy",
    source: "model",
    played: "Raise 2,2 BB",
    best: "Fold",
    net_bb: 3.5,
    loss_bb: 0.4,
  }),
];

const START = "2026/11/18 20:04:00";

/** Loss per kind of spot of the demo session, costliest first: the hands behind „Als Nächstes üben“. */
export const SESSION_KINDS: KindLoss[] = [
  { key: "vs_open|BB|2|9", title: "Gegen Open im Big Blind", detail: "20–40 BB · 7–10 Spieler", lage: "vs_open", decisions: 14, flawed: 5, loss_bb: 6.1 },
  { key: "river_vs_bet|-|3|6", title: "River: auf einen Einsatz reagieren", detail: "ab 40 BB · 6 Spieler", lage: "river_vs_bet", decisions: 4, flawed: 2, loss_bb: 3.0 },
];

function spans(): SessionSpan[] {
  // One currency for the evening, like most players.
  const ids = demoTournaments().rows.filter((t) => t.currency === "USD").map((t) => t.id);
  const mk = (start: string, end: string, hands: number, t: number[], cash = 0): SessionSpan => ({ start, end, hands, tournaments: t, cash_hands: cash });
  return [
    mk(START, "2026/11/18 22:41:30", preview === "few" ? 22 : 152, ids.slice(0, 3)),
    mk("2026/11/17 19:12:00", "2026/11/17 21:58:00", 211, ids.slice(3, 7)),
    mk("2026/11/15 20:30:00", "2026/11/15 21:05:00", 48, [], 48),
    mk("2026/11/14 18:40:00", "2026/11/14 23:10:00", 380, ids.slice(7, 12)),
    mk("2026/11/12 21:00:00", "2026/11/12 22:20:00", 96, ids.slice(12, 14)),
  ];
}

function review(span: SessionSpan, at: number): SessionReview {
  const pending = preview === "pending";
  const hands = span.hands;
  const p = preflop(hands);
  const worst = at === 0 ? WORST : WORST.slice(1).map((m) => ({ ...m, played_at: span.start }));
  return {
    span,
    report: {
      hands,
      unanalyzed: pending ? Math.round(hands * 0.4) : 0,
      pending: pending ? 14 : at === 0 ? 6 : 0,
      unrated: 2,
      luck: { net: -184000, ev: 62000, luck: -246000, net_bb: -18.4, ev_bb: 6.2, luck_bb: -24.6, all_ins: 7 },
      score: { decisions: 241, good: 199, inaccuracies: 19, mistakes: 23, ratings: [150, 20, 29, 19, 15, 8], loss_bb: 21.7, per100: 9, good_pct: 82.6, estimate_loss_pct: 12 },
      breakdowns: [],
      worst,
      best: [],
      starting: [],
      timeline: [],
      kinds: SESSION_KINDS,
      next: [],
      history: [],
      average: null,
    },
    preflop: p,
    bad_luck: preview === "few" ? BAD_LUCK.slice(0, 1) : BAD_LUCK,
    good_luck: GOOD_LUCK,
    locked: false,
  };
}

/** What the backend leaves of a review while PokerStars runs (`SessionReview::locked`). */
function locked(r: SessionReview): SessionReview {
  const p = r.preflop;
  return {
    span: r.span,
    report: {
      ...r.report,
      score: { decisions: 0, good: 0, inaccuracies: 0, mistakes: 0, ratings: [], loss_bb: 0, per100: null, good_pct: null, estimate_loss_pct: 0 },
      breakdowns: [],
      worst: [],
      best: [],
      starting: [],
      timeline: [],
      kinds: [],
      next: [],
    },
    preflop: {
      hands: p.hands,
      compared: p.compared,
      played: p.played,
      you_pct: p.you_pct,
      sample: p.sample,
      model_played: 0,
      equilibrium: 0,
      model_pct: null,
      classes: [],
      wide: [],
      tight: [],
      patterns: [],
      decisions: 0,
      good: 0,
      loss_bb: 0,
      verdict: null,
    },
    bad_luck: [],
    good_luck: [],
    locked: true,
  };
}

export function demoSession(start: string | null): SessionPage {
  if (preview === "empty") return { sessions: [], review: null, tournaments: [] };
  const sessions = spans();
  const at = Math.max(0, sessions.findIndex((s) => s.start === start));
  const span = sessions[at];
  const rows = demoTournaments().rows;
  const tournaments = span.tournaments.map((id, i) => {
    const t = rows.find((r) => r.id === id)!;
    // The evening's three: one still running, one deep run, one early exit.
    if (at !== 0) return t;
    const place = [null, 14, 388][i];
    const prize = [null, 1240, 0][i];
    const profit = prize === null ? null : prize + t.bounties - (t.cost ?? 0);
    return { ...t, place, prize, profit, entrants: [null, 412, 690][i], hands: [61, 58, 33][i], started_at: ["2026/11/18 20:04:00", "2026/11/18 20:09:00", "2026/11/18 21:20:00"][i] };
  });
  const r = review(span, at);
  const isLocked = typeof window !== "undefined" && new URLSearchParams(window.location.search).get("locked") === "1";
  return { sessions, review: isLocked ? locked(r) : r, tournaments };
}
