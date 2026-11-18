// Sample data so the UI can be designed and previewed in a plain browser without Tauri.
import { DEMO_HANDS } from "./analysis/demo-hands";
import type {
  Dashboard,
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
  PlayerView,
  Reports,
  SeatPos,
  Settings,
  ShownHand,
  StatId,
  StatValue,
  TableView,
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

type Demo = [string, number, Partial<Record<StatId, number>>];
const PLAYERS: Demo[] = [
  ["Hero", 6124, { vpip: 21, pfr: 16, three_bet: 6, fold_to_three_bet: 58, steal: 38, fold_to_steal: 71, cbet: 64, fold_to_cbet: 44, wtsd: 26, wsd: 53, af: 2.6, afq: 47 }],
  ["Kestrel", 27, { vpip: 18, pfr: 15, three_bet: 7, fold_to_three_bet: 100, steal: 44, fold_to_steal: 71, cbet: 71, fold_to_cbet: 45, wtsd: 24, wsd: 55, af: 3.1, afq: 51 }],
  ["marlowe_77", 33, { vpip: 41, pfr: 9, three_bet: 6, steal: 18, fold_to_steal: 48, cbet: 38, fold_to_cbet: 29, wtsd: 37, wsd: 44, af: 0.9, afq: 24 }],
  ["Lee Ann", 412, { vpip: 33, pfr: 26, three_bet: 12, fold_to_three_bet: 40, steal: 52, fold_to_steal: 55, cbet: 80, fold_to_cbet: 30, wtsd: 22, wsd: 50, af: 4.2, afq: 58 }],
  ["Ondine", 25, { vpip: 15, pfr: 12, three_bet: 0, fold_to_three_bet: 50, steal: 67, fold_to_steal: 80, cbet: 55, fold_to_cbet: 62, wtsd: 21, wsd: 52, af: 2.0, afq: 40 }],
  ["juba_7", 8, { vpip: 50, pfr: 13, three_bet: 8, af: 1.0 }],
  ["Nadir", 233, { vpip: 24, pfr: 19, three_bet: 0, fold_to_three_bet: 0, steal: 41, fold_to_steal: 100, cbet: 62, wtsd: 27, wsd: 54, af: 2.6, afq: 48 }],
  ["Rüdiger", 96, { vpip: 32, pfr: 3, three_bet: 1, steal: 9, fold_to_steal: 62, cbet: 31, fold_to_cbet: 52, wtsd: 39, wsd: 41, af: 1.0, afq: 22 }],
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
  const seats = PLAYERS.slice(0, maxSeats).map(([name, hands, v], i) => {
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

export function demoPlayers(filter?: Filter): PlayerView[] {
  const all = PLAYERS.map(([name, hands, v], i) => ({ name, net: [0, 920, -1830, 4210, -310, -2650, 330, -1200][i], stats: stats(hands, v) }));
  if (!filter || !inTournament(filter)) return all;
  return PLAYERS.slice(0, 6).map(([name, , v], i) => ({ name, net: [0, 410, -980, 1250, -60, -2650][i], stats: stats([212, 27, 33, 140, 25, 8][i], v) }));
}

function series(n: number, drift: number, seed: number) {
  let s = seed;
  const rnd = () => (s = (s * 16807) % 2147483647) / 2147483647 - 0.5;
  let total = 0;
  let sd = 0;
  let nsd = 0;
  let luck = 0;
  let allIns = 0;
  const points = [];
  const every = Math.ceil(n / 1500);
  for (let i = 1; i <= n; i++) {
    const net = rnd() * 9 + drift;
    total += net;
    if (rnd() > 0.3) sd += net * 1.6;
    else nsd += net - net * 1.6 * 0.7;
    // About one all-in with every hand shown in 40 hands; its result swings around the EV.
    if (rnd() > 0.475) {
      allIns++;
      luck += rnd() * 30 + 0.4;
    }
    if (i % every === 0 || i === n)
      points.push({
        n: i,
        played_at: `2026/11/${String(1 + Math.floor((i / n) * 16)).padStart(2, "0")} 20:00:00`,
        net_bb: total,
        showdown_bb: sd,
        non_showdown_bb: total - sd,
        ev_bb: total - luck,
      });
  }
  void nsd;
  return { total, sd, points, luck, allIns };
}

export function demoDashboard(filter?: Filter): Dashboard {
  const one = !!filter && inTournament(filter);
  const hands = one ? 212 : 6124;
  const s = series(hands, one ? 0.15 : 0.06, one ? (filter.tournament_id ?? 1) % 997 : 11);
  return {
    hand_count: 6431,
    heroes: ["Hero"],
    results: {
      hands,
      net: Math.round(s.total * 100),
      net_bb: s.total,
      showdown_bb: s.sd,
      non_showdown_bb: s.total - s.sd,
      ev_bb: s.total - s.luck,
      all_ins: s.allIns,
      luck_bb: s.luck,
      bounties: { USD: one ? 250 : 3745 },
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

export function demoReports(filter?: Filter): Reports {
  const one = !!filter && inTournament(filter);
  const positions: GroupView[] = ["UTG", "UTG+1", "LJ", "HJ", "CO", "BTN", "SB", "BB"].map((key, i) => {
    const hands = Math.round([690, 512, 701, 745, 768, 790, 801, 817][i] / (one ? 28 : 1));
    const netBb = [-12, 4, 18, 35, 61, 120, -88, -150][i];
    return { key, net: netBb * 100, net_bb: netBb, stats: stats(hands, { vpip: [12, 14, 17, 21, 27, 38, 30, 34][i], pfr: [11, 12, 15, 18, 23, 31, 18, 9][i], steal: i >= 4 && i <= 6 ? [32, 44, 30][i - 4] : undefined }) };
  });
  const starting_hands: GroupView[] = [];
  for (let r = 0; r < 13; r++)
    for (let c = 0; c < 13; c++) {
      const hi = RANKS[Math.min(r, c)];
      const lo = RANKS[Math.max(r, c)];
      const key = r === c ? hi + lo : r < c ? `${hi}${lo}s` : `${hi}${lo}o`;
      const strength = 26 - r - c + (r === c ? 8 : 0) + (r < c ? 2 : 0);
      const hands = Math.max(1, Math.round(10 + ((r * 7 + c * 3) % 9) * (r === c ? 1 : r < c ? 1.5 : 4)));
      const netBb = (strength - 9) * 0.9 * (hands / 20);
      const played = Math.max(0, Math.min(100, strength * 5 - 25));
      starting_hands.push({ key, net: Math.round(netBb * 100), net_bb: netBb, stats: stats(hands, { vpip: played, pfr: played * 0.8 }) });
    }
  const sessions = Array.from({ length: 9 }, (_, i) => ({
    start: `2026/11/${String(16 - i).padStart(2, "0")} 19:${String(10 + i * 3).padStart(2, "0")}:00`,
    end: `2026/11/${String(16 - i).padStart(2, "0")} 22:${String(40 - i * 2).padStart(2, "0")}:00`,
    hands: [412, 388, 501, 220, 640, 310, 290, 450, 180][i],
    net: 0,
    net_bb: [38.5, -61.2, 12.4, 88.0, -20.6, 5.1, -44.0, 71.3, -9.8][i],
    bounties: Object.fromEntries([["USD", [375, 0, 250, 845, 125, 0, 0, 500, 0][i]]].filter(([, a]) => a)) as Record<string, number>,
    tables: [3, 4, 2, 4, 3, 2, 1, 4, 2][i],
  }));
  return { positions, starting_hands, sessions: one ? sessions.slice(0, 1) : sessions };
}

export function demoShown(): ShownHand[] {
  return [
    { hand_id: 9, played_at: "2026/11/16 21:04:10", cards: "Kc Jc", board: "Jd 8s Js 4h 2c", position: "BTN", net: 110000, big_blind: 10000 },
    { hand_id: 8, played_at: "2026/11/16 20:51:44", cards: "7h 7d", board: "Ah Kd 7c 2s 9h", position: "BB", net: 64000, big_blind: 8000 },
    { hand_id: 7, played_at: "2026/11/15 22:13:02", cards: "As 4s", board: "Qs 9s 3d Td 2h", position: "CO", net: -36000, big_blind: 6000 },
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
  played_at: "2024/01/15 20:31:10",
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

/** Hands listed for a chosen tournament carry its id (see `demoHands`). */
export function demoHandById(id: number): Hand {
  const own = DEMO_HANDS.find((h) => h.id === id);
  if (own) return own;
  if (id < 1e12) return { ...demoHand, id };
  const tournament = Math.floor(id / 1000);
  return { ...demoHand, id, game: "tournament", tournament_id: tournament, table_name: `${tournament} 10`, currency: null };
}

export function demoHands(query?: HandQuery): HandSummary[] {
  const tournament = query?.tournament_id ?? null;
  const cashAt = (i: number) => tournament === null && i % 3 === 2;
  return Array.from({ length: tournament === null ? 40 : 18 }, (_, i) => ({
    id: tournament === null ? 250000000002 - i : tournament * 1000 + 999 - i,
    game: cashAt(i) ? "cash" : "tournament",
    played_at: `2026/11/16 ${String(22 - Math.floor(i / 6)).padStart(2, "0")}:${String(59 - ((i * 7) % 60)).padStart(2, "0")}:10`,
    table_name: cashAt(i) ? "Aquila II" : `${tournament ?? 4046789300} 10`,
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
    return { ...seat, stack: seat.stack_bb * 10, bounty: null, name, note: demoNote(name) };
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
  const kinds = [
    { game: "NL Hold'em", buy: [500, 0, 50], cur: "USD", name: "$5.50 Mini Early Battle [Progressive KO], $1.5K Gtd" },
    { game: "NL Hold'em", buy: [1000, 1000, 200], cur: "USD", name: "Bounty Builder $22, $10K Gtd" },
    { game: "NL Hold'em", buy: [300, 0, 30], cur: "USD", name: "Hot $3.30, $1.5K Gtd" },
    { game: "PL Omaha", buy: [460, 0, 40], cur: "EUR", name: "PLO Turbo €5" },
    { game: "NL Hold'em", buy: [200, 200, 40], cur: "USD", name: "Sunday Million - Day 1" },
    { game: "NL Hold'em", buy: [180, 0, 20], cur: "EUR", name: "Sit & Go 9-Max Hyper" },
  ];
  let s = 7;
  const rnd = () => (s = (s * 16807) % 2147483647) / 2147483647;
  const rows: TournamentRow[] = Array.from({ length: 34 }, (_, i) => {
    const k = kinds[Math.floor(rnd() * kinds.length)];
    const freeroll = i === 9;
    const [prizePart, bountyPart, fee] = freeroll ? [0, 0, 0] : k.buy;
    const entrants = Math.round(18 + rnd() * 700);
    const known = i >= 2 && i !== 5;
    const place = known ? (i === 12 ? 3 : Math.max(1, Math.round(rnd() * entrants))) : null;
    const reentry = !freeroll && rnd() > 0.85 ? prizePart + bountyPart + fee : 0;
    const paid = Math.ceil(entrants * 0.15);
    const pool = (prizePart || 20) * entrants;
    // The newest one is still running; the others without a place left without a prize.
    const running = i === 0;
    const prize = running ? null : place === null ? 0 : place <= paid ? Math.round(Math.max(pool * 0.22 * place ** -1.15, (prizePart || 20) * 1.6)) : 0;
    const bounties = bountyPart ? Math.round(rnd() * 3) * Math.round(bountyPart / 2) : 0;
    const cost = prizePart + bountyPart + fee + reentry;
    const day = 16 - Math.floor(i / 2.4);
    return {
      id: 4046789300 - i * 731,
      game: k.game,
      currency: k.cur,
      buy_in: { prize: prizePart, bounty: bountyPart, fee, currency: freeroll ? null : k.cur },
      cost,
      entrants: known ? entrants : null,
      place,
      prize,
      bounties,
      profit: prize === null ? null : prize + bounties - cost,
      started_at: `2026/11/${String(day).padStart(2, "0")} ${String(18 + (i % 5)).padStart(2, "0")}:${String((i * 13) % 60).padStart(2, "0")}:00`,
      hands: Math.round(40 + rnd() * 260),
      chips: Math.round((rnd() - 0.55) * 800000),
      has_summary: known && i % 7 !== 3,
      // Names come from table windows; tournaments played before Ringside ran have none.
      name: i % 4 === 3 ? null : k.name,
    };
  });
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
      profit: r.profit === null ? null : r.profit * 1000,
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
