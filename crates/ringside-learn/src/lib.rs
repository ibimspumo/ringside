//! Ringside's learning area: seeded exercises with graders, exercises built from the
//! user's own hands, and the mastery rules that turn attempts into progress.
//!
//! Exercises are pure functions of `(kind, level, seed)`: [`check`] regenerates the
//! exercise from those three numbers and grades against that, so nothing a client
//! sends back can change the expected answer.

mod deal;
pub mod fmt;
mod math;
pub mod mastery;
mod multiway;
pub mod own;
mod pushfold;
mod score;
mod showdown;
mod table;
mod equity;

use ringside_engine::Card;
use serde::{Deserialize, Serialize};

pub use mastery::{Progress, SkillProgress, progress};
pub use multiway::{TableSpot, lookup_or_solve, position_names};
pub use own::{AllInResult, OwnSpot, detect_spots};

/// Exercise families. Each one is a skill in [`Progress`]; `all_in` only comes from
/// the user's own hands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Outs,
    PotOdds,
    Probability,
    Combos,
    Position,
    Showdown,
    EquityHand,
    EquityRange,
    PushFoldSb,
    PushFoldBb,
    /// Shove or fold from any seat once everyone before folded, 2–9 players.
    PushFoldOpen,
    /// Call or fold against a shove, also after a caller, 2–9 players.
    PushFoldCall,
    AllIn,
}

impl Kind {
    pub const ALL: [Kind; 13] = [
        Kind::Outs,
        Kind::PotOdds,
        Kind::Probability,
        Kind::Combos,
        Kind::Position,
        Kind::Showdown,
        Kind::EquityHand,
        Kind::EquityRange,
        Kind::PushFoldSb,
        Kind::PushFoldBb,
        Kind::PushFoldOpen,
        Kind::PushFoldCall,
        Kind::AllIn,
    ];

    /// The snake_case id used in JSON and in the database.
    pub fn id(self) -> &'static str {
        match self {
            Kind::Outs => "outs",
            Kind::PotOdds => "pot_odds",
            Kind::Probability => "probability",
            Kind::Combos => "combos",
            Kind::Position => "position",
            Kind::Showdown => "showdown",
            Kind::EquityHand => "equity_hand",
            Kind::EquityRange => "equity_range",
            Kind::PushFoldSb => "push_fold_sb",
            Kind::PushFoldBb => "push_fold_bb",
            Kind::PushFoldOpen => "push_fold_open",
            Kind::PushFoldCall => "push_fold_call",
            Kind::AllIn => "all_in",
        }
    }

    pub fn parse(s: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| k.id() == s)
    }

    pub fn name_de(self) -> &'static str {
        match self {
            Kind::Outs => "Outs zählen",
            Kind::PotOdds => "Pot Odds",
            Kind::Probability => "Trefferwahrscheinlichkeit",
            Kind::Combos => "Kombinatorik",
            Kind::Position => "Positionen",
            Kind::Showdown => "Showdown",
            Kind::EquityHand => "Equity Hand gegen Hand",
            Kind::EquityRange => "Equity gegen eine Range",
            Kind::PushFoldSb => "Push/Fold im Small Blind",
            Kind::PushFoldBb => "Callen gegen einen Shove im Big Blind",
            Kind::PushFoldOpen => "Shove oder Fold, wenn alle zu dir gefoldet haben",
            Kind::PushFoldCall => "Call gegen einen Shove",
            Kind::AllIn => "All-ins eigener Hände",
        }
    }

    /// Whether [`generate`] can make exercises of this kind.
    pub fn generated(self) -> bool {
        self != Kind::AllIn
    }
}

/// How firm the expected answer is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Grading {
    /// Exact combinatorics or enumeration.
    Exakt,
    /// A formula or rule of thumb, graded against its own result.
    Formel,
    /// Exact within a stated model; `model_note` names the assumptions.
    Modell,
}

/// One exercise as shown to the user. `prompt` holds what to draw, `answer` what kind
/// of input to offer. For generated exercises `hand_id` is `None` and `(kind, level,
/// seed)` reproduces the exercise; for own-hand spots `hand_id` is set and `seed` is 0.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Exercise {
    pub kind: Kind,
    /// 1–3.
    pub level: u8,
    pub seed: u64,
    pub hand_id: Option<u64>,
    /// The question in German, one or two sentences.
    pub question: String,
    pub prompt: Prompt,
    pub answer: AnswerSpec,
    pub grading: Grading,
    /// German text naming the model's assumptions; set when `grading` is `modell`.
    pub model_note: Option<String>,
}

/// What the exercise shows, one variant per kind (`push_fold` serves both heads-up
/// push/fold kinds, `push_fold_table` both table kinds). Cards are strings like "Ah", "Td"; amounts are in big blinds unless named
/// otherwise.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Prompt {
    /// Hero is behind and looks for the cards that put them ahead on the next card.
    /// `villain` is always known in stage 1.
    Outs {
        hero: Vec<Card>,
        villain: Option<Vec<Card>>,
        /// Flop (3) or turn (4).
        board: Vec<Card>,
    },
    /// The pot already holds the villain's bet; hero has to call `to_call`.
    /// `equity_pct` is given on level 3 (call or fold).
    PotOdds {
        pot: f64,
        to_call: f64,
        equity_pct: Option<f64>,
    },
    /// Chance to hit one of `outs` with `cards_to_come` cards from `unseen` cards.
    Probability {
        outs: u8,
        unseen: u8,
        cards_to_come: u8,
        /// `rule`: what the rule of 2 and 4 says; `exact`: the exact chance.
        method: ProbabilityMethod,
    },
    /// Count the combos of `class` or of `range` that don't touch `dead`.
    Combos {
        class: Option<String>,
        range: Option<String>,
        dead: Vec<Card>,
    },
    /// Seats are numbered 1..=table_size clockwise. Blinds are only given when the
    /// table has a dead button or a missing small blind (level 3).
    Position {
        table_size: u8,
        occupied: Vec<u8>,
        button: u8,
        dead_button: bool,
        small_blind: Option<u8>,
        big_blind: Option<u8>,
        ask: PositionAsk,
    },
    /// Who wins at showdown? Players are numbered from 0.
    Showdown {
        board: Vec<Card>,
        players: Vec<Vec<Card>>,
    },
    /// Hero's all-in equity against one known hand.
    EquityHand {
        hero: Vec<Card>,
        villain: Vec<Card>,
        board: Vec<Card>,
    },
    /// Hero's all-in equity against every hand of a named range.
    EquityRange {
        hero: Vec<Card>,
        range_name: String,
        /// Range notation, e.g. "QQ+, AKs, AKo".
        range: String,
        /// Share of all 1326 combos in the range, in percent.
        range_pct: f64,
        board: Vec<Card>,
    },
    /// Heads-up push/fold between SB and BB, everyone else folded. `hero_position` is
    /// "sb" (shove or fold) or "bb" (call or fold against a shove).
    PushFold {
        hero_position: String,
        hand: Vec<Card>,
        /// Starting hand class, e.g. "A5s".
        class: String,
        /// Effective stack before posting, blinds and own ante included.
        stack_bb: f64,
        small_blind_bb: f64,
        /// Ante paid by each of SB and BB.
        ante_bb: f64,
        /// Other money in the pot: antes of folded players, a big-blind ante.
        dead_bb: f64,
        /// Players dealt in, for the table picture.
        players: u8,
    },
    /// Push/fold at a table of 2–9 players. Seats are in preflop action order: the first
    /// player after the big blind first, the big blind last.
    PushFoldTable {
        hand: Vec<Card>,
        class: String,
        /// Position name per seat: "UTG", …, "BTN", "SB", "BB".
        positions: Vec<String>,
        /// Stack per seat before posting.
        stacks_bb: Vec<f64>,
        hero: usize,
        /// `None`: everyone before the hero folded, hero shoves or folds.
        shover: Option<usize>,
        /// Seats that called the shove before the hero.
        callers: Vec<usize>,
        /// Hero's stack against the deepest player it can lose to.
        effective_bb: f64,
        small_blind_bb: f64,
        /// Ante paid by every player.
        ante_bb: f64,
        /// Ante paid by the big blind alone.
        bb_ante_bb: f64,
        /// Other money in the pot, e.g. antes of players who are not dealt in.
        dead_bb: f64,
    },
    /// An all-in from the user's own hand with every hand known. Pots hold side pots
    /// separately; player 0 is the hero, then `opponents` in order.
    AllIn {
        hero: Vec<Card>,
        opponents: Vec<Vec<Card>>,
        /// Board when the last chips went in.
        board: Vec<Card>,
        pots: Vec<PotView>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbabilityMethod {
    Rule,
    Exact,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PositionAsk {
    /// Tap the seat that has this label; answer with `seat`.
    SeatOf { label: String },
    /// Name this seat's label; answer with `choice` from the offered labels.
    LabelOf { seat: u8 },
}

/// One pot of an all-in, in big blinds. `players` are the indices that can win it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PotView {
    pub amount_bb: f64,
    pub players: Vec<usize>,
}

/// The input the UI offers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AnswerSpec {
    /// A number in `unit` ("%", "Kombos", "Outs").
    Number { unit: String, min: f64, max: f64, step: f64 },
    /// One of `options`; answer with its `id`.
    Choice { options: Vec<ChoiceOption> },
    /// Any subset of `from`; a `number` answer (the count) is accepted as well.
    Cards { from: Vec<Card> },
    /// One of `seats`.
    Seat { seats: Vec<u8> },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChoiceOption {
    pub id: String,
    pub label: String,
}

impl ChoiceOption {
    fn new(id: &str, label: &str) -> ChoiceOption {
        ChoiceOption { id: id.into(), label: label.into() }
    }
}

/// The user's answer: `{"number": 23.5}`, `{"choice": "fold"}`, `{"cards": ["Ah"]}` or
/// `{"seat": 3}`. Percentages are given as 0–100.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Answer {
    Number(f64),
    Choice(String),
    Cards(Vec<String>),
    Seat(u8),
}

/// The result of grading one answer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Feedback {
    pub correct: bool,
    /// 0–1. Estimates score fully within a tolerance band and fall linearly to 0;
    /// wrong exact answers and wrong decisions get at most 0.5.
    pub score: f64,
    /// The expected answer in the same shape as [`Answer`]; for a mixed push/fold
    /// class the more frequent action (both are graded correct).
    pub truth: Answer,
    /// The expected answer as German text, e.g. "22,7 %".
    pub truth_text: String,
    /// Numbers that explain the answer, in reading order.
    pub facts: Vec<Fact>,
    /// Cards to mark on the table (outs exercises).
    pub highlight: Vec<Highlight>,
    /// What the chosen action costs against the best one, in big blinds per hand.
    pub ev_loss_bb: Option<f64>,
    pub grading: Grading,
    pub model_note: Option<String>,
}

/// One explaining line, e.g. `{label: "Benötigte Equity", value: "22,7 %",
/// formula: "4,60 ÷ (15,70 + 4,60)"}`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Fact {
    pub label: String,
    pub value: String,
    pub formula: Option<String>,
}

impl Fact {
    fn new(label: &str, value: impl Into<String>) -> Fact {
        Fact { label: label.into(), value: value.into(), formula: None }
    }

    fn with(mut self, formula: impl Into<String>) -> Fact {
        self.formula = Some(formula.into());
        self
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Highlight {
    pub card: Card,
    pub tag: HighlightTag,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HighlightTag {
    /// An out (picked, or shown after a number answer).
    Out,
    /// An out the user did not pick.
    Missed,
    /// A card that splits the pot.
    HalfOut,
    /// Picked, but not an out.
    Wrong,
    /// Improves the hero's hand but still loses: a dirty out.
    Dirty,
}

/// What a generator returns; [`generate`] adds kind, level and seed.
struct Task {
    question: String,
    prompt: Prompt,
    answer: AnswerSpec,
    grading: Grading,
    model_note: Option<String>,
}

pub fn generate(kind: Kind, level: u8, seed: u64) -> Result<Exercise, String> {
    if !(1..=3).contains(&level) {
        return Err(format!("Stufe {level} gibt es nicht (1–3)"));
    }
    let mut rng = deal::rng(kind, level, seed);
    let task = match kind {
        Kind::Outs => math::outs(level, &mut rng)?,
        Kind::PotOdds => math::pot_odds(level, &mut rng),
        Kind::Probability => math::probability(level, &mut rng),
        Kind::Combos => math::combos(level, &mut rng),
        Kind::Position => table::position(level, &mut rng),
        Kind::Showdown => showdown::generate(level, &mut rng)?,
        Kind::EquityHand => equity::hand(level, &mut rng),
        Kind::EquityRange => equity::range(level, &mut rng),
        Kind::PushFoldSb | Kind::PushFoldBb => pushfold::generate(kind, level, &mut rng)?,
        Kind::PushFoldOpen | Kind::PushFoldCall => multiway::generate(kind, level, &mut rng)?,
        Kind::AllIn => return Err("All-in-Übungen gibt es nur aus eigenen Händen".into()),
    };
    Ok(Exercise {
        kind,
        level,
        seed,
        hand_id: None,
        question: task.question,
        prompt: task.prompt,
        answer: task.answer,
        grading: task.grading,
        model_note: task.model_note,
    })
}

/// Regenerates the exercise from `(kind, level, seed)` and grades `answer` against it.
pub fn check(kind: Kind, level: u8, seed: u64, answer: &Answer) -> Result<(Exercise, Feedback), String> {
    let exercise = generate(kind, level, seed)?;
    let feedback = grade(&exercise, answer)?;
    Ok((exercise, feedback))
}

/// Grades against the given exercise as it stands; callers must not pass exercises
/// that came back from a client (use [`check`] or [`own::check_spot`]).
fn grade(ex: &Exercise, answer: &Answer) -> Result<Feedback, String> {
    let mut feedback = match &ex.prompt {
        Prompt::Outs { .. } => math::grade_outs(ex, answer)?,
        Prompt::PotOdds { .. } => math::grade_pot_odds(ex, answer)?,
        Prompt::Probability { .. } => math::grade_probability(ex, answer)?,
        Prompt::Combos { .. } => math::grade_combos(ex, answer)?,
        Prompt::Position { .. } => table::grade_position(ex, answer)?,
        Prompt::Showdown { .. } => showdown::grade(ex, answer)?,
        Prompt::EquityHand { .. } => equity::grade_hand(ex, answer)?,
        Prompt::EquityRange { .. } => equity::grade_range(ex, answer)?,
        Prompt::PushFold { .. } => pushfold::grade(ex, answer)?,
        Prompt::PushFoldTable { .. } => multiway::grade(ex, answer)?,
        Prompt::AllIn { .. } => equity::grade_all_in(ex, answer)?,
    };
    feedback.grading = ex.grading;
    feedback.model_note = ex.model_note.clone();
    Ok(feedback)
}

fn number(answer: &Answer) -> Result<f64, String> {
    match answer {
        Answer::Number(x) if x.is_finite() => Ok(*x),
        _ => Err("Hier wird eine Zahl erwartet".into()),
    }
}

fn choice(answer: &Answer) -> Result<&str, String> {
    match answer {
        Answer::Choice(c) => Ok(c),
        _ => Err("Hier wird eine Auswahl erwartet".into()),
    }
}

/// Feedback with the fields every grader fills; `grade` sets grading and model note.
fn feedback(correct: bool, score: f64, truth: Answer, truth_text: String, facts: Vec<Fact>) -> Feedback {
    Feedback {
        correct,
        score: score.clamp(0.0, 1.0),
        truth,
        truth_text,
        facts,
        highlight: Vec::new(),
        ev_loss_bb: None,
        grading: Grading::Exakt,
        model_note: None,
    }
}
