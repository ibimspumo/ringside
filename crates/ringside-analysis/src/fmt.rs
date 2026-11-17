//! German number and action texts for stored decisions.

use ringside_core::model::{Action, ActionKind, Hand};

/// "2,2 BB"; whole numbers without decimals.
pub fn bb(x: f64) -> String {
    format!("{} BB", num(x))
}

/// One decimal, two below 1 (antes), none for whole numbers.
pub fn num(x: f64) -> String {
    let digits = if x.abs() < 1.0 { 2 } else { 1 };
    let scale = 10f64.powi(digits);
    let r = (x * scale).round() / scale;
    if r.fract() == 0.0 {
        format!("{r:.0}")
    } else {
        format!("{r:.*}", digits as usize).trim_end_matches('0').replace('.', ",")
    }
}

/// What a player did, sizes in BB: "Fold", "Call 3 BB", "Raise auf 2,2 BB", "All-in 14 BB".
pub fn real_action(hand: &Hand, a: &Action) -> String {
    let big = hand.big_blind.max(1) as f64;
    let to = a.to.unwrap_or(a.amount) as f64 / big;
    match a.kind {
        _ if a.all_in && a.kind != ActionKind::Fold => format!("All-in {}", bb(to)),
        ActionKind::Fold => "Fold".into(),
        ActionKind::Check => "Check".into(),
        ActionKind::Call => format!("Call {}", bb(a.amount as f64 / big)),
        ActionKind::Bet => format!("Bet {}", bb(a.amount as f64 / big)),
        ActionKind::Raise => format!("Raise auf {}", bb(to)),
        _ => "Blind".into(),
    }
}
