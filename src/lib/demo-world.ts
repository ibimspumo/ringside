// The browser demo's one calendar: every session with its tournaments and cash hands, and the
// hero's result in each. Tournament list, sessions, totals, hand counts, bounties and dates on the
// demo pages are derived from here, so the pages agree (scripts/demo-consistency.test.mjs).
import type { Site, TournamentRow } from "./types";

/** "Now" in the demo: late in the evening of the last session, one tournament still running. */
export const NOW = "2026/11/18 22:45:00";
/** One hand at one table; tables run side by side. */
export const HAND_SECONDS = 60;

export const GG_TOURNAMENT = 2 ** 52 + 241300777;
/** The running tournament, also the demo's live table. */
export const RUNNING_TOURNAMENT = 4046789300;
const FIRST_PS_ID = RUNNING_TOURNAMENT;

type KindId = "mini" | "builder" | "hot" | "plo" | "sunday" | "sng";
const KINDS: Record<KindId, { game: string; buy: [number, number, number]; cur: string; name: string; field?: number }> = {
  mini: { game: "NL Hold'em", buy: [500, 0, 50], cur: "USD", name: "$5.50 Mini Early Battle [Progressive KO], $1.5K Gtd" },
  builder: { game: "NL Hold'em", buy: [1000, 1000, 200], cur: "USD", name: "Bounty Builder $22, $10K Gtd" },
  hot: { game: "NL Hold'em", buy: [300, 0, 30], cur: "USD", name: "Hot $3.30, $1.5K Gtd" },
  plo: { game: "PL Omaha", buy: [460, 0, 40], cur: "EUR", name: "PLO Turbo €5" },
  sunday: { game: "NL Hold'em", buy: [200, 200, 40], cur: "USD", name: "Sunday Million - Day 1" },
  sng: { game: "NL Hold'em", buy: [180, 0, 20], cur: "EUR", name: "Sit & Go 9-Max Hyper", field: 9 },
};

/** What a plan fixes about one tournament; the rest is drawn from a seeded generator. */
interface PlanT {
  kind: KindId;
  /** Minutes after the session start. */
  after: number;
  hands?: number;
  entrants?: number;
  place?: number | null;
  prize?: number;
  bounties?: number;
  net_bb?: number;
  freeroll?: boolean;
  running?: boolean;
}

interface PlanSession {
  day: number;
  at: string;
  t: PlanT[];
  cash?: { hands: number; net_bb: number };
}

/** Newest first. The last evening: one tournament running, one deep run on GGPoker, one early exit. */
const PLAN: PlanSession[] = [
  {
    day: 18,
    at: "20:04",
    t: [
      { kind: "mini", after: 0, running: true, net_bb: 19.1 },
      { kind: "builder", after: 45, hands: 52, entrants: 412, place: 14, prize: 1240, bounties: 0, net_bb: -12.3 },
      { kind: "sunday", after: 76, hands: 30, entrants: 690, place: 388, prize: 0, bounties: 100, net_bb: -27.6 },
    ],
    cash: { hands: 15, net_bb: 2.4 },
  },
  { day: 17, at: "19:12", t: [{ kind: "sunday", after: 0 }, { kind: "mini", after: 22 }, { kind: "hot", after: 48 }, { kind: "plo", after: 95 }] },
  { day: 16, at: "18:00", t: [{ kind: "plo", after: 0 }, { kind: "hot", after: 31 }, { kind: "builder", after: 70 }] },
  { day: 15, at: "20:30", t: [], cash: { hands: 48, net_bb: 6.5 } },
  { day: 14, at: "18:05", t: [{ kind: "sng", after: 0 }, { kind: "mini", after: 73 }, { kind: "hot", after: 146 }] },
  { day: 13, at: "21:44", t: [{ kind: "builder", after: 0 }, { kind: "plo", after: 73, freeroll: true }] },
  { day: 12, at: "18:10", t: [{ kind: "hot", after: 0 }, { kind: "sunday", after: 73 }] },
  { day: 11, at: "20:36", t: [{ kind: "sunday", after: 0, place: 3, entrants: 676 }, { kind: "hot", after: 73 }, { kind: "sunday", after: 86 }] },
  { day: 10, at: "18:15", t: [{ kind: "plo", after: 0 }, { kind: "sng", after: 73 }] },
  { day: 9, at: "20:41", t: [{ kind: "mini", after: 0 }, { kind: "sunday", after: 73 }, { kind: "hot", after: 86 }] },
  { day: 8, at: "18:20", t: [{ kind: "sng", after: 0 }, { kind: "sunday", after: 73 }] },
  { day: 7, at: "20:46", t: [{ kind: "sunday", after: 0 }, { kind: "hot", after: 73 }] },
  { day: 6, at: "18:25", t: [{ kind: "hot", after: 0 }, { kind: "hot", after: 73 }, { kind: "sunday", after: 227 }] },
  { day: 5, at: "20:51", t: [{ kind: "hot", after: 0 }, { kind: "mini", after: 13 }] },
  { day: 4, at: "18:30", t: [{ kind: "sunday", after: 0 }, { kind: "hot", after: 73 }, { kind: "builder", after: 227 }] },
  { day: 3, at: "20:56", t: [{ kind: "sunday", after: 0 }, { kind: "plo", after: 13 }] },
];

export interface WorldTournament {
  row: TournamentRow;
  /** Hero's result over the tournament's hands, in big blinds of each hand. */
  net_bb: number;
  /** When the hero's last hand there was played. */
  ended_at: string;
}

export interface WorldSession {
  start: string;
  end: string;
  hands: number;
  tournaments: number[];
  cash_hands: number;
  sites: Site[];
  net_bb: number;
  /** Bounties won, by currency, in cents. */
  bounties: Record<string, number>;
  /** Most tables open at once. */
  tables: number;
}

/** "2026/11/18 20:04:00" plus minutes, in the same form. */
export function addMinutes(at: string, minutes: number): string {
  const [d, t] = at.split(" ");
  const [y, mo, da] = d.split("/").map(Number);
  const [h, mi, s] = t.split(":").map(Number);
  const x = new Date(Date.UTC(y, mo - 1, da, h, mi, s) + Math.round(minutes * 60_000));
  const p = (n: number) => String(n).padStart(2, "0");
  return `${x.getUTCFullYear()}/${p(x.getUTCMonth() + 1)}/${p(x.getUTCDate())} ${p(x.getUTCHours())}:${p(x.getUTCMinutes())}:${p(x.getUTCSeconds())}`;
}

/** Minutes from `a` to `b`. */
export function minutesBetween(a: string, b: string): number {
  const ms = (x: string) => Date.parse(x.replace(/\//g, "-").replace(" ", "T") + "Z");
  return (ms(b) - ms(a)) / 60_000;
}

/** Seconds since the epoch as the app reads a stored time (local time of the demo calendar). */
export function epochOf(at: string): number {
  return Math.floor(new Date(at.replace(/\//g, "-").replace(" ", "T")).getTime() / 1000);
}

function build(): { tournaments: WorldTournament[]; sessions: WorldSession[] } {
  let s = 7;
  const rnd = () => (s = (s * 16807) % 2147483647) / 2147483647;
  const tournaments: WorldTournament[] = [];
  const sessions: WorldSession[] = [];
  let psIndex = 0;
  let unknown = 0;
  for (const plan of PLAN) {
    const start = `2026/11/${String(plan.day).padStart(2, "0")} ${plan.at}:00`;
    const ids: number[] = [];
    let end = plan.cash ? addMinutes(start, (plan.cash.hands * HAND_SECONDS) / 60) : start;
    let net = plan.cash?.net_bb ?? 0;
    let hands = plan.cash?.hands ?? 0;
    const bounties: Record<string, number> = {};
    const windows: [string, string][] = [];
    for (const p of plan.t) {
      const k = KINDS[p.kind];
      const gg = plan.day === 18 && p.kind === "builder";
      const id = gg ? GG_TOURNAMENT : FIRST_PS_ID - 731 * psIndex++;
      const [prizePart, bountyPart, fee] = p.freeroll ? [0, 0, 0] : k.buy;
      const entrants = p.entrants ?? (k.field ?? Math.round(18 + rnd() * 700));
      // A few early ones were played before Ringside read summaries: place unknown, no prize.
      const known = p.running || p.place !== undefined || !(rnd() < 0.08 && unknown++ < 3);
      const place = p.running ? null : p.place !== undefined ? p.place : known ? Math.max(1, Math.round(rnd() ** 1.4 * entrants)) : null;
      const paid = k.field ? 2 : Math.ceil(entrants * 0.15);
      const pool = (prizePart || 20) * entrants;
      const share = (n: number) => (k.field ? [0.65, 0.35][n - 1] : Math.max(0.22 * n ** -1.15, (1.6 * (prizePart || 20)) / pool));
      const prize = p.running ? null : (p.prize ?? (place !== null && place <= paid ? Math.round(pool * share(place)) : 0));
      // Re-entries only where the first bullet went out without a prize, so a cash stays a profit.
      const reentry = !p.freeroll && !p.running && p.place === undefined && !prize && rnd() > 0.85 ? prizePart + bountyPart + fee : 0;
      const won = p.bounties ?? (bountyPart ? Math.round(rnd() * 3) * Math.round(bountyPart / 2) : 0);
      const depth = place === null ? (p.running ? 0.6 : 0.3) : 1 - place / entrants;
      const played = p.hands ?? (p.running ? Math.round(minutesBetween(addMinutes(start, p.after), NOW) - 4) : Math.round(35 + depth * 170 + rnd() * 30));
      const cashed = (prize ?? 0) > 0;
      const netBb = p.net_bb ?? Math.round((cashed ? 40 + depth * 120 + rnd() * 30 : -(15 + rnd() * 40)) * 10) / 10;
      const cost = prizePart + bountyPart + fee + reentry;
      // A session ends after 30 minutes without a hand: a later tournament starts before that.
      const planned = addMinutes(start, p.after);
      const latest = addMinutes(end, 20);
      const startedAt = windows.length && planned > latest ? latest : planned;
      const endedAt = addMinutes(startedAt, (played * HAND_SECONDS) / 60);
      tournaments.push({
        net_bb: netBb,
        ended_at: endedAt,
        row: {
          id,
          site: gg ? "ggpoker" : "pokerstars",
          game: k.game,
          currency: k.cur,
          buy_in: gg
            ? { prize: 500, bounty: 500, fee: 80, currency: "USD" }
            : { prize: prizePart, bounty: bountyPart, fee, currency: p.freeroll ? null : k.cur },
          cost: gg ? 1080 : cost,
          entrants: known && !p.running ? entrants : null,
          place,
          prize,
          bounties: won,
          profit: prize === null ? null : prize + won - (gg ? 1080 : cost),
          started_at: startedAt,
          hands: played,
          chips: Math.round(netBb * 400) * 100,
          has_summary: known && !p.running && psIndex % 7 !== 4,
          // Names come from table windows; tournaments played before Ringside ran have none.
          name: gg ? "Bounty Hunters $10.80, $5K GTD" : psIndex % 4 === 0 && plan.day < 16 ? null : k.name,
        },
      });
      ids.push(id);
      hands += played;
      net += netBb;
      if (won) bounties[k.cur] = (bounties[k.cur] ?? 0) + won;
      windows.push([startedAt, endedAt]);
      if (endedAt > end) end = endedAt;
    }
    const at = (x: string) => windows.filter(([a, b]) => a <= x && x < b).length;
    const tables = Math.max(1, ...windows.map(([a]) => at(a))) + (plan.cash ? 1 : 0);
    sessions.push({
      start,
      end,
      hands,
      tournaments: ids,
      cash_hands: plan.cash?.hands ?? 0,
      sites: ids.includes(GG_TOURNAMENT) ? ["pokerstars", "ggpoker"] : ["pokerstars"],
      net_bb: Math.round(net * 10) / 10,
      bounties,
      tables,
    });
  }
  tournaments.sort((a, b) => b.row.started_at.localeCompare(a.row.started_at));
  return { tournaments, sessions };
}

const WORLD = build();

/** Every tournament, newest first. */
export const WORLD_TOURNAMENTS: WorldTournament[] = WORLD.tournaments;
/** Every session, newest first. */
export const WORLD_SESSIONS: WorldSession[] = WORLD.sessions;

/** All the hero's hands in the demo. */
export const TOTAL_HANDS = WORLD_SESSIONS.reduce((n, x) => n + x.hands, 0);
/** The hero's result over all hands, in big blinds. */
export const TOTAL_NET_BB = Math.round(WORLD_SESSIONS.reduce((n, x) => n + x.net_bb, 0) * 10) / 10;
export const GG_HANDS = WORLD_TOURNAMENTS.find((t) => t.row.id === GG_TOURNAMENT)!.row.hands;

/** Bounties won over all sessions, by currency, in cents. */
export function totalBounties(): Record<string, number> {
  const out: Record<string, number> = {};
  for (const x of WORLD_SESSIONS) for (const [c, a] of Object.entries(x.bounties)) out[c] = (out[c] ?? 0) + a;
  return out;
}

export const worldTournament = (id: number) => WORLD_TOURNAMENTS.find((t) => t.row.id === id) ?? null;
export const worldSessionOf = (id: number) => WORLD_SESSIONS.find((x) => x.tournaments.includes(id)) ?? null;

/** The last session's hands, newest first: demo hand ids count down from here (see `demoHands`). */
export const NEWEST_HAND = 250000000002;

/**
 * When a demo hand was played. The last session's hands count down from `NEWEST_HAND`, with all
 * its tables side by side; a tournament's own hands (`id * 1000 + 999 - n`) count back from its end.
 */
export function handTime(id: number): string {
  const last = WORLD_SESSIONS[0];
  if (id > 1e12 && id <= NEWEST_HAND) {
    const tables = Math.max(1, last.tables);
    return addMinutes(last.end, -((NEWEST_HAND - id) * HAND_SECONDS) / tables / 60 - 0.5);
  }
  const t = worldTournament(Math.floor(id / 1000));
  if (t) return addMinutes(t.ended_at, -((999 - (id % 1000)) * HAND_SECONDS) / 60);
  return addMinutes(last.end, -1);
}

/** The tournament whose report has a stored coach review. */
export const REVIEWED_TOURNAMENT = 4046787838;
