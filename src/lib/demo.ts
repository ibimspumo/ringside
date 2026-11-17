// Sample data so the UI can be designed and previewed in a plain browser without Tauri.
import type {
  Dashboard,
  GroupView,
  Hand,
  HandSummary,
  PlayerView,
  Reports,
  SeatPos,
  Settings,
  ShownHand,
  StatId,
  StatValue,
  TableView,
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
  hud: {
    mode: "graphic",
    palette: "traffic",
    classic: { preset: "beginner", custom: ["vpip", "pfr", "three_bet", "af", "hands"] },
    graphic: { segments: ["vpip", "pfr", "af", "three_bet"], plate_stats: ["three_bet", "fold_to_three_bet", "steal"], cover_plate: true },
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
  },
  layouts: {},
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
    layout_key: `${maxSeats}@1.45`,
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

export function demoPlayers(): PlayerView[] {
  return PLAYERS.map(([name, hands, v], i) => ({ name, net: [0, 920, -1830, 4210, -310, -2650, 330, -1200][i], stats: stats(hands, v) }));
}

function series(n: number, drift: number, seed: number) {
  let s = seed;
  const rnd = () => (s = (s * 16807) % 2147483647) / 2147483647 - 0.5;
  let total = 0;
  let sd = 0;
  let nsd = 0;
  const points = [];
  const every = Math.ceil(n / 1500);
  for (let i = 1; i <= n; i++) {
    const net = rnd() * 9 + drift;
    total += net;
    if (rnd() > 0.3) sd += net * 1.6;
    else nsd += net - net * 1.6 * 0.7;
    if (i % every === 0 || i === n)
      points.push({ n: i, played_at: `2026/11/${String(1 + Math.floor((i / n) * 16)).padStart(2, "0")} 20:00:00`, net_bb: total, showdown_bb: sd, non_showdown_bb: total - sd });
  }
  void nsd;
  return { total, sd, points };
}

export function demoDashboard(): Dashboard {
  const s = series(6124, 0.06, 11);
  return {
    hand_count: 6431,
    heroes: ["Hero"],
    results: {
      hands: 6124,
      net: Math.round(s.total * 100),
      net_bb: s.total,
      showdown_bb: s.sd,
      non_showdown_bb: s.total - s.sd,
      bounties: { USD: 3745 },
      points: s.points,
    },
    hero: demoPlayers()[0],
  };
}

const RANKS = "AKQJT98765432";

export function demoReports(): Reports {
  const positions: GroupView[] = ["UTG", "UTG+1", "LJ", "HJ", "CO", "BTN", "SB", "BB"].map((key, i) => {
    const hands = [690, 512, 701, 745, 768, 790, 801, 817][i];
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
  return { positions, starting_hands, sessions };
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

export function demoHands(): HandSummary[] {
  return Array.from({ length: 40 }, (_, i) => ({
    id: 250000000002 - i,
    game: i % 3 === 2 ? "cash" : "tournament",
    played_at: `2026/11/16 ${String(22 - Math.floor(i / 6)).padStart(2, "0")}:${String(59 - ((i * 7) % 60)).padStart(2, "0")}:10`,
    table_name: i % 3 === 2 ? "Aquila II" : "4046789300 10",
    small_blind: 5,
    big_blind: 10,
    currency: i % 3 === 2 ? "USD" : null,
    hero_cards: ["9s 9d", "Ah Kc", "7c 2d", "Qs Qh", "Jd Td", "As 5s", "8h 6h"][i % 7],
    hero_position: ["BTN", "CO", "BB", "SB", "UTG", "HJ", "LJ"][i % 7],
    hero_net: [-1000, 107, -5, 0, 245, -30, 60][i % 7],
    hero_showdown: i % 4 === 0,
    pot: [2030, 225, 15, 30, 510, 60, 120][i % 7],
    board: ["9c 4h 2s Kd 3c", "2c 7d Jh", "", "", "Td 9d 2s 4c 8h", "Kh 3s 3d", "Qc Jc 5d 2h"][i % 7],
  }));
}
