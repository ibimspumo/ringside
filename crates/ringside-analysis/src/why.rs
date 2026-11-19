//! The reason under a verdict: why the best move is best, from what the source measured.
//! Every sentence states a number its source computed, and only that; what a source does
//! not give (blockers, a range split it does not hold) is left out. The reason speaks of
//! the spot and the best move, never of the move played, so a drill that decides the same
//! spot again shows it unchanged.

use ringside_core::db::Choice;
use ringside_play::grade::BEST_PCT;

use crate::fmt::bb;

/// A move by its kind, as `Choice::kind` names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Move {
    Fold,
    Check,
    Call,
    Limp,
    Bet,
    Raise,
    AllIn,
}

impl Move {
    pub const ALL: [Move; 7] = [Move::Fold, Move::Check, Move::Call, Move::Limp, Move::Bet, Move::Raise, Move::AllIn];

    pub fn of(kind: &str) -> Move {
        match kind {
            "fold" => Move::Fold,
            "check" => Move::Check,
            "limp" => Move::Limp,
            "bet" => Move::Bet,
            "raise" => Move::Raise,
            "all_in" => Move::AllIn,
            _ => Move::Call,
        }
    }

    pub fn aggressive(self) -> bool {
        matches!(self, Move::Bet | Move::Raise | Move::AllIn)
    }

    /// The move as a noun, the way the estimate names it without a size: "Erhöhen".
    pub fn noun(self) -> &'static str {
        match self {
            Move::Fold => "Folden",
            Move::Check => "Checken",
            Move::Call => "Callen",
            Move::Limp => "Limpen",
            Move::Bet => "Setzen",
            Move::Raise => "Erhöhen",
            Move::AllIn => "All-in",
        }
    }

    /// Third person present and the part that goes to the end: "geht … all-in".
    fn verb(self) -> (&'static str, &'static str) {
        match self {
            Move::Fold => ("foldet", ""),
            Move::Check => ("checkt", ""),
            Move::Call => ("callt", ""),
            Move::Limp => ("limpt", ""),
            Move::Bet => ("setzt", ""),
            Move::Raise => ("erhöht", ""),
            Move::AllIn => ("geht", " all-in"),
        }
    }
}

/// "38 %"; a share that rounds to 0 or 100 without being it is named as close to it.
fn pct(x: f64) -> String {
    let r = (x * 100.0).round();
    if r < 1.0 {
        "unter 1 %".into()
    } else if r >= 100.0 && x < 1.0 {
        "über 99 %".into()
    } else {
        format!("{r} %")
    }
}

/// "in 47 % der Fälle", "so gut wie nie".
fn how_often(x: f64) -> String {
    if x < 0.005 { "so gut wie nie".into() } else { format!("in {} der Fälle", pct(x)) }
}

/// The action with the highest EV and the next best one, when the gap between them is
/// more than the noise of a solve (`BEST_PCT` of the pot). `by_kind`: the estimate rates
/// kinds, not sizes, so its runner-up is the best move of another kind.
fn top_two(strategy: &[Choice], by_kind: bool) -> Option<(usize, usize)> {
    let ev = |i: usize| strategy[i].ev_bb.unwrap_or(f64::NEG_INFINITY);
    let top = (0..strategy.len()).filter(|&i| strategy[i].ev_bb.is_some()).max_by(|&a, &b| ev(a).total_cmp(&ev(b)))?;
    let other = (0..strategy.len())
        .filter(|&i| i != top && strategy[i].ev_bb.is_some())
        .filter(|&i| !by_kind || Move::of(&strategy[i].kind) != Move::of(&strategy[top].kind))
        .max_by(|&a, &b| ev(a).total_cmp(&ev(b)))?;
    Some((top, other))
}

/// "Raise auf 7 BB bringt 0,8 BB mehr als Call." when the best move is clearly ahead.
pub fn gap(strategy: &[Choice], pot_bb: f64, estimated: bool) -> Option<String> {
    let (top, other) = top_two(strategy, estimated)?;
    let diff = strategy[top].ev_bb? - strategy[other].ev_bb?;
    if diff / pot_bb.max(1e-9) * 100.0 <= BEST_PCT {
        return None;
    }
    let name = |i: usize| if estimated { Move::of(&strategy[i].kind).noun().to_string() } else { strategy[i].label.clone() };
    let how = if estimated { " geschätzt" } else { "" };
    Some(format!("{} bringt{how} {} mehr als {}.", name(top), bb(diff), name(other)))
}

/// The best move by EV.
pub fn top(strategy: &[Choice]) -> Option<&Choice> {
    strategy.iter().filter(|c| c.ev_bb.is_some()).max_by(|a, b| a.ev_bb.unwrap_or(0.0).total_cmp(&b.ev_bb.unwrap_or(0.0)))
}

/// Where a hand stands in its own range by equity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Third {
    Top,
    Middle,
    Bottom,
}

impl Third {
    /// `above`: the share of the range (by weight) with more equity.
    pub fn of(above: f64) -> Third {
        if above < 1.0 / 3.0 {
            Third::Top
        } else if above < 2.0 / 3.0 {
            Third::Middle
        } else {
            Third::Bottom
        }
    }
}

/// What a solved spot says about the hero's hand.
#[derive(Clone, Debug, PartialEq)]
pub struct Solved {
    /// Equity against the opponent's range at the node.
    pub equity: f64,
    pub third: Option<Third>,
    /// Equity a call needs, when facing a bet.
    pub need: Option<f64>,
    pub river: bool,
    /// How often the opponent folds to the best move, when it is a bet or raise.
    pub fold: Option<f64>,
}

pub fn solver(f: &Solved, strategy: &[Choice], pot_bb: f64) -> String {
    let mut out: Vec<String> = Vec::new();
    let wins = format!("Gegen die Hände, die der Gegner hier haben kann, gewinnt sie in {} der Fälle.", pct(f.equity));
    out.push(match f.third {
        Some(Third::Top) => format!("Deine Hand gehört zum stärksten Drittel der Hände, die du hier haben kannst. {wins}"),
        Some(Third::Middle) => format!("Deine Hand liegt im Mittelfeld der Hände, die du hier haben kannst. {wins}"),
        Some(Third::Bottom) => format!("Deine Hand gehört zum schwächsten Drittel der Hände, die du hier haben kannst. {wins}"),
        None => format!("Gegen die Hände, die der Gegner hier haben kann, gewinnt deine Hand in {} der Fälle.", pct(f.equity)),
    });
    let best = top(strategy);
    let best_move = best.map(|c| Move::of(&c.kind));
    if best_move.is_some_and(Move::aggressive) {
        match f.third {
            Some(Third::Top) if f.equity >= 0.5 => out.push("Damit setzt du auf Wert: Schwächere Hände sollen bezahlen.".into()),
            Some(Third::Bottom) if f.equity < 0.35 => out.push("Damit ist der Einsatz ein Bluff: Er gewinnt vor allem, wenn der Gegner aufgibt.".into()),
            _ => {}
        }
        if let (Some(fold), Some(c)) = (f.fold, best) {
            out.push(format!("Auf {} gibt der Gegner {} auf.", c.label, how_often(fold)));
        }
    }
    if let Some(need) = f.need {
        let enough = f.equity >= need;
        out.push(if enough {
            format!("Für einen Call reichen schon {}.", pct(need))
        } else {
            format!("Ein Call bräuchte {}, so oft gewinnst du nicht.", pct(need))
        });
        if !f.river {
            match best_move {
                Some(Move::Fold) if enough => {
                    out.push("Trotzdem verliert der Call: Auf Turn und River holst du diese Gewinnchance nicht ganz heraus.".into())
                }
                Some(Move::Call) if !enough => {
                    out.push("Trotzdem lohnt der Call: Auf Turn und River gewinnst du mehr dazu, als die Gewinnchance allein zeigt.".into())
                }
                _ => {}
            }
        }
    }
    out.extend(gap(strategy, pot_bb, false));
    out.join(" ")
}

/// Who plays a chart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chart {
    Model,
    Equilibrium,
}

/// Where the hero's hand stands in a preflop chart.
#[derive(Clone, Debug, PartialEq)]
pub struct Ranked {
    pub chart: Chart,
    /// "Als BTN bei 25 BB", "Als BB gegen den Shove bei 10 BB".
    pub place: String,
    /// The move the share counts; `None`: every move but folding ("spielt … weiter").
    pub act: Option<Move>,
    /// Share of the hands that take `act`.
    pub share: f64,
    /// The share counts all hands (push/fold), not only those that reach the spot.
    pub all_hands: bool,
    /// "KJo".
    pub class: String,
    /// Share of the hands with a better margin for `act` than the hero's.
    pub rank: f64,
}

pub fn ranked(f: &Ranked, strategy: &[Choice], pot_bb: f64) -> String {
    let who = match f.chart {
        Chart::Model => "Das Modell",
        Chart::Equilibrium => "Das Gleichgewicht",
    };
    let (verb, rest) = match f.act {
        Some(m) => m.verb(),
        None => ("spielt", " weiter"),
    };
    let (pool, tail, short) = if f.all_hands { ("aller Hände", "", "aller Hände") } else { ("der Hände", ", die hier ankommen", "davon") };
    let mut out = vec![format!("{}: {who} {verb} mit {} {pool}{rest}{tail}.", f.place, pct(f.share))];
    let stand = if f.rank <= 0.5 { format!("zu den besten {}", pct(f.rank.max(0.01))) } else { format!("zu den schwächsten {}", pct((1.0 - f.rank).max(0.01))) };
    let strong = f.rank <= 0.5;
    let fit = if f.rank <= f.share - 0.03 {
        if strong { ", also klar dazu" } else { ", reicht aber noch" }
    } else if f.rank < f.share + 0.03 {
        " und liegt damit genau an der Grenze"
    } else if strong {
        ", das reicht hier nicht"
    } else {
        ", also nicht dazu"
    };
    out.push(format!("{} gehört {stand} {short}{fit}.", f.class));
    out.extend(gap(strategy, pot_bb, false));
    out.join(" ")
}

/// What the estimate found.
#[derive(Clone, Debug, PartialEq)]
pub struct Estimated {
    pub equity: f64,
    pub opponents: usize,
    pub need: Option<f64>,
    /// How often everyone folds to the best move, when it is a bet or raise.
    pub fold: Option<f64>,
    /// Stack to pot after the flop, when no bet is to be called.
    pub spr: Option<f64>,
}

/// Equity this close to the price is within what the estimate can tell apart.
const CLOSE: f64 = 0.05;

pub fn estimate(f: &Estimated, strategy: &[Choice], pot_bb: f64) -> String {
    let against = if f.opponents == 1 { "der Gegner wahrscheinlich hat".to_string() } else { format!("{} Gegner wahrscheinlich haben", f.opponents) };
    let mut out = vec![format!("Gegen die Hände, die {against}, gewinnt deine Hand geschätzt in {} der Fälle.", pct(f.equity))];
    if let Some(need) = f.need {
        out.push(if f.equity >= need + CLOSE {
            format!("Ein Call braucht nur {}, das reicht klar.", pct(need))
        } else if f.equity > need - CLOSE {
            format!("Ein Call braucht {}, das ist knapp.", pct(need))
        } else {
            format!("Ein Call bräuchte {}, das reicht nicht.", pct(need))
        });
    }
    if let (Some(fold), true) = (f.fold, top(strategy).is_some_and(|c| Move::of(&c.kind).aggressive())) {
        let who = if f.opponents == 1 { "gibt der Gegner" } else { "geben alle Gegner" };
        out.push(format!("Auf einen Einsatz {who} geschätzt {} auf.", how_often(fold)));
    }
    if let Some(spr) = f.spr {
        let meaning = if spr < 1.0 {
            "Fast alles ist schon im Pot."
        } else if spr < 3.0 {
            "Ein weiterer Einsatz bringt fast alles hinein."
        } else if spr < 6.0 {
            "Platz für Einsätze auf ein, zwei Straßen."
        } else {
            "Viel Spielraum, die nächsten Straßen entscheiden mit."
        };
        out.push(format!("Stack zu Pot {}: {meaning}", crate::fmt::num(spr)));
    }
    out.extend(gap(strategy, pot_bb, true));
    if f.opponents > 1 {
        out.push("Mit mehreren Gegnern ist die Schätzung unsicherer als heads-up.".into());
    } else if f.need.is_some_and(|n| (f.equity - n).abs() <= CLOSE) {
        out.push("Gewinnchance und Preis liegen so nah beieinander, dass die Schätzung hier unsicher ist.".into());
    }
    out.join(" ")
}
