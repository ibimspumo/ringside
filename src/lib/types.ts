import type { SkillArea } from "./skill/types";
import type { DecisionBrief, DecisionFilter } from "./analysis/types";
import type { RangeFilter } from "./ranges/types";
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
  /** `plate_pages`: pages of three numbers; a click on a plate shows the next page. */
  graphic: { segments: StatId[]; plate_pages: StatId[][]; cover_plate: boolean; plate_width: number };
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
  /** Changed only through `api.analysisSetAuto`. */
  analysis: { auto: boolean };
  /** Changed only through `libraryApi.setMode`; missing in the browser demo. */
  library?: { background: "off" | "idle" | "always" };
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
  /** null while PokerStars runs. */
  read: PlayerRead | null;
  /** The user's own note and colour label from the PokerStars client, if any. */
  note?: PlayerNote | null;
}

export interface PlayerNote {
  label: { name: string; color: string } | null;
  text: string;
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
  tournament_id: number | null;
  /** Last hand included, "YYYY/MM/DD HH:MM:SS": the end of a session. */
  until?: string | null;
}

export interface ResultPoint {
  n: number;
  played_at: string;
  net_bb: number;
  showdown_bb: number;
  non_showdown_bb: number;
  /** Result with all-ins that had every hand shown counted at their expectation. */
  ev_bb: number;
}

export interface Results {
  hands: number;
  net: number;
  net_bb: number;
  showdown_bb: number;
  non_showdown_bb: number;
  ev_bb: number;
  /** All-ins with every hand shown, and what they paid above their EV. */
  all_ins: number;
  luck_bb: number;
  /** Knockout money won per currency, e.g. { USD: 3745 }. */
  bounties: Record<string, number>;
  points: ResultPoint[];
}

export interface PlayerView {
  name: string;
  net: number;
  stats: StatValue[];
  /** null while PokerStars runs. */
  read: PlayerRead | null;
}

export type PlayerType = "nit" | "tag" | "lag" | "station";

/** A player type with its words, from crates/ringside-learn/src/hud.rs. */
export interface TypeRead {
  id: PlayerType;
  name: string;
  long: string;
  sketch: string;
  counter: string;
  why: string;
}

/** What VPIP and PFR say about an opponent; `type` is null between the types. */
export interface PlayerRead {
  type: TypeRead | null;
  hands: number;
  sure: "sure" | "maybe" | "unsure";
  /** "nach 412 Händen klares Muster". */
  note: string;
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
  /** Hands of one leak-finder spot. */
  spot?: SpotFilter | null;
  /** Hands with a matching analysed decision. */
  decisions?: DecisionFilter | null;
  /** Costliest decision first instead of newest first. */
  sort_by_loss?: boolean;
  /** Hands with a matching preflop decision of yours. */
  range?: RangeFilter | null;
  /** Hands with a decision of this kind of spot of the playing profile. */
  profile?: string | null;
  /** Hands with a rated decision in this area of the Spielwertung. */
  area?: SkillArea | null;
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
  /** The worst analysed decision, among those the decision filter matches. */
  worst?: DecisionBrief | null;
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

export interface BuyIn {
  prize: number;
  bounty: number;
  fee: number;
  /** "USD", "EUR", …, "FPP"/"SC" for points, "PLAY" for play chips, null for freerolls. */
  currency: string | null;
}

export interface TournamentRow {
  id: number;
  game: string;
  currency: string | null;
  buy_in: BuyIn | null;
  /** Buy-in plus re-entries; null when the buy-in is unknown. */
  cost: number | null;
  entrants: number | null;
  /** null while the result is unknown. */
  place: number | null;
  prize: number | null;
  bounties: number;
  profit: number | null;
  started_at: string;
  hands: number;
  /** Hero's chip change over the imported hands. */
  chips: number;
  has_summary: boolean;
  /** From the table window title, once a table of this tournament was open. */
  name: string | null;
}

/** Results in one currency; money only from tournaments with a known result. */
export interface TournamentTotals {
  currency: string;
  tournaments: number;
  finished: number;
  cashes: number;
  cost: number;
  prizes: number;
  bounties: number;
  profit: number;
  roi: number | null;
  itm: number | null;
}

export interface Tournaments {
  rows: TournamentRow[];
  totals: TournamentTotals[];
}

/** A notes file of the PokerStars client. */
export interface NotesSource {
  path: string;
  players: number;
  error: string | null;
}

export interface LiveTable {
  title: string;
  table_key: string;
  /** null until a hand of this table has been imported. */
  table: TableView | null;
}

// Learning area (crates/ringside-learn). Cards are strings like "Ah", "Td"; amounts in big blinds.

export type LearnKind =
  | "outs"
  | "pot_odds"
  | "probability"
  | "combos"
  | "position"
  | "showdown"
  | "equity_hand"
  | "equity_range"
  | "push_fold_sb"
  | "push_fold_bb"
  /** Shove or fold from any seat once everyone before folded, 2–9 players. */
  | "push_fold_open"
  /** Call or fold against a shove, also after a caller, 2–9 players. */
  | "push_fold_call"
  /** Only from own hands (`learnOwnSpots`), never generated. */
  | "all_in"
  /** One attempt per hand of the practice with whole hands, never generated. */
  | "full_hands"
  | "bounty_odds"
  | "icm_call"
  | "open_fold"
  | "open_fold_short"
  | "river_bet"
  | "stack_depth"
  | "hud_read"
  /** A decision of the playing profile decided again, real or a variation; never generated. */
  | "own_decision"
  /** One of the bundled learning spots („Wichtige Spots“); never generated. */
  | "key_spot";

/**
 * exakt = exact maths; formel = formula / rule of thumb; modell = exact within the model in `model_note`;
 * gleichgewicht = a solved equilibrium of the game in `model_note`.
 */
export type Grading = "exakt" | "formel" | "modell" | "gleichgewicht";

/** Generated when `hand_id` is null: (kind, level, seed) rebuilds it. Send it back unchanged. */
export interface Exercise {
  kind: LearnKind;
  level: number;
  /** Below 2^53; pass it back as received. */
  seed: number;
  hand_id: number | null;
  question: string;
  prompt: ExercisePrompt;
  answer: AnswerSpec;
  grading: Grading;
  model_note: string | null;
}

export type ExercisePrompt =
  | { type: "outs"; hero: string[]; villain: string[] | null; board: string[] }
  | { type: "pot_odds"; pot: number; to_call: number; equity_pct: number | null }
  | { type: "probability"; outs: number; unseen: number; cards_to_come: number; method: "rule" | "exact" }
  | { type: "combos"; class: string | null; range: string | null; dead: string[] }
  | {
      type: "position";
      table_size: number;
      occupied: number[];
      button: number;
      dead_button: boolean;
      small_blind: number | null;
      big_blind: number | null;
      ask: PositionAsk;
    }
  | { type: "showdown"; board: string[]; players: string[][] }
  | { type: "equity_hand"; hero: string[]; villain: string[]; board: string[] }
  | { type: "equity_range"; hero: string[]; range_name: string; range: string; range_pct: number; board: string[] }
  | {
      type: "push_fold";
      hero_position: "sb" | "bb";
      hand: string[];
      class: string;
      stack_bb: number;
      small_blind_bb: number;
      ante_bb: number;
      dead_bb: number;
      players: number;
    }
  | {
      /** Seats in preflop action order: first after the big blind first, the big blind last. */
      type: "push_fold_table";
      hand: string[];
      class: string;
      /** "UTG", …, "BTN", "SB", "BB" per seat. */
      positions: string[];
      /** Before posting. */
      stacks_bb: number[];
      hero: number;
      /** null: everyone before the hero folded. */
      shover: number | null;
      callers: number[];
      effective_bb: number;
      small_blind_bb: number;
      ante_bb: number;
      bb_ante_bb: number;
      dead_bb: number;
    }
  | { type: "all_in"; hero: string[]; opponents: string[][]; board: string[]; pots: PotView[] }
  | {
      /** Hero (not in the blinds) faces an all-in and may win the shover's bounty. */
      type: "bounty";
      dead_bb: number;
      shove_bb: number;
      hero_stack_bb: number;
      /** Dollars, levels 2 and 3. */
      villain_bounty: number | null;
      hero_bounty: number | null;
      rate: { start_chips: number; prize_dollars: number; big_blind_chips: number; bb_per_dollar: number } | null;
      /** Level 1 names the bounty in big blinds. */
      bounty_bb: number | null;
      equity_pct: number | null;
    }
  | {
      /** `ask`: seat whose prize value is asked (level 1); otherwise `hero` faces `shover`. Action order, BB last. */
      type: "icm";
      positions: string[];
      stacks_bb: number[];
      /** Dollars per place, first place first. */
      payouts: number[];
      ask: number | null;
      hero: number | null;
      shover: number | null;
      hand: string[];
      class: string | null;
      ante_bb: number;
    }
  | {
      type: "open_fold";
      positions: string[];
      hero: number;
      hand: string[];
      class: string;
      stack_bb: number;
      ante_bb: number;
      bb_ante_bb: number;
      open_to_bb: number;
    }
  | {
      type: "river_bet";
      scenario: string;
      board: string[];
      hand: string[];
      class: string;
      oop: string;
      ip: string;
      hero_ip: boolean;
      pot_type: "srp" | "3bp";
      pot_bb: number;
      stack_bb: number;
      bet_bb: number;
    }
  | {
      /** Chips for "bb" and "m", big blinds for "ante_odds". */
      type: "stack_depth";
      ask: "bb" | "m" | "ante_odds";
      stack: number;
      small_blind: number;
      big_blind: number;
      ante: number;
      bb_ante: boolean;
      players: number;
      shove_bb: number | null;
      shover: string | null;
    }
  | { type: "hud_read"; vpip: number; pfr: number; three_bet: number; hands: number; ask: "type" | "counter" | "trust" };

export type PositionAsk = { type: "seat_of"; label: string } | { type: "label_of"; seat: number };

/** players: 0 = hero, 1.. = opponents in order. */
export interface PotView {
  amount_bb: number;
  players: number[];
}

export type AnswerSpec =
  | { type: "number"; unit: string; min: number; max: number; step: number }
  | { type: "choice"; options: ChoiceOption[] }
  | { type: "cards"; from: string[] }
  | { type: "seat"; seats: number[] };

export interface ChoiceOption {
  id: string;
  label: string;
}

/** Percentages as 0–100. */
export type LearnAnswer = { number: number } | { choice: string } | { cards: string[] } | { seat: number };

export interface Feedback {
  correct: boolean;
  /** 0–1. */
  score: number;
  truth: LearnAnswer;
  truth_text: string;
  facts: Fact[];
  highlight: Highlight[];
  ev_loss_bb: number | null;
  grading: Grading;
  model_note: string | null;
  /** A strategy over the 13×13 classes, shown with the answer. */
  grid: LearnGrid | null;
}

/** How often each class (HandClass index) takes `action`, 0–1; null when the class can't be here. */
export interface LearnGrid {
  title: string;
  action: string;
  cells: (number | null)[];
  hero: number;
  /** Share of all combos here taking the action, in percent. */
  pct: number;
}

export interface Fact {
  label: string;
  value: string;
  formula: string | null;
}

export interface Highlight {
  card: string;
  tag: "out" | "missed" | "half_out" | "wrong" | "dirty";
}

export interface OwnSpot {
  /** "<kind>:<hand_id>", for `learnCheckOwn`. */
  id: string;
  kind: LearnKind;
  hand_id: number;
  played_at: string;
  /** Index into `Hand.actions` of the hero's decision. */
  stop_before_action: number;
  exercise: Exercise;
  /** What it cost comes with the feedback as the fact "Damals gespielt". */
  played: LearnAnswer | null;
  /** Outcome of an all-in; show only after answering. */
  all_in: AllInResult | null;
}

export interface AllInResult {
  equity_pct: number;
  pot_bb: number;
  ev_bb: number;
  net_bb: number;
  luck_bb: number;
  rake_bb: number;
}

export interface LearnProgress {
  /** One per kind, fixed order. */
  skills: SkillProgress[];
  /** Answers since Monday 00:00 UTC. */
  xp_week: number;
  xp_total: number;
}

export interface SkillProgress {
  id: LearnKind;
  name_de: string;
  level: 0 | 1 | 2 | 3 | 4;
  level_name: "neu" | "kennengelernt" | "mit Hilfe" | "selbstständig" | "bestätigt";
  /** Highest exercise level tried (0 = none). */
  max_level: number;
  attempts: number;
  mean_score: number | null;
  /** Unix seconds. */
  last_at: number | null;
}

export type PushFoldAnte = "none" | "classic" | "big_blind";

export interface PushFoldGrid {
  min_players: number;
  max_players: number;
  antes: { ante: PushFoldAnte; ante_bb: number; bb_ante_bb: number; stacks: number[] }[];
}

export interface PushFoldChart {
  players: number;
  stack_bb: number;
  ante: PushFoldAnte;
  /** False: solved live because no precomputed chart matches. */
  precomputed: boolean;
  converged: boolean;
  exploitability_bb: number;
  multiway_probability: number;
  nodes: PushFoldNode[];
}

export interface PushFoldNode {
  /** Action order: first player after the big blind … small blind, big blind. */
  seat: number;
  position: string;
  /** null: everyone before folded and the seat may shove. */
  shover: number | null;
  /** Seats that called the shove before this one. */
  callers: number[];
  reach: number;
  act_pct: number;
  /** Shove/call frequency per hand class (row * 13 + col, ace first), 0–255. */
  freq: number[];
  /** Shove/call EV per hand class and fold EV, in hundredths of a BB. */
  ev_act: number[];
  ev_fold: number;
}

/** Hero hands with the chance for `stat` at `positions` (empty: any) and stack `band`; `made`
 *  narrows to hands where the hero did it or not (AF: aggressive action or postflop call). */
export interface SpotFilter {
  stat: StatId;
  made: boolean | null;
  positions: string[];
  band: number | null;
  game: GameKind | null;
}

export interface LeakSource {
  id: string;
  title: string;
  publisher: string;
  url: string | null;
  scope: string;
}

export interface LeakCost {
  hands: number;
  spot_bb100: number;
  base_bb100: number;
  margin_bb100: number;
  total_bb: number;
}

/** One stat in one spot against its guide range. */
export interface LeakCheck {
  stat: StatId;
  game: GameKind;
  /** Empty for all positions. */
  positions: string[];
  band: number;
  made: number;
  opp: number;
  /** Percent, or the plain ratio for AF; same unit for the interval, range and deviation. */
  value: number;
  ci_low: number;
  ci_high: number;
  low: number;
  high: number;
  direction: "too_high" | "too_low" | null;
  deviation: number;
  /** One-sided probability that the true value lies outside the range on this side. */
  confidence: number;
  uncertain: boolean;
  score: number;
  cost: LeakCost | null;
  cites: { source: string; says: string }[];
  note: string | null;
}

export interface LeakReport {
  leaks: LeakCheck[];
  fine: LeakCheck[];
  too_small: number;
  bands: string[];
  sources: LeakSource[];
}
