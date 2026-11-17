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

/** Avatar centre (ring), plus the classic box / graphic plate once moved on their own. */
export interface SeatPos {
  x: number;
  y: number;
  card?: Point | null;
  plate?: Point | null;
  /** Where this seat's cards show in the recap, once moved. */
  recap?: Point | null;
}

export interface Threshold {
  low: number;
  high: number;
}

export type OverlayMode = "classic" | "graphic";
export type ClassicPreset = "beginner" | "standard" | "tournament" | "detailed" | "custom";
export type Palette = "traffic" | "icefire";

export interface HudConfig {
  mode: OverlayMode;
  palette: Palette;
  classic: { preset: ClassicPreset; custom: StatId[] };
  graphic: { segments: StatId[]; plate_stats: StatId[]; cover_plate: boolean; plate_width: number };
  thresholds: Record<string, Threshold>;
  scale: number;
  opacity: number;
  split_by_game: boolean;
  show_stack: boolean;
  show_bounty: boolean;
  recap_seconds: number;
  /** Own numbers from this tournament (cash: this table today) instead of all hands. */
  hero_session_only: boolean;
  /** Top-left of the recap's board panel as a fraction of the table window. */
  recap_board: Point | null;
}

export interface Preset {
  name: string;
  preferred_seating: boolean;
  hud: HudConfig;
  layouts: Record<string, SeatPos[]>;
}

export interface Settings {
  hand_history_dirs: string[];
  preferred_seating: boolean;
  hud: HudConfig;
  /** One layout per table size ("8"), shared by every table window and the editor. */
  layouts: Record<string, SeatPos[]>;
  presets: Preset[];
}

export interface SeatView {
  seat: number;
  visual: number;
  name: string;
  stack: number;
  stack_bb: number;
  bounty: number | null;
  is_hero: boolean;
  stats: StatValue[];
}

export interface Recap {
  hand_id: number;
  board: string[];
  pot: number;
  /** [visual seat, cards] */
  shown: [number, string[]][];
  winners: [number, number][];
  bounties: [number, number][];
}

export interface TableView {
  table_key: string;
  title: string;
  game: GameKind;
  max_seats: number;
  big_blind: number;
  currency: string | null;
  bounty_currency: string | null;
  hand_id: number;
  seats: SeatView[];
  layout_key: string;
  layout: SeatPos[];
  hud: HudConfig;
  recap: Recap;
}

export interface ClientWindow {
  title: string;
  class: string;
  table_key: string | null;
  has_hands: boolean;
}

export interface RebuildProgress {
  done: number;
  total: number;
}

export interface ImportStatus {
  dirs: string[];
  files_read: number;
  hands_imported: number;
  total_hands: number;
  malformed: number;
  last_error: string | null;
  upgrade: RebuildProgress | null;
}

export interface Filter {
  game: GameKind | null;
  /** "YYYY/MM/DD" */
  since: string | null;
}

export interface ResultPoint {
  n: number;
  played_at: string;
  net_bb: number;
  showdown_bb: number;
  non_showdown_bb: number;
}

export interface Results {
  hands: number;
  net: number;
  net_bb: number;
  showdown_bb: number;
  non_showdown_bb: number;
  /** Knockout money won per currency, e.g. { USD: 3745 }. */
  bounties: Record<string, number>;
  points: ResultPoint[];
}

export interface PlayerView {
  name: string;
  net: number;
  stats: StatValue[];
}

export interface Dashboard {
  hand_count: number;
  heroes: string[];
  results: Results;
  hero: PlayerView | null;
}

export interface GroupView {
  /** Position ("BTN") or starting hand ("AKs"). */
  key: string;
  net: number;
  net_bb: number;
  stats: StatValue[];
}

export interface Session {
  start: string;
  end: string;
  hands: number;
  net: number;
  net_bb: number;
  bounties: Record<string, number>;
  tables: number;
}

export interface Reports {
  positions: GroupView[];
  starting_hands: GroupView[];
  sessions: Session[];
}

export interface ShownHand {
  hand_id: number;
  played_at: string;
  cards: string;
  board: string;
  position: string | null;
  net: number;
  big_blind: number;
}

export interface HandQuery extends Filter {
  player: string | null;
  showdown_only: boolean;
  result: "won" | "lost" | null;
  min_pot_bb: number | null;
  limit: number;
  offset: number;
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
  hero_position: string | null;
  hero_net: number | null;
  hero_showdown: boolean;
  pot: number;
  board: string;
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
  seats: { seat: number; name: string; stack: number; bounty: number | null; sitting_out: boolean }[];
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
  bounties_won: [string, number][];
  bounty_currency: string | null;
}
