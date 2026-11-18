//! Pot odds against an all-in in a progressive knockout (PKO): knocking the shover out
//! pays half their bounty at once, which counts like extra chips in the pot.
//!
//! Needed equity = call ÷ (pot + call + bounty), with the pot counting only what the
//! hero can win: when the shover covers the hero, the rest of the all-in goes back and
//! there is no bounty to win.

use rand::Rng;

use crate::deal::{self, Rng8};
use crate::fmt::{self, chips, dollars, pct};
use crate::score::{band, decision};
use crate::{
    Answer, AnswerSpec, BountyRate, Exercise, Fact, Feedback, Grading, Prompt, Task, choice, feedback, number, options,
};

pub const MODEL_NOTE: &str = "Chip-EV: Das Kopfgeld wird zum Kurs in bb umgerechnet. ICM und der Wert späterer Kopfgelder bleiben außen vor, nach dem Call wird nicht mehr gesetzt.";

/// Start stack and the buy-in's share for the prize pool; the bounty starts at the same amount.
const BUY_INS: [(f64, f64); 3] = [(10_000.0, 5.0), (10_000.0, 10.0), (20_000.0, 50.0)];
/// Big blinds in chips, early to late.
const BIG_BLINDS: [f64; 6] = [400.0, 500.0, 800.0, 1000.0, 2000.0, 4000.0];
/// Bounty on a head as a multiple of the start bounty: it grows with every knockout.
const GROWTH: [f64; 6] = [1.0, 1.5, 2.0, 2.5, 3.0, 4.0];

/// What the hero can win and has to pay, in big blinds.
pub struct Odds {
    pub call: f64,
    /// Blinds, antes and the part of the all-in the hero can win.
    pub pot: f64,
    /// Bounty won with the knockout; 0 when the shover covers the hero.
    pub bounty: f64,
    pub covers: bool,
}

impl Odds {
    pub fn need_pct(&self) -> f64 {
        100.0 * self.call / (self.pot + self.call + self.bounty)
    }

    pub fn need_without_bounty_pct(&self) -> f64 {
        100.0 * self.call / (self.pot + self.call)
    }

    /// Calling against folding, in big blinds.
    pub fn ev_call(&self, equity_pct: f64) -> f64 {
        equity_pct / 100.0 * (self.pot + self.call + self.bounty) - self.call
    }
}

pub fn odds(prompt: &Prompt) -> Odds {
    let Prompt::Bounty { dead_bb, shove_bb, hero_stack_bb, villain_bounty, rate, bounty_bb, .. } = prompt else {
        unreachable!()
    };
    let call = shove_bb.min(*hero_stack_bb);
    let covers = hero_stack_bb >= shove_bb;
    let full = match (bounty_bb, villain_bounty, rate) {
        (Some(b), _, _) => *b,
        (None, Some(v), Some(r)) => 0.5 * v * r.bb_per_dollar,
        _ => 0.0,
    };
    Odds { call, pot: dead_bb + call, bounty: if covers { full } else { 0.0 }, covers }
}

fn rate(rng: &mut Rng8) -> BountyRate {
    loop {
        let (start_chips, prize_dollars) = *deal::pick(rng, &BUY_INS);
        let big_blind_chips = *deal::pick(rng, &BIG_BLINDS);
        let bb_per_dollar = start_chips / prize_dollars / big_blind_chips;
        // Whole or half big blinds per dollar keep the conversion doable in the head.
        if (bb_per_dollar * 2.0).fract() == 0.0 && (0.5..=10.0).contains(&bb_per_dollar) {
            return BountyRate { start_chips, prize_dollars, big_blind_chips, bb_per_dollar };
        }
    }
}

/// Blinds and antes in the pot before the all-in: 1,5 bb plus antes.
fn dead(rng: &mut Rng8) -> f64 {
    *deal::pick(rng, &[1.5, 2.25, 2.5, 2.6])
}

pub fn generate(level: u8, rng: &mut Rng8) -> Task {
    loop {
        let dead_bb = dead(rng);
        let shove_bb = rng.random_range(6..=30) as f64;
        let covers = level < 3 || rng.random_bool(0.6);
        let hero_stack_bb = if covers {
            shove_bb + rng.random_range(2..=25) as f64
        } else {
            (shove_bb - rng.random_range(2..=(shove_bb as i32 - 3).max(2)) as f64).max(3.0)
        };
        let mut prompt = Prompt::Bounty {
            dead_bb,
            shove_bb,
            hero_stack_bb,
            villain_bounty: None,
            hero_bounty: None,
            rate: None,
            bounty_bb: None,
            equity_pct: None,
        };
        if level == 1 {
            let Prompt::Bounty { bounty_bb, .. } = &mut prompt else { unreachable!() };
            *bounty_bb = Some(rng.random_range(2..=24) as f64);
            return task(prompt);
        }
        let r = rate(rng);
        let villain = r.prize_dollars * deal::pick(rng, &GROWTH);
        let hero = r.prize_dollars * deal::pick(rng, &GROWTH);
        let bounty = 0.5 * villain * r.bb_per_dollar;
        if !(1.0..=30.0).contains(&bounty) {
            continue;
        }
        {
            let Prompt::Bounty { villain_bounty, hero_bounty, rate, .. } = &mut prompt else { unreachable!() };
            *villain_bounty = Some(villain);
            *hero_bounty = (level == 3).then_some(hero);
            *rate = Some(r);
        }
        let o = odds(&prompt);
        let (need, plain) = (o.need_pct(), o.need_without_bounty_pct());
        // Mostly equities between the two thresholds: the bounty turns a fold into a call.
        let equity = if o.covers && rng.random_bool(0.6) {
            rng.random_range(need.ceil()..plain.floor().max(need.ceil() + 1.0))
        } else {
            rng.random_range(15.0..70.0f64).round()
        };
        let margin = if level == 2 { 3.0 } else { 1.5 };
        if (equity - need).abs() < margin || !(5.0..=90.0).contains(&equity) {
            continue;
        }
        let Prompt::Bounty { equity_pct, .. } = &mut prompt else { unreachable!() };
        *equity_pct = Some(equity.round());
        return task(prompt);
    }
}

fn task(prompt: Prompt) -> Task {
    let Prompt::Bounty { dead_bb, shove_bb, hero_stack_bb, villain_bounty, hero_bounty, rate, bounty_bb, equity_pct } =
        &prompt
    else {
        unreachable!()
    };
    let mut q = format!(
        "Ein Gegner geht mit {} all-in, vorher lagen {} im Pot. Du hast {}.",
        fmt::bb(*shove_bb),
        fmt::bb(*dead_bb),
        fmt::bb(*hero_stack_bb)
    );
    if let Some(b) = bounty_bb {
        q.push_str(&format!(
            " Wenn du ihn rauswirfst, bekommst du sein Kopfgeld, hier {} wert. Wie viel Equity brauchst du für den Call?",
            fmt::bb(*b)
        ));
        return Task {
            question: q,
            prompt,
            answer: AnswerSpec::Number { unit: "%".into(), min: 0.0, max: 100.0, step: 0.1 },
            grading: Grading::Formel,
            model_note: None,
        };
    }
    let (Some(v), Some(r), Some(e)) = (villain_bounty, rate, equity_pct) else { unreachable!() };
    q.push_str(&format!(
        " Auf seinem Kopf stehen {}, die Hälfte bekommst du sofort. 1 $ ist hier {} wert.",
        dollars(*v),
        fmt::bb(r.bb_per_dollar)
    ));
    if let Some(h) = hero_bounty {
        q.push_str(&format!(" Auf deinem Kopf stehen {}.", dollars(*h)));
    }
    q.push_str(&format!(" Deine Equity beträgt {}. Callst du?", pct(*e)));
    Task {
        question: q,
        prompt,
        answer: options(("call", "Call"), ("fold", "Fold")),
        grading: Grading::Modell,
        model_note: Some(MODEL_NOTE.into()),
    }
}


pub fn grade(ex: &Exercise, answer: &Answer) -> Result<Feedback, String> {
    let Prompt::Bounty { dead_bb, villain_bounty, hero_bounty, rate, equity_pct, .. } = &ex.prompt else { unreachable!() };
    let o = odds(&ex.prompt);
    let need = o.need_pct();
    let mut facts = Vec::new();
    if !o.covers {
        facts.push(Fact::new(
            "Der Gegner deckt dich",
            format!("Er hat mehr Chips. Du kannst nicht sein Kopfgeld gewinnen, nur {}.", fmt::bb(o.pot)),
        ));
        facts.push(
            Fact::new("Was du gewinnen kannst", fmt::bb(o.pot))
                .with(format!("{} + {} (dein Stack, den Rest bekommt er zurück)", fmt::short(*dead_bb), fmt::short(o.call))),
        );
    }
    if let (Some(v), Some(r)) = (villain_bounty, rate) {
        let per_dollar = r.start_chips / r.prize_dollars;
        facts.push(Fact::new("Kurs", format!("1 $ = {}", fmt::bb(r.bb_per_dollar))).with(format!(
            "Startstack {} Chips ÷ {} Preisgeld-Anteil = {} Chips je $, ÷ Big Blind {}",
            chips(r.start_chips),
            dollars(r.prize_dollars),
            chips(per_dollar),
            chips(r.big_blind_chips)
        )));
        if o.covers {
            facts.push(
                Fact::new("Kopfgeld sofort", dollars(0.5 * v))
                    .with(format!("Hälfte von {}", dollars(*v))),
            );
            facts.push(
                Fact::new("Kopfgeld in bb", fmt::bb(o.bounty))
                    .with(format!("{} × {}", dollars(0.5 * v), fmt::short(r.bb_per_dollar))),
            );
        }
    }
    if let Some(h) = hero_bounty {
        facts.push(Fact::new(
            "Dein eigenes Kopfgeld",
            format!("Zählt hier nicht. Die {} gehören dir erst, wenn du jemanden rauswirfst.", dollars(*h)),
        ));
    }
    let formula = if o.bounty > 0.0 {
        format!("{} ÷ ({} + {} + {})", fmt::short(o.call), fmt::short(o.pot), fmt::short(o.call), fmt::short(o.bounty))
    } else {
        format!("{} ÷ ({} + {})", fmt::short(o.call), fmt::short(o.pot), fmt::short(o.call))
    };
    facts.push(Fact::new("Benötigte Equity", pct(need)).with(formula));
    if o.bounty > 0.0 {
        facts.push(Fact::new("Ohne Kopfgeld wären es", pct(o.need_without_bounty_pct())));
    }
    let Some(equity) = equity_pct else {
        let x = number(answer)?;
        let err = x - need;
        return Ok(feedback(err.abs() <= 2.0, band(err, 2.0, 10.0), Answer::Number(need), pct(need), facts));
    };
    let call = match choice(answer)? {
        "call" => true,
        "fold" => false,
        other => return Err(format!("Unbekannte Aktion '{other}'")),
    };
    let ev_call = o.ev_call(*equity);
    let best_call = ev_call > 0.0;
    facts.push(Fact::new("Deine Equity", pct(*equity)));
    facts.push(Fact::new("EV Call", fmt::signed_bb(ev_call)));
    facts.push(Fact::new("EV Fold", fmt::signed_bb(0.0)));
    let correct = call == best_call;
    let loss = if correct { 0.0 } else { ev_call.abs() };
    let (truth, text) = if best_call { ("call", "Call") } else { ("fold", "Fold") };
    let mut fb = feedback(correct, if correct { 1.0 } else { decision(loss) }, Answer::Choice(truth.into()), text.into(), facts);
    fb.ev_loss_bb = Some(loss);
    Ok(fb)
}


#[cfg(test)]
mod tests {
    use super::*;

    fn spot(dead: f64, shove: f64, hero: f64, bounty_bb: f64) -> Prompt {
        Prompt::Bounty {
            dead_bb: dead,
            shove_bb: shove,
            hero_stack_bb: hero,
            villain_bounty: None,
            hero_bounty: None,
            rate: None,
            bounty_bb: Some(bounty_bb),
            equity_pct: None,
        }
    }

    /// Worked examples: 10 bb all-in into 2,5 bb, the bounty worth 5 bb.
    #[test]
    fn known_examples() {
        let o = odds(&spot(2.5, 10.0, 30.0, 5.0));
        assert_eq!((o.call, o.pot, o.bounty), (10.0, 12.5, 5.0));
        // 10 ÷ (12,5 + 10 + 5) = 36,4 % instead of 10 ÷ 22,5 = 44,4 %.
        assert!((o.need_pct() - 100.0 / 2.75).abs() < 1e-9);
        assert!((o.need_without_bounty_pct() - 100.0 * 10.0 / 22.5).abs() < 1e-9);
        // At exactly the needed equity calling is worth nothing.
        assert!(o.ev_call(o.need_pct()).abs() < 1e-9);
        // Covered by the shover: only the own stack counts, and no bounty.
        let o = odds(&spot(2.5, 20.0, 8.0, 5.0));
        assert_eq!((o.call, o.pot, o.bounty, o.covers), (8.0, 10.5, 0.0, false));
    }

    /// PKO: half of a 10 $ bounty at 5 bb per dollar is 25 bb.
    #[test]
    fn pko_converts_half_the_bounty() {
        let p = Prompt::Bounty {
            dead_bb: 1.5,
            shove_bb: 10.0,
            hero_stack_bb: 20.0,
            villain_bounty: Some(10.0),
            hero_bounty: Some(50.0),
            rate: Some(BountyRate { start_chips: 10_000.0, prize_dollars: 5.0, big_blind_chips: 400.0, bb_per_dollar: 5.0 }),
            bounty_bb: None,
            equity_pct: Some(30.0),
        };
        let o = odds(&p);
        assert_eq!(o.bounty, 25.0);
    }

    /// Generated spots are possible, and the truth is the formula's answer.
    #[test]
    fn generated_spots_follow_the_formula() {
        for level in 1..=3 {
            for seed in 0..60 {
                let ex = crate::generate(crate::Kind::BountyOdds, level, seed).unwrap();
                let Prompt::Bounty { dead_bb, shove_bb, hero_stack_bb, villain_bounty, equity_pct, .. } = &ex.prompt else {
                    unreachable!()
                };
                assert!(*dead_bb > 0.0 && *shove_bb > 0.0 && *hero_stack_bb >= 3.0);
                let o = odds(&ex.prompt);
                assert!(o.call <= *hero_stack_bb && o.call <= *shove_bb);
                assert_eq!(o.covers, level < 3 || hero_stack_bb >= shove_bb);
                assert!(o.covers || o.bounty == 0.0);
                assert_eq!(villain_bounty.is_some(), level > 1);
                let (_, fb) = crate::check(crate::Kind::BountyOdds, level, seed, &match equity_pct {
                    Some(_) => Answer::Choice("fold".into()),
                    None => Answer::Number(0.0),
                })
                .unwrap();
                match equity_pct {
                    None => assert_eq!(fb.truth, Answer::Number(o.need_pct())),
                    Some(e) => {
                        assert!((e - o.need_pct()).abs() >= 1.5, "{level} {seed}: too close to call");
                        let call = o.ev_call(*e) > 0.0;
                        assert_eq!(fb.truth, Answer::Choice(if call { "call" } else { "fold" }.into()));
                    }
                }
            }
        }
    }
}
