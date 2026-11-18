//! Grades of the analysis. They are the practice's ([`ringside_play::grade`]): cut in
//! percent of the pot at the decision, a mix is never a mistake, so a real hand reads the
//! same as a practice hand or a spot on the solver page.

use ringside_core::db::{Choice, Decision, Rating, Source, Status};
use ringside_play::grade::{BEST_PCT, GOOD_PCT, Grade, INACCURATE_PCT, MISTAKE_PCT, MIX_MIN_FREQUENCY, judge};

pub fn rating(g: Grade) -> Rating {
    match g {
        Grade::Best => Rating::Best,
        Grade::Mix => Rating::Mix,
        Grade::Good => Rating::Good,
        Grade::Inaccurate => Rating::Inaccuracy,
        Grade::Mistake => Rating::Mistake,
        Grade::Blunder => Rating::Blunder,
    }
}

/// The grade of the played choice of `strategy` at a pot of `pot_bb`.
#[derive(Clone, Debug, PartialEq)]
pub struct Verdict {
    pub rating: Rating,
    /// The choice with the highest EV.
    pub best: usize,
    /// The choice played most often.
    pub main: usize,
    pub freq: f64,
    pub loss_bb: Option<f64>,
    pub loss_pct: Option<f64>,
}

/// `None` when no choice is marked as played.
pub fn verdict(strategy: &[Choice], pot_bb: f64) -> Option<Verdict> {
    let chosen = strategy.iter().position(|c| c.played)?;
    let freqs: Vec<f64> = strategy.iter().map(|c| c.freq).collect();
    let evs: Option<Vec<f64>> = strategy.iter().map(|c| c.ev_bb).collect();
    let j = judge(&freqs, evs.as_deref(), chosen, pot_bb);
    Some(Verdict { rating: rating(j.grade), best: j.best, main: j.main, freq: j.frequency, loss_bb: j.loss_bb, loss_pct: j.loss_pct })
}

/// What every stored decision promises; the tests and the coverage tool check it.
///
/// - exactly one source, and it fits the status: pending only for the solver, forced
///   moves only as such, rated with any source that computes; nothing stays unrated;
/// - a rated decision has a rating, a loss of at least 0 and the pot it is cut by;
/// - the loss fits the rating: best and mix cost nothing, the other levels follow the
///   thresholds of the practice; an action played at least as often as a mix is never
///   worse than "mix";
/// - the best action (highest EV) costs nothing.
pub fn check(d: &Decision) -> Result<(), String> {
    let fail = |m: &str| Err(format!("#{} @{}: {m} ({d:?})", d.hand_id, d.action_index));
    let Some(source) = d.source else { return fail("keine Quelle") };
    match (d.status, source) {
        (Status::Forced, Source::Forced) | (Status::Pending, Source::Solver) => {}
        (Status::Rated, s) if s != Source::Forced => {}
        _ => return fail("Status passt nicht zur Quelle"),
    }
    if d.status != Status::Rated {
        return if d.rating.is_some() || d.ev_loss_bb.is_some() { fail("Bewertung ohne Status „bewertet“") } else { Ok(()) };
    }
    let (Some(r), Some(loss), Some(pot)) = (d.rating, d.ev_loss_bb, d.pot_bb) else { return fail("Bewertung unvollständig") };
    if !(loss >= 0.0 && loss.is_finite()) {
        return fail("Verlust negativ");
    }
    if pot <= 0.0 {
        return fail("Pot fehlt");
    }
    if source == Source::Estimate && !d.reliability.is_some_and(|x| (0.0..=1.0).contains(&x)) {
        return fail("Einschätzung ohne Verlässlichkeit");
    }
    let pct = loss / pot * 100.0;
    let freq = d.played_freq.unwrap_or(0.0);
    let fits = match r {
        Rating::Best | Rating::Mix => pct <= BEST_PCT + 1e-9 || loss == 0.0,
        Rating::Good => pct <= GOOD_PCT + 1e-9,
        Rating::Inaccuracy => pct > GOOD_PCT && pct <= INACCURATE_PCT + 1e-9,
        Rating::Mistake => pct > INACCURATE_PCT && pct <= MISTAKE_PCT + 1e-9,
        Rating::Blunder => pct > MISTAKE_PCT,
    };
    if !fits {
        return fail("Stufe passt nicht zum Verlust");
    }
    if freq >= MIX_MIN_FREQUENCY && r > Rating::Mix {
        return fail("gemischte Aktion schlechter als „Mix“");
    }
    let top = d.strategy.iter().filter_map(|c| c.ev_bb).fold(f64::NEG_INFINITY, f64::max);
    if d.strategy.iter().any(|c| c.played && c.ev_bb == Some(top)) && loss != 0.0 {
        return fail("beste Aktion mit Verlust");
    }
    Ok(())
}
