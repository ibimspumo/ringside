// Sample data so the UI can be developed and previewed in a plain browser without Tauri.
import type { Dashboard, Hand, HandSummary, PlayerView, Settings, StatId, StatValue, TableView } from "./types";

const ovalLayout = (n: number) =>
  Array.from({ length: n }, (_, i) => {
    const a = Math.PI / 2 + (i * 2 * Math.PI) / n;
    return { x: 0.5 + 0.311 * Math.cos(a), y: 0.426 + 0.297 * Math.sin(a) };
  });

export const demoSettings: Settings = {
  hand_history_dirs: [],
  preferred_seating: true,
  hud: {
    ring_outer: "vpip",
    ring_inner: "pfr",
    rows: ["three_bet", "af", "cbet"],
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
    opacity: 0.92,
    split_by_game: true,
    show_stack: true,
  },
  layouts: {},
};

function stats(hands: number, v: Partial<Record<StatId, number>>): StatValue[] {
  const ids: StatId[] = ["hands", "vpip", "pfr", "three_bet", "fold_to_three_bet", "steal", "fold_to_steal", "cbet", "fold_to_cbet", "wtsd", "wsd", "af", "afq"];
  return ids.map((id) => {
    if (id === "hands") return { id, value: hands, made: hands, opp: hands };
    const value = v[id] ?? null;
    const opp = Math.round(hands * (id === "vpip" || id === "pfr" ? 1 : 0.2));
    return { id, value, made: value === null ? 0 : Math.round(((id === "af" ? 0.5 : value / 100) * opp)), opp };
  });
}

const players: [string, number, Partial<Record<StatId, number>>][] = [
  ["Hero", 1840, { vpip: 23, pfr: 19, three_bet: 8, af: 2.6, cbet: 62, steal: 41, fold_to_three_bet: 55, wtsd: 27, wsd: 54, afq: 48, fold_to_cbet: 41, fold_to_steal: 66 }],
  ["shevalex09", 412, { vpip: 41, pfr: 9, three_bet: 2, af: 0.9, cbet: 38, steal: 18, fold_to_three_bet: 34, wtsd: 37, wsd: 44, afq: 24, fold_to_cbet: 29, fold_to_steal: 48 }],
  ["Goban899", 96, { vpip: 18, pfr: 15, three_bet: 7, af: 3.1, cbet: 71, steal: 44, fold_to_three_bet: 62, wtsd: 24, wsd: 55, afq: 51, fold_to_cbet: 45, fold_to_steal: 71 }],
  ["omar miete", 27, { vpip: 33, pfr: 26, three_bet: 12, af: 4.2, cbet: 80, steal: 52, fold_to_three_bet: 40, wtsd: 22, wsd: 50, afq: 58, fold_to_cbet: 30, fold_to_steal: 55 }],
  ["7juguba7", 8, { vpip: 50, pfr: 13, af: 1.0 }],
  ["Ojaniel", 233, { vpip: 15, pfr: 12, three_bet: 3, af: 2.0, cbet: 55, steal: 30, fold_to_three_bet: 72, wtsd: 21, wsd: 52, afq: 40, fold_to_cbet: 62, fold_to_steal: 80 }],
];

export function demoTable(maxSeats = 6): TableView {
  return {
    table_key: "c:Demo",
    title: "Aquila II – $0.05/$0.10",
    game: "cash",
    max_seats: maxSeats,
    big_blind: 10,
    currency: "USD",
    hand_id: 2,
    seats: players.slice(0, maxSeats).map(([name, hands, v], i) => ({
      seat: i + 1,
      visual: i,
      name,
      stack: [1000, 3312, 2016, 4077, 2281, 3918][i],
      stack_bb: [100, 33, 20, 41, 23, 39][i],
      is_hero: i === 0,
      stats: stats(hands, v),
    })),
    layout: ovalLayout(maxSeats),
    hud: demoSettings.hud,
  };
}

export function demoPlayers(): PlayerView[] {
  return players.map(([name, hands, v], i) => ({ name, net: [4210, -1830, 920, -310, -2650, 330][i], stats: stats(hands, v) }));
}

export function demoDashboard(): Dashboard {
  let seed = 7;
  const rnd = () => ((seed = (seed * 16807) % 2147483647) / 2147483647) - 0.5;
  return {
    hand_count: 1840,
    heroes: ["Hero"],
    results: Array.from({ length: 1840 }, (_, i) => ({ hand_id: i, net: Math.round(rnd() * 160 + 3), big_blind: 10 })),
  };
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
    { seat: 1, name: "Alice", stack: 1000, sitting_out: false },
    { seat: 2, name: "Bob", stack: 1140, sitting_out: false },
    { seat: 3, name: "Carol", stack: 800, sitting_out: false },
    { seat: 4, name: "Dave", stack: 995, sitting_out: false },
    { seat: 5, name: "Hero", stack: 1107, sitting_out: false },
    { seat: 6, name: "Eve", stack: 1000, sitting_out: false },
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
};

export function demoHands(): HandSummary[] {
  return Array.from({ length: 24 }, (_, i) => ({
    id: 250000000002 - i,
    game: i % 5 === 4 ? "tournament" : "cash",
    played_at: `2024/01/15 ${String(20 - Math.floor(i / 6)).padStart(2, "0")}:${String(59 - ((i * 7) % 60)).padStart(2, "0")}:10`,
    table_name: i % 5 === 4 ? "3500000001 12" : "Aquila II",
    small_blind: 5,
    big_blind: 10,
    currency: i % 5 === 4 ? null : "USD",
    hero_cards: ["9s 9d", "Ah Kc", "7c 2d", "Qs Qh", "Jd Td", "As 5s"][i % 6],
    hero_net: [-1000, 107, -5, 0, 245, -30][i % 6],
  }));
}
