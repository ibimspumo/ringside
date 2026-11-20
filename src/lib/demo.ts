// Sample data so the UI can be designed and previewed in a plain browser without Tauri.
import { DEMO_HANDS } from "./analysis/demo-hands";
import { GG_TOURNAMENT, HAND_SECONDS, RUNNING_TOURNAMENT, TOTAL_HANDS, WORLD_SESSIONS, WORLD_TOURNAMENTS, addMinutes, handTime, minutesBetween, worldTournament } from "./demo-world";
import type { WorldSession } from "./demo-world";
import type {
  Dashboard,
  Site,
  Filter,
  GroupView,
  Hand,
  HandQuery,
  HandSummary,
  LeakCheck,
  LeakReport,
  LiveTable,
  NotesSource,
  PlayerNote,
  PlayerRead,
  PlayerType,
  PlayerView,
  Reports,
  SeatPos,
  Settings,
  ShownHand,
  StatId,
  StatValue,
  TableView,
  TypeRead,
  TournamentRow,
  TournamentTotals,
  Tournaments,
} from "./types";

/** Mirrors `default_layout` in ringside-core (measured PokerStars 8-seat table, else an oval). */
export function defaultLayout(n: number): SeatPos[] {
  if (n === 8) {
    return [
      [0.432, 0.723],
      [0.278, 0.629],
      [0.189, 0.406],
      [0.278, 0.198],
      [0.57, 0.129],
      [0.722, 0.198],
      [0.812, 0.406],
      [0.722, 0.629],
    ].map(([x, y]) => ({ x, y }));
  }
  return Array.from({ length: n }, (_, i) => {
    const a = Math.PI / 2 + (i * 2 * Math.PI) / n;
    return { x: 0.5 + 0.311 * Math.cos(a), y: 0.426 + 0.297 * Math.sin(a) };
  });
}

export const demoSettings: Settings = {
  hand_history_dirs: [],
  gg_dirs: [],
  preferred_seating: true,
  analysis: { auto: true },
  hud: {
    mode: "graphic",
    palette: "traffic",
    classic: { preset: "beginner", custom: ["vpip", "pfr", "three_bet", "af", "hands"] },
    graphic: { segments: ["vpip", "pfr", "af", "three_bet"], plate_pages: [
        ["vpip", "pfr", "three_bet"],
        ["fold_to_three_bet", "steal", "fold_to_steal"],
        ["cbet", "fold_to_cbet", "af"],
      ],
      cover_plate: true,
      plate_width: 1,
    },
    thresholds: {
      vpip: { low: 20, high: 35 },
      pfr: { low: 12, high: 25 },
      three_bet: { low: 4, high: 10 },
      fold_to_three_bet: { low: 45, high: 70 },
      steal: { low: 25, high: 45 },
      fold_to_steal: { low: 50, high: 75 },
      cbet: { low: 45, high: 70 },
      fold_to_cbet: { low: 35, high: 60 },
      wtsd: { low: 22, high: 32 },
      wsd: { low: 45, high: 55 },
      af: { low: 1.5, high: 3.5 },
      afq: { low: 35, high: 55 },
    },
    scale: 1,
    opacity: 1,
    split_by_game: true,
    show_stack: true,
    show_bounty: true,
    recap_seconds: 4,
    hero_session_only: true,
    recap_board: null,
  },
  layouts: {},
  presets: [],
};

const IDS: StatId[] = ["hands", "vpip", "pfr", "three_bet", "fold_to_three_bet", "steal", "fold_to_steal", "cbet", "fold_to_cbet", "wtsd", "wsd", "af", "afq"];

function stats(hands: number, v: Partial<Record<StatId, number>>): StatValue[] {
  return IDS.map((id) => {
    if (id === "hands") return { id, value: hands, made: hands, opp: hands };
    const value = v[id] ?? null;
    const opp = Math.round(hands * (id === "vpip" || id === "pfr" ? 1 : 0.22));
    const made = value === null ? 0 : Math.round(id === "af" ? value * opp * 0.4 : (value / 100) * opp);
    return { id, value, made, opp: id === "af" ? Math.round(opp * 0.4) : opp };
  });
}

/** The words the backend sends with each player type (crates/ringside-learn/src/hud.rs). */
const TYPES: Record<PlayerType, TypeRead> = {
  nit: { id: "nit", name: "Nit", long: "Nit (sehr eng)", sketch: "Spielt nur sehr starke Hände.", counter: "Öfter seine Blinds stehlen, gegen seine Raises folden.", why: "Er foldet fast alles. Wenn er doch erhöht, hat er eine starke Hand." },
  tag: { id: "tag", name: "TAG", long: "TAG (eng und aggressiv)", sketch: "Spielt wenige Hände, die aber mit Raise.", counter: "Solide spielen, ohne große Anpassung.", why: "Er spielt eng und meist mit Raise. Große Anpassungen gegen ihn lohnen selten." },
  lag: { id: "lag", name: "LAG", long: "LAG (locker und aggressiv)", sketch: "Spielt viele Hände und erhöht oft.", counter: "Öfter mitgehen und mit starken Händen selbst erhöhen.", why: "Er erhöht auch mit schwachen Händen. Starke Hände bekommen von ihm viel Geld." },
  station: { id: "station", name: "Calling Station", long: "Calling Station (locker und passiv)", sketch: "Spielt viele Hände, callt aber meist nur.", counter: "Mit guten Händen oft und groß setzen, kaum bluffen.", why: "Er foldet selten. Bluffs gehen nicht durch, gute Hände werden bezahlt." },
};

/** A read as the backend gives it; `code` is type letter (n, t, l, s, - for none) and sure (s), maybe (m) or unsure (u). */
function read(code: string, hands: number): PlayerRead {
  const type = ({ n: "nit", t: "tag", l: "lag", s: "station" } as const)[code[0] as "n" | "t" | "l" | "s"];
  const sure = ({ s: "sure", m: "maybe", u: "unsure" } as const)[code[1] as "s" | "m" | "u"];
  const word = { sure: "klares Muster", maybe: "knapp", unsure: "noch unsicher" }[sure];
  return { type: type ? TYPES[type] : null, hands, sure, note: `nach ${hands.toLocaleString("de-DE")} ${hands === 1 ? "Hand" : "Händen"} ${word}` };
}

/** Name, hands, stats, read over all hands and over the hands of the tournament slice. */
type Demo = [string, number, Partial<Record<StatId, number>>, string, string];
const PLAYERS: Demo[] = [
  ["Hero", TOTAL_HANDS, { vpip: 21, pfr: 16, three_bet: 6, fold_to_three_bet: 58, steal: 38, fold_to_steal: 71, cbet: 64, fold_to_cbet: 44, wtsd: 26, wsd: 53, af: 2.6, afq: 47 }, "ts", "tm"],
  ["Kestrel", 27, { vpip: 18, pfr: 15, three_bet: 7, fold_to_three_bet: 100, steal: 44, fold_to_steal: 71, cbet: 71, fold_to_cbet: 45, wtsd: 24, wsd: 55, af: 3.1, afq: 51 }, "tu", "tu"],
  ["marlowe_77", 33, { vpip: 41, pfr: 9, three_bet: 6, steal: 18, fold_to_steal: 48, cbet: 38, fold_to_cbet: 29, wtsd: 37, wsd: 44, af: 0.9, afq: 24 }, "sm", "sm"],
  ["Lee Ann", 412, { vpip: 33, pfr: 26, three_bet: 12, fold_to_three_bet: 40, steal: 52, fold_to_steal: 55, cbet: 80, fold_to_cbet: 30, wtsd: 22, wsd: 50, af: 4.2, afq: 58 }, "ls", "lu"],
  ["Ondine", 25, { vpip: 15, pfr: 12, three_bet: 0, fold_to_three_bet: 50, steal: 67, fold_to_steal: 80, cbet: 55, fold_to_cbet: 62, wtsd: 21, wsd: 52, af: 2.0, afq: 40 }, "tu", "tu"],
  ["juba_7", 8, { vpip: 50, pfr: 13, three_bet: 8, af: 1.0 }, "su", "su"],
  ["Nadir", 233, { vpip: 24, pfr: 19, three_bet: 0, fold_to_three_bet: 0, steal: 41, fold_to_steal: 100, cbet: 62, wtsd: 27, wsd: 54, af: 2.6, afq: 48 }, "tu", "tu"],
  ["Rüdiger", 96, { vpip: 32, pfr: 3, three_bet: 1, steal: 9, fold_to_steal: 62, cbet: 31, fold_to_cbet: 52, wtsd: 39, wsd: 41, af: 1.0, afq: 22 }, "su", "su"],
];

/** The user's PokerStars notes in the demo, by player. */
const NOTES: Record<string, PlayerNote> = {
  Kestrel: { label: { name: "Reg", color: "#FF304E" }, text: "3-bettet light aus den Blinds" },
  "Lee Ann": { label: { name: "Fisch", color: "#30DBFF" }, text: "" },
  Rüdiger: { label: { name: "Calling Station", color: "#F5C542" }, text: "Callt jeden Flop, foldet am River gegen Druck." },
  marlowe_77: { label: null, text: "Limpt viel, zahlt Value-Bets aus." },
};

export function demoNote(player: string): PlayerNote | null {
  return NOTES[player] ?? null;
}

export function demoNotesSources(): NotesSource[] {
  return [{ path: "C:\\Users\\du\\AppData\\Local\\PokerStars.EU\\notes.Hero.xml", players: Object.keys(NOTES).length, error: null }];
}

export function demoTable(maxSeats = 8): TableView {
  const seats = PLAYERS.slice(0, maxSeats).map(([name, hands, v, r], i) => {
    const stackBb = [61, 40, 66, 47, 82, 46, 78, 114][i];
    return {
      seat: i + 1,
      visual: i,
      name,
      stack: stackBb * 10000,
      stack_bb: stackBb,
      bounty: [345, 125, 250, 190, 125, 470, 125, 250][i],
      is_hero: i === 0,
      stats: stats(hands, v),
      read: read(r, hands),
      note: demoNote(name),
    };
  });
  return {
    table_key: "t:4046789300:10",
    title: "Mini Early Battle – Turnier 4046789300 Tisch 10",
    game: "tournament",
    max_seats: maxSeats,
    big_blind: 10000,
    currency: null,
    bounty_currency: "USD",
    hand_id: 2,
    seats,
    layout_key: `${maxSeats}`,
    layout: defaultLayout(maxSeats),
    hud: structuredClone(demoSettings.hud),
    recap: {
      hand_id: 2,
      board: ["Jd", "8s", "Js", "4h", "2c"],
      pot: 220000,
      shown: [
        [0, ["Ah", "Ad"]],
        [3, ["Kc", "Jc"]],
      ],
      winners: [[3, 220000]],
      bounties: [],
    },
  };
}

/** With a tournament chosen, the demo shows a smaller slice of the same players. */
const inTournament = (f: Filter) => f.tournament_id !== null && f.tournament_id !== undefined;

/** Reads of `manyPlayers` in order, codes as in `read`. */
const MANY_READS = "ns ss lm tm ss tu su tu ss ls nu ss tu su tu su ls ts ss tm su tu su lu ns ss tu -u tm su lu tu ss ts ss tu ss lu nu su ts -u ts ss lu tu su lu ss tu ss lm nu ss tu -u ts ss ls tu ss lu su nu".split(" ");

/** `?many=1`: a long list with long names, for density and overflow. */
function manyPlayers(): PlayerView[] {
  return Array.from({ length: 64 }, (_, i) => {
    const [, , v] = PLAYERS[1 + (i % (PLAYERS.length - 1))];
    const shift = ((i * 7) % 11) - 5;
    const name = i % 9 === 0 ? `Ein_sehr_langer_Spielername_${i}` : `Gegner_${String(i + 1).padStart(2, "0")}`;
    const tuned = Object.fromEntries(Object.entries(v).map(([k, x]) => [k, Math.max(0, k === "af" ? x : x + shift)]));
    const hands = [1_234_567, 2400, 640, 180, 75, 31, 12, 4][i % 8];
    return { name, net: 0, stats: stats(hands, tuned), read: read(MANY_READS[i], hands) };
  });
}

export function demoPlayers(filter?: Filter): PlayerView[] {
  if (new URLSearchParams(window.location.search).get("many") === "1") return [...PLAYERS.slice(0, 1).map(([name, hands, v, r]) => ({ name, net: 0, stats: stats(hands, v), read: read(r, hands) })), ...manyPlayers()];
  const all = PLAYERS.map(([name, hands, v, r], i) => ({ name, net: [0, 920, -1830, 4210, -310, -2650, 330, -1200][i], stats: stats(hands, v), read: read(r, hands) }));
  if (!filter || !inTournament(filter)) return all;
  const own = worldTournament(filter.tournament_id!)?.row.hands ?? 212;
  return PLAYERS.slice(0, 6).map(([name, , v, , r], i) => {
    // Hands with the hero: never more than the hero played there.
    const hands = i === 0 ? own : Math.min(own, [0, 27, 33, 140, 25, 8][i]);
    return { name, net: [0, 410, -980, 1250, -60, -2650][i], stats: stats(hands, v), read: read(r, hands) };
  });
}

/** The sessions a filter covers: one tournament, one session (from–until), or everything. */
function scope(filter?: Filter): { sessions: WorldSession[]; hands: number; net_bb: number; bounties: Record<string, number> } {
  const t = filter?.tournament_id != null ? worldTournament(filter.tournament_id) : null;
  if (t) {
    const session = WORLD_SESSIONS.find((x) => x.tournaments.includes(t.row.id))!;
    const one = { ...session, hands: t.row.hands, net_bb: t.net_bb, start: t.row.started_at, end: t.ended_at, cash_hands: 0, tournaments: [t.row.id] };
    return { sessions: [one], hands: t.row.hands, net_bb: t.net_bb, bounties: t.row.bounties && t.row.currency ? { [t.row.currency]: t.row.bounties } : {} };
  }
  const { since, until } = filter ?? {};
  const sessions = since && until ? WORLD_SESSIONS.filter((x) => x.start >= since && x.start <= until) : WORLD_SESSIONS;
  const bounties: Record<string, number> = {};
  for (const x of sessions) for (const [c, v] of Object.entries(x.bounties)) bounties[c] = (bounties[c] ?? 0) + v;
  return {
    sessions,
    hands: sessions.reduce((n, x) => n + x.hands, 0),
    net_bb: Math.round(sessions.reduce((n, x) => n + x.net_bb, 0) * 10) / 10,
    bounties,
  };
}

/** The hero's hands of `sessions` (newest first) as one curve, oldest hand first; every session ends on its own result. */
function series(sessions: WorldSession[], seed: number) {
  let s = seed;
  const rnd = () => (s = (s * 16807) % 2147483647) / 2147483647 - 0.5;
  const n = sessions.reduce((a, x) => a + x.hands, 0);
  const every = Math.ceil(n / 1500);
  let total = 0;
  let sd = 0;
  let luck = 0;
  let allIns = 0;
  let i = 0;
  const points = [];
  for (const x of [...sessions].reverse()) {
    const steps = Array.from({ length: x.hands }, () => rnd() * 9);
    const drift = (x.net_bb - steps.reduce((a, b) => a + b, 0)) / Math.max(1, x.hands);
    const minutes = minutesBetween(x.start, x.end);
    for (let k = 0; k < x.hands; k++) {
      const net = steps[k] + drift;
      total += net;
      i++;
      if (rnd() > 0.3) sd += net * 1.6;
      // About one all-in with every hand shown in 40 hands; its result swings around the EV.
      if (rnd() > 0.475) {
        allIns++;
        luck += rnd() * 30 + 0.4;
      }
      if (i % every === 0 || i === n)
        points.push({
          n: i,
          played_at: addMinutes(x.start, (minutes * (k + 1)) / x.hands),
          net_bb: total,
          showdown_bb: sd,
          non_showdown_bb: total - sd,
          ev_bb: total - luck,
        });
    }
  }
  return { total, sd, points, luck, allIns };
}

export function demoDashboard(filter?: Filter): Dashboard {
  const sc = scope(filter);
  const s = series(sc.sessions, filter?.tournament_id ? filter.tournament_id % 997 : 11);
  return {
    hand_count: TOTAL_HANDS,
    heroes: ["Hero"],
    results: {
      hands: sc.hands,
      net: Math.round(s.total * 100),
      net_bb: sc.net_bb,
      showdown_bb: s.sd,
      non_showdown_bb: sc.net_bb - s.sd,
      ev_bb: sc.net_bb - s.luck,
      all_ins: s.allIns,
      luck_bb: s.luck,
      bounties: sc.bounties,
      points: s.points,
    },
    hero: demoPlayers(filter)[0],
  };
}

export function emptyDashboard(): Dashboard {
  return {
    hand_count: 0,
    heroes: [],
    results: {
      hands: 0,
      net: 0,
      net_bb: 0,
      showdown_bb: 0,
      non_showdown_bb: 0,
      ev_bb: 0,
      all_ins: 0,
      luck_bb: 0,
      bounties: {},
      points: [],
    },
    hero: null,
  };
}

const RANKS = "AKQJT98765432";

/** `parts` rounded to whole numbers that add up to `total` (largest remainder). */
function apportion(total: number, parts: number[]): number[] {
  const sum = parts.reduce((a, b) => a + b, 0) || 1;
  const exact = parts.map((p) => (p / sum) * total);
  const out = exact.map(Math.floor);
  const order = exact.map((x, i) => [x - Math.floor(x), i]).sort((x, y) => y[0] - x[0]);
  const missing = total - out.reduce((a, b) => a + b, 0);
  for (let k = 0; k < missing; k++) out[order[k % order.length][1]]++;
  return out;
}

export function demoReports(filter?: Filter): Reports {
  const sc = scope(filter);
  const posHands = apportion(sc.hands, [690, 512, 701, 745, 768, 790, 801, 817]);
  // Blinds lose, the button wins most; whatever is left of the total goes to the seats that open.
  const per100 = [-1.7, 0.8, 2.6, 4.7, 7.9, 15.2, -11.0, -18.4];
  const base = per100.map((b, i) => (b * posHands[i]) / 100);
  const open = posHands.slice(0, 6).reduce((a, b) => a + b, 0) || 1;
  const rest = sc.net_bb - base.reduce((a, b) => a + b, 0);
  const nets = base.map((b, i) => Math.round((i < 6 ? b + (rest * posHands[i]) / open : b) * 10) / 10);
  nets[5] = Math.round((nets[5] + sc.net_bb - nets.reduce((a, b) => a + b, 0)) * 10) / 10;
  const positions: GroupView[] = ["UTG", "UTG+1", "LJ", "HJ", "CO", "BTN", "SB", "BB"].map((key, i) => {
    const hands = posHands[i];
    const netBb = nets[i];
    return { key, net: Math.round(netBb * 100), net_bb: netBb, stats: stats(hands, { vpip: [12, 14, 17, 21, 27, 38, 30, 34][i], pfr: [11, 12, 15, 18, 23, 31, 18, 9][i], steal: i >= 4 && i <= 6 ? [32, 44, 30][i - 4] : undefined }) };
  });
  const raw: { key: string; hands: number; strength: number }[] = [];
  for (let r = 0; r < 13; r++)
    for (let c = 0; c < 13; c++) {
      const hi = RANKS[Math.min(r, c)];
      const lo = RANKS[Math.max(r, c)];
      const key = r === c ? hi + lo : r < c ? `${hi}${lo}s` : `${hi}${lo}o`;
      const strength = 26 - r - c + (r === c ? 8 : 0) + (r < c ? 2 : 0);
      raw.push({ key, strength, hands: Math.max(1, Math.round(10 + ((r * 7 + c * 3) % 9) * (r === c ? 1 : r < c ? 1.5 : 4))) });
    }
  // The same hands and the same total as everywhere else: counts scaled, results shifted by hands.
  const classHands = apportion(sc.hands, raw.map((x) => x.hands));
  const shaped = raw.map((x, i) => (x.strength - 9) * 0.9 * (classHands[i] / 20));
  const shift = (sc.net_bb - shaped.reduce((a, b) => a + b, 0)) / Math.max(1, sc.hands);
  const starting_hands: GroupView[] = raw.flatMap((x, i) => {
    const hands = classHands[i];
    if (!hands) return [];
    const netBb = shaped[i] + shift * hands;
    const played = Math.max(0, Math.min(100, x.strength * 5 - 25));
    return [{ key: x.key, net: Math.round(netBb * 100), net_bb: netBb, stats: stats(hands, { vpip: played, pfr: played * 0.8 }) }];
  });
  const sessions = sc.sessions.map((x) => ({
    start: x.start,
    end: x.end,
    hands: x.hands,
    net: 0,
    net_bb: x.net_bb,
    bounties: x.bounties,
    tables: x.tables,
    sites: x.sites,
  }));
  return { positions, starting_hands, sessions };
}

export function demoShown(): ShownHand[] {
  return [
    { hand_id: 9, played_at: "2026/11/16 21:04:10", cards: "Kc Jc", board: "Jd 8s Js 4h 2c", position: "BTN", net: 110000, big_blind: 10000 },
    { hand_id: 8, played_at: "2026/11/16 20:51:44", cards: "7h 7d", board: "Ah Kd 7c 2s 9h", position: "BB", net: 64000, big_blind: 8000 },
    { hand_id: 7, played_at: "2026/11/14 22:13:02", cards: "As 4s", board: "Qs 9s 3d Td 2h", position: "CO", net: -36000, big_blind: 6000 },
  ];
}

export const demoHand: Hand = {
  id: 250000000002,
  game: "cash",
  tournament_id: null,
  variant: "Hold'em No Limit",
  table_name: "Aquila II",
  max_seats: 6,
  button_seat: 4,
  small_blind: 5,
  big_blind: 10,
  currency: "USD",
  played_at: handTime(250000000002),
  seats: [
    { seat: 1, name: "Alice", stack: 1000, bounty: null, sitting_out: false },
    { seat: 2, name: "Bob", stack: 1140, bounty: null, sitting_out: false },
    { seat: 3, name: "Carol", stack: 800, bounty: null, sitting_out: false },
    { seat: 4, name: "Dave", stack: 995, bounty: null, sitting_out: false },
    { seat: 5, name: "Hero", stack: 1107, bounty: null, sitting_out: false },
    { seat: 6, name: "Eve", stack: 1000, bounty: null, sitting_out: false },
  ],
  hero: "Hero",
  hero_cards: ["9s", "9d"],
  board: ["9c", "4h", "2s", "Kd", "3c"],
  actions: [
    { street: "preflop", player: "Hero", kind: "post_small_blind", amount: 5, to: null, all_in: false },
    { street: "preflop", player: "Eve", kind: "post_big_blind", amount: 10, to: null, all_in: false },
    { street: "preflop", player: "Alice", kind: "call", amount: 10, to: null, all_in: false },
    { street: "preflop", player: "Bob", kind: "fold", amount: 0, to: null, all_in: false },
    { street: "preflop", player: "Carol", kind: "fold", amount: 0, to: null, all_in: false },
    { street: "preflop", player: "Dave", kind: "fold", amount: 0, to: null, all_in: false },
    { street: "preflop", player: "Hero", kind: "call", amount: 5, to: null, all_in: false },
    { street: "preflop", player: "Eve", kind: "check", amount: 0, to: null, all_in: false },
    { street: "flop", player: "Hero", kind: "check", amount: 0, to: null, all_in: false },
    { street: "flop", player: "Eve", kind: "bet", amount: 20, to: null, all_in: false },
    { street: "flop", player: "Alice", kind: "call", amount: 20, to: null, all_in: false },
    { street: "flop", player: "Hero", kind: "raise", amount: 80, to: 80, all_in: false },
    { street: "flop", player: "Eve", kind: "fold", amount: 0, to: null, all_in: false },
    { street: "flop", player: "Alice", kind: "call", amount: 60, to: null, all_in: false },
    { street: "turn", player: "Hero", kind: "bet", amount: 150, to: null, all_in: false },
    { street: "turn", player: "Alice", kind: "call", amount: 150, to: null, all_in: false },
    { street: "river", player: "Hero", kind: "bet", amount: 300, to: null, all_in: false },
    { street: "river", player: "Alice", kind: "raise", amount: 760, to: 760, all_in: true },
    { street: "river", player: "Hero", kind: "call", amount: 460, to: null, all_in: false },
  ],
  returned: [],
  shown: [
    ["Alice", ["Ah", "5c"]],
    ["Hero", ["9s", "9d"]],
  ],
  collected: [["Alice", 1930]],
  showdown: true,
  total_pot: 2030,
  rake: 100,
  bounties_won: [],
  bounty_currency: null,
};

/** GGPoker ids in Ringside's id space (`gg_hand_id` in ringside-core): "TM5100200301". */
function ggHandId(prefix: string, n: number): number {
  const code = (prefix.charCodeAt(0) - 65) * 26 + (prefix.charCodeAt(1) - 65);
  return 2 ** 52 + code * 6e12 + n;
}

/** The demo's GGPoker tournament: "Bounty Hunters $10.80" (#241300777). */
export { GG_TOURNAMENT };
const GG = worldTournament(GG_TOURNAMENT)!;

/** A GGPoker tournament hand: the hero is "Hero", the others anonymous ids. */
export const demoGgHand: Hand = {
  id: ggHandId("TM", 5100200301),
  site: "ggpoker",
  game: "tournament",
  tournament_id: GG_TOURNAMENT,
  variant: "Hold'em No Limit",
  table_name: "12",
  max_seats: 8,
  button_seat: 3,
  small_blind: 15000,
  big_blind: 30000,
  currency: null,
  played_at: addMinutes(GG.ended_at, -1),
  seats: [
    { seat: 1, name: "3a9f0c21", stack: 1420000, bounty: null, sitting_out: false },
    { seat: 2, name: "b7e44d0", stack: 985000, bounty: null, sitting_out: false },
    { seat: 3, name: "Hero", stack: 360000, bounty: null, sitting_out: false },
    { seat: 4, name: "5c2d81fe", stack: 2241000, bounty: null, sitting_out: false },
    { seat: 5, name: "e19a7b33", stack: 730000, bounty: null, sitting_out: false },
    { seat: 6, name: "f4c6d9a", stack: 1100000, bounty: null, sitting_out: false },
    { seat: 7, name: "82be15c7", stack: 625000, bounty: null, sitting_out: false },
  ],
  hero: "Hero",
  hero_cards: ["Ah", "Qh"],
  board: ["7s", "2h", "Qc", "9c", "4h"],
  actions: [
    ...["3a9f0c21", "b7e44d0", "Hero", "5c2d81fe", "e19a7b33", "f4c6d9a", "82be15c7"].map((player) => ({
      street: "preflop" as const,
      player,
      kind: "post_ante" as const,
      amount: 4000,
      to: null,
      all_in: false,
    })),
    { street: "preflop", player: "5c2d81fe", kind: "post_small_blind", amount: 15000, to: null, all_in: false },
    { street: "preflop", player: "e19a7b33", kind: "post_big_blind", amount: 30000, to: null, all_in: false },
    { street: "preflop", player: "f4c6d9a", kind: "fold", amount: 0, to: null, all_in: false },
    { street: "preflop", player: "82be15c7", kind: "fold", amount: 0, to: null, all_in: false },
    { street: "preflop", player: "3a9f0c21", kind: "fold", amount: 0, to: null, all_in: false },
    { street: "preflop", player: "b7e44d0", kind: "fold", amount: 0, to: null, all_in: false },
    { street: "preflop", player: "Hero", kind: "raise", amount: 356000, to: 356000, all_in: true },
    { street: "preflop", player: "5c2d81fe", kind: "fold", amount: 0, to: null, all_in: false },
    { street: "preflop", player: "e19a7b33", kind: "call", amount: 326000, to: null, all_in: false },
  ],
  returned: [],
  shown: [
    ["Hero", ["Ah", "Qh"]],
    ["e19a7b33", ["Kc", "Kd"]],
  ],
  collected: [["e19a7b33", 755000]],
  showdown: true,
  total_pot: 755000,
  rake: 0,
  bounties_won: [],
  bounty_currency: null,
};

/** Hands listed for a chosen tournament carry its id (see `demoHands`). */
export function demoHandById(id: number): Hand {
  const own = DEMO_HANDS.find((h) => h.id === id);
  if (own) return { ...own, played_at: handTime(id) };
  if (id >= 2 ** 52) return { ...demoGgHand, id };
  if (id < 1e12) return { ...demoHand, id, played_at: handTime(id) };
  const tournament = Math.floor(id / 1000);
  return { ...demoHand, id, game: "tournament", tournament_id: tournament, table_name: `${tournament} 10`, currency: null, played_at: handTime(id) };
}

export function demoHands(query?: HandQuery): HandSummary[] {
  const tournament = query?.tournament_id ?? null;
  if (tournament === GG_TOURNAMENT || query?.site === "ggpoker") return demoGgHands(tournament === null ? 6 : 18);
  const cashAt = (i: number) => tournament === null && i % 3 === 2;
  const list: HandSummary[] = Array.from({ length: tournament === null ? 40 : 18 }, (_, i) => ({
    id: tournament === null ? 250000000002 - i : tournament * 1000 + 999 - i,
    site: "pokerstars",
    game: cashAt(i) ? "cash" : "tournament",
    played_at: handTime(tournament === null ? 250000000002 - i : tournament * 1000 + 999 - i),
    table_name: cashAt(i) ? "Aquila II" : `${tournament ?? RUNNING_TOURNAMENT} 10`,
    small_blind: 5,
    big_blind: 10,
    currency: cashAt(i) ? "USD" : null,
    hero_cards: ["9s 9d", "Ah 7c", "Ah Qd", "8d 3s", "Jd Td", "As 5s", "8h 6h"][i % 7],
    hero_position: ["SB", "BTN", "BTN", "SB", "UTG", "HJ", "LJ"][i % 7],
    hero_net: [-1000, -6, 101, -65, 245, -30, 60][i % 7],
    hero_showdown: i % 4 === 0,
    pot: [2030, 1360, 211, 2225, 510, 60, 120][i % 7],
    board: ["9c 4h 2s Kd 3c", "", "Qs 7c 2d 5h Jc", "Kd 9s 4c 2h Jd", "Td 9d 2s 4c 8h", "Kh 3s 3d", "Qc Jc 5d 2h"][i % 7],
  }));
  if (tournament !== null || query?.site === "pokerstars") return list;
  // The evening's GGPoker tournament ran next to the PokerStars tables.
  const gg = demoGgHands(6);
  return [...list.slice(0, 1), gg[0], ...list.slice(1, 4), gg[1], ...list.slice(4, 9), ...gg.slice(2), ...list.slice(9)];
}

function demoGgHands(n: number): HandSummary[] {
  return Array.from({ length: n }, (_, i) => ({
    id: ggHandId("TM", 5100200301 - i * 13),
    site: "ggpoker",
    game: "tournament",
    played_at: addMinutes(GG.ended_at, -1 - (i * HAND_SECONDS) / 60),
    table_name: "12",
    small_blind: 15000,
    big_blind: 30000,
    currency: null,
    hero_cards: ["Ah Qh", "Jc Jd", "Ks 9s", "7d 7c", "Ac 4c", "Qd Td"][i % 6],
    hero_position: ["BTN", "BB", "CO", "HJ", "SB", "UTG"][i % 6],
    hero_net: [-360000, 116500, -64000, 0, 31000, -4000][i % 6],
    hero_showdown: i % 6 === 0,
    pot: [755000, 213000, 96000, 28000, 75000, 28000][i % 6],
    board: ["7s 2h Qc 9c 4h", "Js 8h 3c", "Kd 8c 2s", "", "Ac 9d 5h", ""][i % 6],
  }));
}

export function demoLiveTables(): LiveTable[] {
  if (new URLSearchParams(window.location.search).get("tables") === "0") return [];
  const mtt = demoTable();
  const cash = demoTable(6);
  cash.table_key = "c:Aquila II";
  cash.title = "Aquila II - $0.05/$0.10 USD - No Limit Hold'em";
  cash.game = "cash";
  cash.big_blind = 10;
  cash.currency = "USD";
  cash.bounty_currency = null;
  cash.seats = cash.seats.map((seat, i) => {
    const name = i === 0 ? "Hero" : ["Ondine", "Kestrel", "Rüdiger", "Nadir", "marlowe_77"][i - 1];
    const same = mtt.seats.find((s) => s.name === name) ?? seat;
    return { ...seat, stack: seat.stack_bb * 10, bounty: null, name, stats: same.stats, read: same.read, note: demoNote(name) };
  });
  return [
    { title: "$5.50 Mini Early Battle - 50/100 Ante 12 - Turnier 4046789300 Tisch 10", table_key: mtt.table_key, table: mtt },
    { title: cash.title, table_key: cash.table_key, table: cash },
    { title: "$11 Bounty Hunters - Turnier 4046799120 Tisch 4", table_key: "t:4046799120:4", table: null },
  ];
}

/** Mirrors `totals` in ringside-core (tournament.rs) for the browser demo. */
function demoTotals(rows: TournamentRow[]): TournamentTotals[] {
  const by = new Map<string, TournamentTotals>();
  for (const r of rows) {
    const key = r.currency ?? "";
    const t = by.get(key) ?? { currency: key, tournaments: 0, finished: 0, cashes: 0, cost: 0, prizes: 0, bounties: 0, profit: 0, roi: null, itm: null };
    by.set(key, t);
    t.tournaments++;
    if (r.cost === null || r.prize === null || r.profit === null) continue;
    t.finished++;
    t.cashes += r.prize > 0 ? 1 : 0;
    t.cost += r.cost;
    t.prizes += r.prize;
    t.bounties += r.bounties;
    t.profit += r.profit;
  }
  return [...by.values()]
    .map((t) => ({ ...t, roi: t.cost > 0 ? (t.profit / t.cost) * 100 : null, itm: t.finished > 0 ? (t.cashes / t.finished) * 100 : null }))
    .sort((a, b) => Number(a.currency === "") - Number(b.currency === "") || b.tournaments - a.tournaments);
}

export function demoTournaments(): Tournaments {
  const rows = WORLD_TOURNAMENTS.map((t) => ({ ...t.row }));
  return { rows, totals: demoTotals(rows) };
}

const DEMO_SOURCES: LeakReport["sources"] = [
  { id: "pcg", title: "HUD Stats Glossary: VPIP, PFR, 3-Bet% and 20 More Explained", publisher: "Poker Club Games", url: "https://pokerclubgames.com/poker-news/hud-stats-glossary-vpip-pfr-3-bet-and-20-more-explained/", scope: "Grobe Richtwerte für einen soliden Regular, Online 6-max No-Limit Cash" },
  { id: "pokercoaching", title: "How to Use Poker HUD Stats to Exploit Weak Players", publisher: "PokerCoaching.com", url: "https://pokercoaching.com/blog/poker-stats/", scope: "Typische Werte guter Spieler, 6-max und Full Ring; VPIP auch für 6-max-Turniere" },
  { id: "rangerpro", title: "Short stack MTT strategy: push-fold and open ranges", publisher: "RangerPro", url: "https://rangerpro.fr/poker-guide/short-stack", scope: "Eröffnungsbreite je Position bei 15–25 BB im Turnier" },
  { id: "nash", title: "Push/Fold-Gleichgewicht (Chip-EV)", publisher: "Ringside-Push/Fold-Tabellen", url: null, scope: "Nash-Gleichgewicht für 8 Spieler mit Big-Blind-Ante bei 5, 7,5 und 10 BB; im Bereich Push/Fold nachrechenbar" },
];

function demoCheck(c: Partial<LeakCheck> & Pick<LeakCheck, "stat" | "made" | "opp" | "low" | "high">): LeakCheck {
  const value = (100 * c.made) / c.opp;
  const direction = value > c.high ? "too_high" : value < c.low ? "too_low" : null;
  const spread = 196 * Math.sqrt((value / 100) * (1 - value / 100) / c.opp);
  return {
    game: "tournament",
    positions: [],
    band: 2,
    value,
    ci_low: Math.max(0, value - spread),
    ci_high: Math.min(100, value + spread),
    direction,
    deviation: direction === "too_high" ? value - c.high : direction === "too_low" ? c.low - value : 0,
    confidence: 0,
    uncertain: false,
    score: 0,
    cost: null,
    cites: [{ source: "pokercoaching", says: "um 60" }],
    note: null,
    ...c,
  };
}

export function demoLeaks(filter?: Filter): LeakReport {
  const cash = filter?.game === "cash";
  if (cash) return { leaks: [], fine: [], too_small: 11, bands: ["unter 10 BB", "10–20 BB", "20–40 BB", "ab 40 BB"], sources: DEMO_SOURCES };
  return {
    leaks: [
      demoCheck({
        stat: "fold_to_steal", positions: ["SB", "BB"], band: 1, made: 141, opp: 182, low: 50, high: 70,
        confidence: 0.996, score: 0.37,
        cost: { hands: 1240, spot_bb100: -21.4, base_bb100: -9.8, margin_bb100: 8.1, total_bb: 143.8 },
        cites: [{ source: "pokercoaching", says: "um 60 (Turnier-Wert)" }],
        note: "Eine Angabe „um X“ ist hier als X ± 10 Punkte gelesen.",
      }),
      demoCheck({
        stat: "steal", positions: ["BTN"], band: 1, made: 38, opp: 141, low: 33, high: 48, confidence: 0.93, score: 0.33,
        cites: [{ source: "rangerpro", says: "Button ~36 % bei 15 BB, ~45 % bei 20 BB" }],
        note: "Chartwerte sind Eröffnungen, wenn alle vor dir gefoldet haben; der Bereich lässt einige Punkte Spiel.",
      }),
      demoCheck({
        stat: "wtsd", band: 2, made: 118, opp: 352, low: 24, high: 30, confidence: 0.91, score: 0.5,
        cites: [{ source: "pcg", says: "24–30 %" }, { source: "pokercoaching", says: "um 30" }],
        note: "Aus Cash-Quellen übernommen; im Turnier erst ab 20 BB geprüft.",
      }),
      demoCheck({
        stat: "steal", positions: ["SB"], band: 0, made: 9, opp: 17, low: 68, high: 86, confidence: 0.86, score: 0.6, uncertain: true,
        cites: [{ source: "nash", says: "Shove 72 % bei 10 BB, 77 % bei 7,5 BB, 84 % bei 5 BB" }],
      }),
    ],
    fine: [
      demoCheck({ stat: "vpip", band: 2, made: 512, opp: 2210, low: 15, high: 30 }),
      demoCheck({ stat: "pfr", band: 2, made: 371, opp: 2210, low: 10, high: 22 }),
      demoCheck({ stat: "cbet", band: 3, made: 61, opp: 98, low: 50, high: 70 }),
    ],
    too_small: 14,
    bands: ["unter 10 BB", "10–20 BB", "20–40 BB", "ab 40 BB"],
    sources: DEMO_SOURCES,
  };
}

/** Browser preview `?data=big`: long names, seven-digit numbers and long lists for the edge cases. */
export function bigDashboard(filter?: Filter): Dashboard {
  const d = demoDashboard(filter);
  const k = 2437;
  const r = d.results;
  const name = "Sehr_Langer_Spielername_2026";
  return {
    ...d,
    heroes: [name],
    hero: d.hero && { ...d.hero, name },
    results: {
      ...r,
      hands: r.hands * 41,
      net_bb: r.net_bb * k,
      ev_bb: r.ev_bb * k,
      showdown_bb: r.showdown_bb * k,
      non_showdown_bb: r.non_showdown_bb * k,
      luck_bb: r.luck_bb * k,
      all_ins: r.all_ins * 41,
      bounties: { USD: 123456789, EUR: 4567800 },
      points: r.points.map((p) => ({
        ...p,
        n: p.n * 41,
        net_bb: p.net_bb * k,
        ev_bb: p.ev_bb * k,
        showdown_bb: p.showdown_bb * k,
        non_showdown_bb: p.non_showdown_bb * k,
      })),
    },
  };
}

/** About 4000 tournaments, the history of 200 000 hands, for timing long lists. */
export function scaleTournaments(): Tournaments {
  const t = demoTournaments();
  const rows = Array.from({ length: 120 }, (_, copy) =>
    t.rows.map((r) => ({ ...r, id: r.id + copy * 1_000_000, started_at: r.started_at.replace(/^\d{4}/, String(2026 - Math.floor(copy / 12))) })),
  ).flat();
  const totals = t.totals.map((x) => ({
    ...x,
    tournaments: x.tournaments * 120,
    finished: x.finished * 120,
    cashes: x.cashes * 120,
    cost: x.cost * 120,
    prizes: x.prizes * 120,
    bounties: x.bounties * 120,
    profit: x.profit * 120,
  }));
  return { rows, totals };
}

export function bigTournaments(): Tournaments {
  const t = demoTournaments();
  const rows = Array.from({ length: 5 }, (_, copy) =>
    t.rows.map((r, i) => ({
      ...r,
      id: r.id + copy * 1_000_000,
      name: i % 3 === 0 ? "$109 Sunday Million Anniversary Edition – Day 2 [Progressive Knockout, Turbo, 8-Max]" : r.name,
      entrants: r.entrants === null ? null : r.entrants * 120,
      place: r.place === null ? null : r.place * 120,
      prize: r.prize === null ? null : r.prize * 1000,
      bounties: r.bounties * 1000,
      // Prizes and bounties a thousand times larger, the buy-in as it was: the same sum as the totals.
      profit: r.profit === null ? null : r.profit + ((r.prize ?? 0) + r.bounties) * 999,
    })),
  ).flat();
  const totals = t.totals.map((x) => ({
    ...x,
    tournaments: x.tournaments * 5,
    finished: x.finished * 5,
    cashes: x.cashes * 5,
    // Five copies of every row with prizes, bounties and results a thousand times larger.
    cost: x.cost * 5,
    prizes: x.prizes * 5000,
    bounties: x.bounties * 5000,
    profit: (x.prizes + x.bounties) * 5000 - x.cost * 5,
    roi: x.cost > 0 ? (((x.prizes + x.bounties) * 5000 - x.cost * 5) / (x.cost * 5)) * 100 : null,
  }));
  return { rows, totals };
}

export function bigLeaks(filter?: Filter): LeakReport {
  const l = demoLeaks(filter);
  const positions = [["UTG"], ["HJ"], ["CO"], ["BTN"], ["SB", "BB"]];
  return {
    ...l,
    leaks: positions.flatMap((p) => l.leaks.map((x) => ({ ...x, positions: p }))),
    fine: positions.flatMap((p) => l.fine.map((x) => ({ ...x, positions: p }))),
    too_small: 1234,
  };
}
