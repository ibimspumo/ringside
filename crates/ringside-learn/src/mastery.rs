//! Mastery per skill, derived by replaying the stored attempts in order, so the rules
//! live in one place and a rule change applies to the whole history.
//!
//! Levels: 0 neu (no attempt), 1 kennengelernt (attempted), 2 mit Hilfe (a window of
//! at least 3 attempts with a mean of at least 60 %; a window under 3 attempts keeps
//! the previous level), 3 selbstständig, 4 bestätigt. The window is the last 8 attempts at the highest exercise level the user
//! has tried in that skill. Selbstständig: a full window with a mean of at least 80 %
//! spread over at least 2 days. Bestätigt: the same holds again at least 3 days after
//! selbstständig was reached. A full window below 60 % drops 3 and 4 back to what the window shows (1).
//! Days are UTC days.

use ringside_core::db::Attempt;
use serde::{Deserialize, Serialize};

use crate::Kind;

pub const LEVEL_NAMES: [&str; 5] = ["neu", "kennengelernt", "mit Hilfe", "selbstständig", "bestätigt"];
const WINDOW: usize = 8;
const DAY: i64 = 86_400;
const CONFIRM_AFTER_DAYS: i64 = 3;

/// Progress over all skills.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Progress {
    /// One entry per [`Kind`], in [`Kind::ALL`] order.
    pub skills: Vec<SkillProgress>,
    /// Exercises answered since Monday 00:00 UTC; mistakes count the same.
    pub xp_week: u32,
    pub xp_total: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SkillProgress {
    pub id: Kind,
    pub name_de: String,
    /// 0–4, see the module documentation.
    pub level: u8,
    /// "neu", "kennengelernt", "mit Hilfe", "selbstständig" or "bestätigt".
    pub level_name: String,
    /// Highest exercise level tried (0 without attempts); the window is taken there.
    pub max_level: u8,
    pub attempts: u32,
    /// Mean score (0–1) of the window.
    pub mean_score: Option<f64>,
    /// Unix seconds of the latest attempt.
    pub last_at: Option<i64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample {
    pub level: u8,
    pub score: f64,
    /// Unix seconds.
    pub at: i64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SkillState {
    pub level: u8,
    pub max_level: u8,
    pub mean_score: Option<f64>,
}

/// Replays `samples` (oldest first) through the mastery rules.
pub fn skill_state(samples: &[Sample]) -> SkillState {
    let mut state = SkillState::default();
    let mut independent_since: Option<i64> = None;
    for (i, s) in samples.iter().enumerate() {
        state.max_level = state.max_level.max(s.level);
        let window: Vec<&Sample> =
            samples[..=i].iter().rev().filter(|x| x.level == state.max_level).take(WINDOW).collect();
        let mean = window.iter().map(|x| x.score).sum::<f64>() / window.len() as f64;
        let full = window.len() == WINDOW;
        let mut days: Vec<i64> = window.iter().map(|x| x.at.div_euclid(DAY)).collect();
        days.sort_unstable();
        days.dedup();
        let today = s.at.div_euclid(DAY);
        let holds = full && mean >= 0.8 && days.len() >= 2;
        if state.level >= 3 && full && mean < 0.6 {
            state.level = 2;
            independent_since = None;
        } else if holds {
            match independent_since {
                None => {
                    state.level = 3;
                    independent_since = Some(today);
                }
                Some(since) if today >= since + CONFIRM_AFTER_DAYS => state.level = 4,
                Some(_) => {}
            }
        }
        if state.level < 3 && window.len() >= 3 {
            state.level = if mean >= 0.6 { 2 } else { 1 };
        }
        state.level = state.level.max(1);
        state.mean_score = Some(mean);
    }
    state
}

/// Monday 00:00 UTC of the week containing `now` (Unix seconds).
fn week_start(now: i64) -> i64 {
    let day = now.div_euclid(DAY);
    // 1970-01-01 was a Thursday.
    (day - (day + 3).rem_euclid(7)) * DAY
}

pub fn progress(attempts: &[Attempt], now: i64) -> Progress {
    let skills = Kind::ALL
        .into_iter()
        .map(|kind| {
            let mut samples: Vec<Sample> = attempts
                .iter()
                .filter(|a| a.kind == kind.id())
                .map(|a| Sample { level: a.level, score: a.score, at: a.at })
                .collect();
            samples.sort_by_key(|s| s.at);
            let state = skill_state(&samples);
            SkillProgress {
                id: kind,
                name_de: kind.name_de().into(),
                level: state.level,
                level_name: LEVEL_NAMES[state.level as usize].into(),
                max_level: state.max_level,
                attempts: samples.len() as u32,
                mean_score: state.mean_score,
                last_at: samples.last().map(|s| s.at),
            }
        })
        .collect();
    let start = week_start(now);
    Progress {
        skills,
        xp_week: attempts.iter().filter(|a| a.at >= start && a.at <= now).count() as u32,
        xp_total: attempts.len() as u32,
    }
}

/// Exercise level to practise next: the highest tried, one up once it is solid.
pub fn next_level(s: &SkillProgress) -> u8 {
    if s.max_level == 0 {
        return 1;
    }
    if s.level >= 3 && s.mean_score.unwrap_or(0.0) >= 0.8 {
        return (s.max_level + 1).min(3);
    }
    s.max_level
}

/// Days since the last attempt, or `None` without one.
pub fn days_since(s: &SkillProgress, now: i64) -> Option<f64> {
    s.last_at.map(|at| (now - at) as f64 / DAY as f64)
}

/// Due for review: practised before, and either shaky or not seen for a while (sooner when
/// the level is low). Only generated exercises are due; the other kinds schedule their own.
pub fn is_due(s: &SkillProgress, now: i64) -> bool {
    if s.attempts == 0 || !s.id.generated() {
        return false;
    }
    let gap = [0.0, 0.5, 1.0, 3.0, 7.0][s.level.min(4) as usize];
    days_since(s, now).unwrap_or(0.0) >= gap || s.mean_score.unwrap_or(1.0) < 0.6
}

#[cfg(test)]
mod tests {
    use super::*;

    const T0: i64 = 1_700_000_000;

    fn run(scores: &[(u8, f64, i64)]) -> SkillState {
        let samples: Vec<Sample> =
            scores.iter().map(|&(level, score, day)| Sample { level, score, at: T0 + day * DAY }).collect();
        skill_state(&samples)
    }

    fn many(level: u8, score: f64, day: i64, n: usize) -> Vec<(u8, f64, i64)> {
        vec![(level, score, day); n]
    }

    #[test]
    fn new_and_first_steps() {
        assert_eq!(run(&[]).level, 0);
        assert_eq!(run(&[(1, 0.0, 0)]).level, 1);
        assert_eq!(run(&many(1, 1.0, 0, 2)).level, 1);
        assert_eq!(run(&many(1, 0.7, 0, 3)).level, 2);
        assert_eq!(run(&many(1, 0.5, 0, 5)).level, 1);
    }

    #[test]
    fn independent_needs_eight_attempts_on_two_days() {
        // Eight perfect answers on one day are not enough.
        assert_eq!(run(&many(1, 1.0, 0, 20)).level, 2);
        let mut s = many(1, 1.0, 0, 7);
        assert_eq!(run(&s).level, 2);
        s.push((1, 1.0, 1));
        assert_eq!(run(&s).level, 3);
        // A mean just below 80 % is not enough.
        let mut s = many(1, 0.78, 0, 4);
        s.extend(many(1, 0.78, 1, 4));
        assert_eq!(run(&s).level, 2);
    }

    #[test]
    fn only_the_highest_level_counts() {
        let mut s = many(1, 1.0, 0, 4);
        s.extend(many(1, 1.0, 1, 4));
        s.push((2, 0.0, 1));
        let state = run(&s);
        // Already independent; the level-2 window is not full, so nothing drops.
        assert_eq!((state.level, state.max_level), (3, 2));
        assert_eq!(state.mean_score, Some(0.0));
        // Before reaching it, a new level starts a fresh window that needs 3 attempts
        // before it can change 1 or 2.
        let mut s = many(1, 1.0, 0, 7);
        s.push((2, 0.0, 1));
        assert_eq!(run(&s).level, 2);
        s.extend(many(2, 0.0, 1, 2));
        assert_eq!(run(&s).level, 1);
    }

    #[test]
    fn confirmed_after_three_more_days() {
        let mut s = many(1, 1.0, 0, 4);
        s.extend(many(1, 1.0, 1, 4));
        s.extend(many(1, 1.0, 3, 1));
        assert_eq!(run(&s).level, 3, "two days later is too early");
        s.extend(many(1, 1.0, 4, 1));
        assert_eq!(run(&s).level, 4);
    }

    #[test]
    fn mastery_drops_below_sixty_percent() {
        let mut s = many(1, 1.0, 0, 4);
        s.extend(many(1, 1.0, 1, 4));
        s.extend(many(1, 1.0, 5, 1));
        assert_eq!(run(&s).level, 4);
        s.extend(many(1, 0.0, 6, 3));
        assert_eq!(run(&s).level, 4, "mean 5/8 stays at or above 60 %");
        s.extend(many(1, 0.0, 6, 1));
        assert_eq!(run(&s).level, 1);
        // Back to 80 % over two days: independent again, not confirmed.
        s.extend(many(1, 1.0, 7, 4));
        s.extend(many(1, 1.0, 8, 4));
        assert_eq!(run(&s).level, 3);
    }

    /// Whatever the history, the level stays in range and agrees with its window.
    #[test]
    fn invariants_on_random_histories() {
        use rand::{Rng, SeedableRng};
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(11);
        for _ in 0..500 {
            let n = rng.random_range(0..40);
            let mut day = 0;
            let mut s = Vec::new();
            for _ in 0..n {
                day += rng.random_range(0..2);
                s.push((rng.random_range(1..=3), rng.random_range(0..=4) as f64 / 4.0, day));
            }
            let state = run(&s);
            assert!(state.level <= 4);
            assert_eq!(state.level == 0, s.is_empty());
            let window: Vec<f64> =
                s.iter().rev().filter(|x| x.0 == state.max_level).take(WINDOW).map(|x| x.1).collect();
            if window.len() == WINDOW && window.iter().sum::<f64>() / 8.0 < 0.6 {
                assert!(state.level <= 2, "{s:?}");
            }
            if state.level >= 3 {
                assert!(s.len() >= WINDOW);
            }
        }
    }

    #[test]
    fn weekly_xp_counts_every_answer_since_monday() {
        // 2023-11-13 was a Monday.
        let monday = 1_699_833_600;
        assert_eq!(week_start(monday + 3 * DAY + 5), monday);
        assert_eq!(week_start(monday), monday);
        assert_eq!(week_start(monday - 1), monday - 7 * DAY);
        let attempt = |at: i64, score: f64| Attempt {
            kind: "outs".into(),
            level: 1,
            seed: Some(1),
            hand_id: None,
            answer: "{}".into(),
            score,
            ev_loss_bb: None,
            at,
        };
        let list = vec![attempt(monday - 10, 1.0), attempt(monday + 10, 0.0), attempt(monday + 20, 1.0)];
        let p = progress(&list, monday + 100);
        assert_eq!((p.xp_week, p.xp_total), (2, 3));
        let outs = &p.skills[0];
        assert_eq!((outs.id, outs.attempts, outs.last_at), (Kind::Outs, 3, Some(monday + 20)));
        assert_eq!(p.skills.len(), Kind::ALL.len());
    }
}
