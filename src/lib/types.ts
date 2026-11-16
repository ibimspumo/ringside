export type StatId =
  | "hands"
  | "vpip"
  | "pfr"
  | "three_bet"
  | "fold_to_three_bet"
  | "steal"
  | "fold_to_steal"
  | "cbet"
  | "fold_to_cbet"
  | "wtsd"
  | "wsd"
  | "af"
  | "afq";

export type GameKind = "cash" | "tournament";

export interface StatValue {
  id: StatId;
  value: number | null;
  made: number;
  opp: number;
}

export interface Point {
  x: number;
  y: number;
}

/** Ring centre, plus the card's top-left corner once the card was moved on its own. */
export interface SeatPos {
  x: number;
  y: number;
  card?: Point | null;
}

export interface Threshold {
  low: number;
  high: number;
}

export interface HudConfig {
  ring_outer: StatId;
  ring_inner: StatId;
  rows: StatId[];
  thresholds: Record<string, Threshold>;
  scale: number;
  opacity: number;
  split_by_game: boolean;
  show_stack: boolean;
}

export interface Settings {
  hand_history_dirs: string[];
  preferred_seating: boolean;
  hud: HudConfig;
  layouts: Record<string, SeatPos[]>;
}

export interface SeatView {
  seat: number;
  visual: number;
  name: string;
  stack: number;
  stack_bb: number;
  is_hero: boolean;
  stats: StatValue[];
}

export interface TableView {
  table_key: string;
  title: string;
  game: GameKind;
  max_seats: number;
  big_blind: number;
  currency: string | null;
  hand_id: number;
  seats: SeatView[];
  layout: SeatPos[];
  hud: HudConfig;
}

export interface ClientWindow {
  title: string;
  class: string;
  table_key: string | null;
  has_hands: boolean;
}

export interface ImportStatus {
  dirs: string[];
  files_read: number;
  hands_imported: number;
  total_hands: number;
  malformed: number;
  last_error: string | null;
}

export interface ResultPoint {
  hand_id: number;
  net: number;
  big_blind: number;
}

export interface Dashboard {
  hand_count: number;
  heroes: string[];
  results: ResultPoint[];
}

export interface PlayerView {
  name: string;
  net: number;
  stats: StatValue[];
}

export interface HandSummary {
  id: number;
  game: GameKind;
  played_at: string;
  table_name: string;
  small_blind: number;
  big_blind: number;
  currency: string | null;
  hero_cards: string;
  hero_net: number | null;
}

export type Street = "preflop" | "flop" | "turn" | "river";
export type ActionKind =
  | "post_small_blind"
  | "post_big_blind"
  | "post_ante"
  | "post_both"
  | "fold"
  | "check"
  | "call"
  | "bet"
  | "raise";

export interface Action {
  street: Street;
  player: string;
  kind: ActionKind;
  amount: number;
  to: number | null;
  all_in: boolean;
}

export interface Hand {
  id: number;
  game: GameKind;
  tournament_id: number | null;
  variant: string;
  table_name: string;
  max_seats: number;
  button_seat: number;
  small_blind: number;
  big_blind: number;
  currency: string | null;
  played_at: string;
  seats: { seat: number; name: string; stack: number; sitting_out: boolean }[];
  hero: string | null;
  hero_cards: string[];
  board: string[];
  actions: Action[];
  returned: [string, number][];
  shown: [string, string[]][];
  collected: [string, number][];
  showdown: boolean;
  total_pot: number;
  rake: number;
}
