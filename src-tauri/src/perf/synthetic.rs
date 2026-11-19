//! Whole tournaments as PokerStars writes them, for measuring Ringside at scale: blind levels,
//! antes, short-stack shoves, side pots, knockouts, players busting and new ones taking their
//! seats, several tournaments of an evening side by side. Deterministic for a seed; every
//! name is made up.

use std::fmt::Write;

use ringside_engine::Card;
use ringside_engine::eval::{HandCategory, evaluate_cards};

pub const HERO: &str = "Hero";
/// The id of the first tournament of every history.
pub const FIRST_TOURNAMENT: u64 = 4_100_000_000;

/// One tournament's hand history file.
pub struct Tournament {
    pub file_name: String,
    /// Each hand's text, in play order; the file is these joined by blank lines.
    pub hands: Vec<String>,
}

impl Tournament {
    pub fn text(&self) -> String {
        self.hands.iter().map(|h| format!("{h}\n\n\n")).collect()
    }
}

/// Tournaments of about `total` hands in all, against opponents from a pool of `pool` names.
pub fn history(total: usize, pool: usize, seed: u64) -> Vec<Tournament> {
    let mut rng = Rng(seed ^ 0x9e37_79b9_7f4a_7c15);
    let names = Names { pool };
    let mut out = Vec::new();
    let mut made = 0;
    // Day 0 is 2025/01/01; one session an evening, now and then a day off.
    let mut day = 0;
    while made < total {
        let parallel = 1 + rng.below(4) as usize;
        let start = day * 86_400 + 18 * 3600 + rng.below(7200) as i64;
        for slot in 0..parallel {
            if made >= total {
                break;
            }
            let id = FIRST_TOURNAMENT + out.len() as u64;
            let begin = start + rng.below(40 * 60) as i64;
            let mut t = play(&mut rng, &names, id, begin, slot as u64);
            t.hands.truncate(total - made);
            made += t.hands.len();
            out.push(t);
        }
        day += 1 + i64::from(rng.chance(0.3));
    }
    out
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }

    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }

    fn chance(&mut self, p: f64) -> bool {
        self.unit() < p
    }

    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.unit()
    }
}

struct Names {
    pool: usize,
}

impl Names {
    const FIRST: [&str; 16] = [
        "Kali", "Bo", "Mira", "Tex", "Jun", "Olli", "Vex", "Raya", "Duk", "Lumo", "Fenn", "Zora", "Pix", "Nando", "Ilka", "Quin",
    ];
    const SECOND: [&str; 12] = ["mar", "star", " Tran", "_x", "berg", "fish", " Ace", "lo", "nut", "river", "_KO", "zz"];

    fn get(&self, i: usize) -> String {
        let i = i % self.pool.max(1);
        let a = Self::FIRST[i % 16];
        let b = Self::SECOND[(i / 16) % 12];
        format!("{a}{b}{}", i / 192)
    }
}

const LEVELS: [(i64, i64); 22] = [
    (25, 50),
    (30, 60),
    (40, 80),
    (50, 100),
    (60, 120),
    (75, 150),
    (100, 200),
    (125, 250),
    (150, 300),
    (200, 400),
    (250, 500),
    (300, 600),
    (400, 800),
    (500, 1000),
    (600, 1200),
    (800, 1600),
    (1000, 2000),
    (1250, 2500),
    (1500, 3000),
    (2000, 4000),
    (2500, 5000),
    (3000, 6000),
];
const ROMAN: [&str; 22] = [
    "I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X", "XI", "XII", "XIII", "XIV", "XV", "XVI", "XVII", "XVIII",
    "XIX", "XX", "XXI", "XXII",
];
const START: i64 = 10_000;

#[derive(Clone)]
struct Player {
    name: String,
    stack: i64,
    /// Above 1 plays more hands, below fewer.
    loose: f64,
    aggressive: f64,
    /// Knockout bounty in cents.
    bounty: i64,
}

struct Buy {
    prize: i64,
    bounty: i64,
    fee: i64,
}

fn money(cents: i64) -> String {
    if cents % 100 == 0 { format!("${}", cents / 100) } else { format!("${}.{:02}", cents / 100, cents % 100) }
}

fn ordinal(n: usize) -> String {
    let suffix = match (n % 10, n % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

/// "2025/03/07 19:04:11" for seconds since 2025/01/01 00:00.
fn stamp(secs: i64) -> String {
    let (mut days, rest) = (secs / 86_400, secs % 86_400);
    let mut year = 2025;
    loop {
        let len = if year % 4 == 0 { 366 } else { 365 };
        if days < len {
            break;
        }
        days -= len;
        year += 1;
    }
    let months = [31, if year % 4 == 0 { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut month = 0;
    while days >= months[month] {
        days -= months[month];
        month += 1;
    }
    format!("{year}/{:02}/{:02} {}:{:02}:{:02}", month + 1, days + 1, rest / 3600, rest / 60 % 60, rest % 60)
}

/// One tournament of the hero from the first hand until the hero busts or wins.
fn play(rng: &mut Rng, names: &Names, id: u64, begin: i64, slot: u64) -> Tournament {
    let buys = [
        Buy { prize: 98, bounty: 0, fee: 12 },
        Buy { prize: 196, bounty: 0, fee: 24 },
        Buy { prize: 240, bounty: 250, fee: 60 },
        Buy { prize: 490, bounty: 0, fee: 60 },
        Buy { prize: 500, bounty: 500, fee: 100 },
    ];
    let buy = &buys[rng.below(buys.len() as u64) as usize];
    let max_seats = [6usize, 8, 9][rng.below(3) as usize];
    let entrants = 18 + rng.below(480) as usize;
    let mut remaining = entrants;
    let table_no = 1 + rng.below((entrants / max_seats) as u64 + 1);
    let mut seats: Vec<Option<Player>> = vec![None; max_seats];
    let hero_seat = rng.below(max_seats as u64) as usize;
    let mut taken: Vec<String> = vec![HERO.into()];
    let mut new_player = |rng: &mut Rng, stack: i64| {
        let mut name = names.get(rng.below(1 << 40) as usize);
        while taken.contains(&name) {
            name = names.get(rng.below(1 << 40) as usize);
        }
        taken.push(name.clone());
        Player { name, stack, loose: rng.range(0.7, 1.4), aggressive: rng.range(0.5, 1.6), bounty: buy.bounty }
    };
    for (i, seat) in seats.iter_mut().enumerate() {
        *seat = Some(if i == hero_seat {
            Player { name: HERO.into(), stack: START, loose: 1.0, aggressive: 1.0, bounty: buy.bounty }
        } else {
            new_player(rng, START)
        });
    }
    let level_hands = 8 + rng.below(6) as usize;
    let mut button = rng.below(max_seats as u64) as usize;
    let mut time = begin;
    let mut hands = Vec::new();
    let buy_text = if buy.bounty > 0 {
        format!("{}+{}+{} USD", money(buy.prize), money(buy.bounty), money(buy.fee))
    } else {
        format!("{}+{} USD", money(buy.prize), money(buy.fee))
    };
    let pool_cents = buy.prize * entrants as i64;
    let paid = (entrants / 7).max(1);
    let prize_of = |place: usize| -> Option<i64> {
        (place <= paid).then(|| pool_cents * 2 / (place as i64 * 3 + 2) / 2)
    };
    for n in 0..600 {
        // Players on other tables bust too; the tables break until this one is the final one.
        let mut seated = seats.iter().flatten().count();
        if remaining > seated {
            let rate = entrants as f64 / 160.0;
            let out = (rate * rng.range(0.0, 2.0)).round() as usize;
            remaining = remaining.saturating_sub(out).max(seated);
        }
        for seat in seats.iter_mut() {
            if seat.is_none() && remaining > seated {
                let avg = START * entrants as i64 / remaining as i64;
                let stack = (avg as f64 * rng.range(0.3, 2.0)) as i64;
                *seat = Some(new_player(rng, stack));
                seated += 1;
            }
        }
        let level = (n / level_hands).min(LEVELS.len() - 1);
        let hand_id = 260_000_000_000 + (time as u64) * 8 + slot;
        let (mut text, summary) = deal(rng, &mut seats, &mut button, &HandSetup {
            hand_id,
            tournament: id,
            buy: &buy_text,
            level,
            when: stamp(time),
            table: format!("{id} {table_no}"),
            max_seats,
            knockout: buy.bounty > 0,
        });
        // Busted players, the smallest stack at the start of the hand placed last.
        let mut busted: Vec<usize> = (0..max_seats).filter(|&s| seats[s].as_ref().is_some_and(|p| p.stack == 0)).collect();
        busted.sort_by_key(|&s| seats[s].as_ref().map(|p| p.bounty));
        let mut hero_out = false;
        for s in busted {
            let p = seats[s].take().unwrap();
            let place = remaining;
            remaining -= 1;
            match prize_of(place) {
                Some(prize) => writeln!(
                    text,
                    "{} finished the tournament in {} place and received {}.",
                    p.name,
                    ordinal(place),
                    money(prize)
                )
                .unwrap(),
                None => writeln!(text, "{} finished the tournament in {} place", p.name, ordinal(place)).unwrap(),
            }
            hero_out |= p.name == HERO;
        }
        let alive = seats.iter().flatten().count();
        let hero_won = !hero_out && remaining <= 1 && alive == 1;
        if hero_won {
            writeln!(text, "{HERO} wins the tournament and receives {} - congratulations!", money(prize_of(1).unwrap_or(0)))
                .unwrap();
        }
        text.push_str(&summary);
        hands.push(text);
        time += 45 + rng.below(70) as i64;
        if hero_out || hero_won {
            break;
        }
    }
    let day = stamp(begin);
    let file_name = format!(
        "HH{} T{id} No Limit Hold'em {}.txt",
        day[..10].replace('/', ""),
        buy_text.replace('.', ",").replace(" USD", "")
    );
    Tournament { file_name, hands }
}

struct HandSetup<'a> {
    hand_id: u64,
    tournament: u64,
    buy: &'a str,
    level: usize,
    when: String,
    table: String,
    max_seats: usize,
    knockout: bool,
}

/// One player's part of the hand.
struct Seat {
    seat: usize,
    name: String,
    cards: [Card; 2],
    behind: i64,
    /// Everything put in, antes included.
    total: i64,
    street: i64,
    folded: bool,
    all_in: bool,
    loose: f64,
    aggressive: f64,
}

impl Seat {
    fn active(&self) -> bool {
        !self.folded && !self.all_in
    }
}

struct Table<'a> {
    seats: Vec<Seat>,
    board: Vec<Card>,
    out: String,
    bb: i64,
    rng: &'a mut Rng,
}

fn class_strength(cards: [Card; 2]) -> f64 {
    let (hi, lo) = if cards[0].rank() >= cards[1].rank() { (cards[0], cards[1]) } else { (cards[1], cards[0]) };
    let (h, l) = (hi.rank() as f64, lo.rank() as f64);
    let mut s = if hi.rank() == lo.rank() { 0.55 + h / 12.0 * 0.45 } else { (h * 2.0 + l) / 38.0 };
    if hi.suit() == lo.suit() {
        s += 0.06;
    }
    if hi.rank() - lo.rank() == 1 {
        s += 0.03;
    }
    s.clamp(0.0, 1.0)
}

fn made_strength(cards: [Card; 2], board: &[Card]) -> f64 {
    let mut all = board.to_vec();
    all.extend(cards);
    let rank = evaluate_cards(&all);
    let board_only = (board.len() >= 5).then(|| evaluate_cards(board));
    if board_only == Some(rank) {
        return 0.2;
    }
    match rank.category() {
        HandCategory::HighCard => 0.15 + class_strength(cards) * 0.2,
        HandCategory::Pair => 0.45 + cards.iter().map(|c| c.rank() as f64).fold(0.0, f64::max) / 60.0,
        HandCategory::TwoPair => 0.7,
        HandCategory::Trips => 0.8,
        _ => 0.92,
    }
}

fn category_text(cards: [Card; 2], board: &[Card]) -> &'static str {
    let mut all = board.to_vec();
    all.extend(cards);
    match evaluate_cards(&all).category() {
        HandCategory::HighCard => "high card",
        HandCategory::Pair => "a pair",
        HandCategory::TwoPair => "two pair",
        HandCategory::Trips => "three of a kind",
        HandCategory::Straight => "a straight",
        HandCategory::Flush => "a flush",
        HandCategory::FullHouse => "a full house",
        HandCategory::Quads => "four of a kind",
        HandCategory::StraightFlush => "a straight flush",
    }
}

fn cards_text(cards: &[Card]) -> String {
    cards.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(" ")
}

impl Table<'_> {
    fn put(&mut self, i: usize, amount: i64, live: bool) -> i64 {
        let s = &mut self.seats[i];
        let a = amount.min(s.behind);
        s.behind -= a;
        s.total += a;
        if live {
            s.street += a;
        }
        if s.behind == 0 {
            s.all_in = true;
        }
        a
    }

    fn all_in_suffix(&self, i: usize) -> &'static str {
        if self.seats[i].behind == 0 { " and is all-in" } else { "" }
    }

    fn strength(&self, i: usize) -> f64 {
        let s = &self.seats[i];
        if self.board.is_empty() { class_strength(s.cards) } else { made_strength(s.cards, &self.board) }
    }

    fn pot(&self) -> i64 {
        self.seats.iter().map(|s| s.total).sum()
    }

    /// One betting round from seat index `first`.
    fn betting(&mut self, first: usize, preflop: bool) {
        let n = self.seats.len();
        let mut bet = self.seats.iter().map(|s| s.street).max().unwrap_or(0);
        let mut min_raise = self.bb;
        let mut acted = vec![false; n];
        let mut raises = 0;
        let mut i = first;
        loop {
            let live: Vec<usize> = (0..n).filter(|&k| !self.seats[k].folded).collect();
            if live.len() <= 1 {
                break;
            }
            let active: Vec<usize> = (0..n).filter(|&k| self.seats[k].active()).collect();
            if active.iter().all(|&k| acted[k] && self.seats[k].street == bet) {
                break;
            }
            if active.len() == 1 && self.seats[active[0]].street >= bet {
                break;
            }
            let k = i;
            i = (i + 1) % n;
            if !self.seats[k].active() || (acted[k] && self.seats[k].street == bet) {
                continue;
            }
            acted[k] = true;
            let to_call = bet - self.seats[k].street;
            let s = self.strength(k);
            let (loose, aggressive) = (self.seats[k].loose, self.seats[k].aggressive);
            let behind = self.seats[k].behind;
            let eff_bb = (behind + self.seats[k].street) as f64 / self.bb as f64;
            let name = self.seats[k].name.clone();
            let pot = self.pot();
            let can_raise = raises < 4 && behind > to_call;
            let shove_threshold = 0.72 - (13.0 - eff_bb).max(0.0) / 45.0 - (loose - 1.0) * 0.15;

            enum Move {
                Fold,
                Check,
                Call,
                RaiseTo(i64),
            }
            let roll = self.rng.unit();
            let mv = if preflop {
                if to_call == 0 {
                    if s > 0.78 / loose && can_raise {
                        if eff_bb < 14.0 { Move::RaiseTo(i64::MAX) } else { Move::RaiseTo(bet * 3) }
                    } else {
                        Move::Check
                    }
                } else if bet <= self.bb {
                    // Unopened or limped.
                    if eff_bb < 13.0 {
                        if s > shove_threshold && can_raise { Move::RaiseTo(i64::MAX) } else { Move::Fold }
                    } else if s > 0.70 / loose && can_raise {
                        Move::RaiseTo(self.bb * 2 + if roll < 0.5 { self.bb / 4 } else { 0 })
                    } else if s > 0.45 && roll < 0.06 * loose {
                        Move::Call
                    } else {
                        Move::Fold
                    }
                } else if to_call as f64 >= behind as f64 * 0.45 {
                    if s > 0.74 - (loose - 1.0) * 0.1 { Move::Call } else { Move::Fold }
                } else if s > 0.88 && can_raise {
                    if eff_bb < 28.0 { Move::RaiseTo(i64::MAX) } else { Move::RaiseTo(bet * 3) }
                } else if s > 0.62 / loose {
                    Move::Call
                } else {
                    Move::Fold
                }
            } else if to_call == 0 {
                let p = if s > 0.6 { 0.65 } else { 0.22 * aggressive };
                if can_raise && roll < p {
                    Move::RaiseTo(bet + (pot as f64 * self.rng.range(0.33, 0.8)) as i64)
                } else {
                    Move::Check
                }
            } else {
                let odds = to_call as f64 / (pot + to_call) as f64;
                if s > 0.86 && can_raise && roll < 0.4 * aggressive {
                    Move::RaiseTo(bet * 3)
                } else if s > 0.4 + odds * 0.4 || (s > 0.3 && roll < 0.15 * loose) {
                    Move::Call
                } else {
                    Move::Fold
                }
            };
            match mv {
                Move::Fold => {
                    self.seats[k].folded = true;
                    writeln!(self.out, "{name}: folds").unwrap();
                }
                Move::Check if to_call == 0 => writeln!(self.out, "{name}: checks").unwrap(),
                Move::Check | Move::Call => {
                    if to_call == 0 {
                        writeln!(self.out, "{name}: checks").unwrap();
                    } else {
                        let a = self.put(k, to_call, true);
                        writeln!(self.out, "{name}: calls {a}{}", self.all_in_suffix(k)).unwrap();
                    }
                }
                Move::RaiseTo(to) => {
                    let most = self.seats[k].street + behind;
                    let to = to.max(bet + min_raise).min(most);
                    if to <= bet {
                        let a = self.put(k, to_call, true);
                        writeln!(self.out, "{name}: calls {a}{}", self.all_in_suffix(k)).unwrap();
                        continue;
                    }
                    let add = to - self.seats[k].street;
                    self.put(k, add, true);
                    if bet == 0 {
                        writeln!(self.out, "{name}: bets {to}{}", self.all_in_suffix(k)).unwrap();
                    } else {
                        writeln!(self.out, "{name}: raises {} to {to}{}", to - bet, self.all_in_suffix(k)).unwrap();
                    }
                    if to - bet >= min_raise {
                        min_raise = to - bet;
                    }
                    bet = to;
                    raises += 1;
                }
            }
        }
        // The part of the top bet nobody matched goes back.
        let mut streets: Vec<(i64, usize)> = self.seats.iter().enumerate().map(|(k, s)| (s.street, k)).collect();
        streets.sort_by(|a, b| b.cmp(a));
        if streets.len() > 1 && streets[0].0 > streets[1].0 {
            let (top, k) = streets[0];
            let back = top - streets[1].0;
            let s = &mut self.seats[k];
            s.street -= back;
            s.total -= back;
            s.behind += back;
            s.all_in = false;
            writeln!(self.out, "Uncalled bet ({back}) returned to {}", s.name).unwrap();
        }
        for s in &mut self.seats {
            s.street = 0;
        }
    }
}

/// Plays one hand at the table and returns its text up to the summary and the summary;
/// stacks in `players` change.
fn deal(rng: &mut Rng, players: &mut [Option<Player>], button: &mut usize, h: &HandSetup) -> (String, String) {
    let max = players.len();
    *button = (1..=max).map(|d| (*button + d) % max).find(|&s| players[s].is_some()).unwrap();
    let occupied: Vec<usize> = (0..max).map(|d| (*button + d) % max).filter(|&s| players[s].is_some()).collect();
    let (sb, bb) = LEVELS[h.level];
    let ante = (bb + 4) / 8;
    let mut out = String::new();
    writeln!(
        out,
        "PokerStars Hand #{}: Tournament #{}, {} Hold'em No Limit - Level {} ({sb}/{bb}) - {} ET",
        h.hand_id, h.tournament, h.buy, ROMAN[h.level], h.when
    )
    .unwrap();
    writeln!(out, "Table '{}' {}-max Seat #{} is the button", h.table, h.max_seats, *button + 1).unwrap();
    for (s, p) in players.iter().enumerate() {
        if let Some(p) = p {
            if h.knockout {
                writeln!(out, "Seat {}: {} ({} in chips, {} bounty) ", s + 1, p.name, p.stack, money(p.bounty)).unwrap();
            } else {
                writeln!(out, "Seat {}: {} ({} in chips) ", s + 1, p.name, p.stack).unwrap();
            }
        }
    }
    let mut deck: Vec<Card> = (0..52).map(Card::from_index).collect();
    let mut draw = |rng: &mut Rng| deck.swap_remove(rng.below(deck.len() as u64) as usize);
    // Seats in order from the small blind (heads-up: the button), so index 0 posts it.
    let order: Vec<usize> = if occupied.len() == 2 { occupied.clone() } else { occupied[1..].iter().chain(&occupied[..1]).copied().collect() };
    let seats: Vec<Seat> = order
        .iter()
        .map(|&s| {
            let p = players[s].as_ref().unwrap();
            Seat {
                seat: s,
                name: p.name.clone(),
                cards: [draw(rng), draw(rng)],
                behind: p.stack,
                total: 0,
                street: 0,
                folded: false,
                all_in: false,
                loose: p.loose,
                aggressive: p.aggressive,
            }
        })
        .collect();
    let board: Vec<Card> = (0..5).map(|_| draw(rng)).collect();
    let mut t = Table { seats, board: Vec::new(), out, bb, rng };
    let n = t.seats.len();
    for i in 0..n {
        let a = t.put(i, ante, false);
        let name = t.seats[i].name.clone();
        writeln!(t.out, "{name}: posts the ante {a}{}", t.all_in_suffix(i)).unwrap();
    }
    for (i, (blind, label)) in [(sb, "small blind"), (bb, "big blind")].into_iter().enumerate() {
        if t.seats[i].behind > 0 {
            let a = t.put(i, blind, true);
            let name = t.seats[i].name.clone();
            writeln!(t.out, "{name}: posts {label} {a}{}", t.all_in_suffix(i)).unwrap();
        }
    }
    writeln!(t.out, "*** HOLE CARDS ***").unwrap();
    if let Some(hero) = t.seats.iter().find(|s| s.name == HERO) {
        let line = format!("Dealt to {HERO} [{}]", cards_text(&hero.cards));
        writeln!(t.out, "{line}").unwrap();
    }
    let first_preflop = if n == 2 { 0 } else { 2 % n };
    t.betting(first_preflop, true);
    for (street, cards) in [("FLOP", 3usize), ("TURN", 4), ("RIVER", 5)] {
        if t.seats.iter().filter(|s| !s.folded).count() <= 1 {
            break;
        }
        let before = cards_text(&board[..cards.min(3)]);
        t.board = board[..cards].to_vec();
        match cards {
            3 => writeln!(t.out, "*** FLOP *** [{before}]").unwrap(),
            _ => writeln!(t.out, "*** {street} *** [{}] [{}]", cards_text(&board[..cards - 1]), board[cards - 1]).unwrap(),
        }
        if t.seats.iter().filter(|s| s.active()).count() >= 2 {
            let first = (0..n).find(|&k| t.seats[k].active()).unwrap();
            t.betting(first, false);
        }
    }
    let live: Vec<usize> = (0..n).filter(|&k| !t.seats[k].folded).collect();
    let mut won = vec![0i64; n];
    let total = t.pot();
    let mut pot_lines = Vec::new();
    let showdown = live.len() > 1;
    if showdown {
        writeln!(t.out, "*** SHOW DOWN ***").unwrap();
        for &k in &live {
            let s = &t.seats[k];
            let line = format!("{}: shows [{}] ({})", s.name, cards_text(&s.cards), category_text(s.cards, &board));
            writeln!(t.out, "{line}").unwrap();
        }
        let mut levels: Vec<i64> = live.iter().map(|&k| t.seats[k].total).collect();
        levels.sort();
        levels.dedup();
        let mut prev = 0;
        let pots = levels.len();
        for (pi, &level) in levels.iter().enumerate() {
            let amount: i64 = t.seats.iter().map(|s| (s.total.min(level) - prev).max(0)).sum();
            let eligible: Vec<usize> = live.iter().copied().filter(|&k| t.seats[k].total >= level).collect();
            prev = level;
            if amount == 0 {
                continue;
            }
            let rank = |k: usize| {
                let mut all = board.clone();
                all.extend(t.seats[k].cards);
                evaluate_cards(&all)
            };
            let best = eligible.iter().map(|&k| rank(k)).max().unwrap();
            let winners: Vec<usize> = eligible.iter().copied().filter(|&k| rank(k) == best).collect();
            let share = amount / winners.len() as i64;
            for (wi, &w) in winners.iter().enumerate() {
                let got = share + if wi == 0 { amount - share * winners.len() as i64 } else { 0 };
                won[w] += got;
                let from = if pots == 1 { "pot".to_string() } else if pi == 0 { "main pot".into() } else { format!("side pot-{pi}") };
                pot_lines.push(format!("{} collected {got} from {from}", t.seats[w].name));
            }
        }
    } else {
        let w = live[0];
        won[w] = total;
        pot_lines.push(format!("{} collected {total} from pot", t.seats[w].name));
    }
    for line in &pot_lines {
        writeln!(t.out, "{line}").unwrap();
    }
    let mut out = std::mem::take(&mut t.out);
    let finals: Vec<(usize, i64, String, i64)> =
        t.seats.iter().enumerate().map(|(k, s)| (s.seat, s.behind + won[k], s.name.clone(), won[k])).collect();
    // Knockouts: the biggest winner of the hand takes the bounty of each player who busted.
    if h.knockout {
        let top = (0..n).max_by_key(|&k| won[k]).unwrap();
        for &(seat, stack, ref name, _) in &finals {
            if stack == 0 {
                let bounty = players[seat].as_ref().map_or(0, |p| p.bounty);
                let half = bounty / 2;
                let winner_seat = t.seats[top].seat;
                if let Some(w) = players[winner_seat].as_mut() {
                    w.bounty += bounty - half;
                    writeln!(
                        out,
                        "{} wins {} for eliminating {name} and their own bounty increases by {} to {}",
                        w.name,
                        money(half),
                        money(bounty - half),
                        money(w.bounty)
                    )
                    .unwrap();
                }
            }
        }
    }
    let mut summary = String::new();
    writeln!(summary, "*** SUMMARY ***").unwrap();
    writeln!(summary, "Total pot {total} | Rake 0").unwrap();
    if !t.board.is_empty() {
        writeln!(summary, "Board [{}]", cards_text(&t.board)).unwrap();
    }
    for &(seat, _, ref name, got) in &finals {
        let s = t.seats.iter().find(|s| s.seat == seat).unwrap();
        let what = if s.folded {
            "folded".to_string()
        } else if showdown {
            if got > 0 { format!("showed [{}] and won ({got})", cards_text(&s.cards)) } else { format!("showed [{}] and lost", cards_text(&s.cards)) }
        } else {
            format!("collected ({got})")
        };
        writeln!(summary, "Seat {}: {name} {what}", seat + 1).unwrap();
    }
    for (seat, stack, _, _) in finals {
        if let Some(p) = players[seat].as_mut() {
            p.stack = stack;
        }
    }
    (out, summary.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use ringside_core::{parse_hand, split_hands};

    use super::*;

    #[test]
    fn every_hand_parses_and_chips_add_up() {
        let tournaments = history(3000, 500, 7);
        let total: usize = tournaments.iter().map(|t| t.hands.len()).sum();
        assert_eq!(total, 3000);
        assert!(tournaments.len() >= 10, "{} tournaments", tournaments.len());
        let mut ids = std::collections::HashSet::new();
        let mut finished = 0;
        for t in &tournaments {
            let text = t.text();
            let hands = split_hands(&text);
            assert_eq!(hands.len(), t.hands.len());
            for raw in hands {
                let hand = parse_hand(raw).unwrap_or_else(|e| panic!("{e}\n{raw}"));
                assert!(ids.insert(hand.id), "hand ids are unique");
                assert_eq!(hand.hero.as_deref(), Some(HERO));
                let nets: i64 = hand.seats.iter().map(|s| hand.net(&s.name)).sum();
                assert_eq!(nets, 0, "chips are neither made nor lost\n{raw}");
                finished += hand.finishes.iter().filter(|f| f.player == HERO).count();
            }
        }
        assert!(finished > 0, "the hero busts or wins now and then");
    }
}
