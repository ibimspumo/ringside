//! Reading an opponent from HUD numbers: the player type from VPIP and PFR, the move
//! that works against it, and whether the sample is large enough to say.
//!
//! Types follow common rules of thumb (see [`classify`]). How far a VPIP can be trusted
//! comes from the Wilson 95 % interval of a share measured over `hands` hands.

use rand::Rng;

use crate::deal::{self, Rng8};
use crate::fmt;
use crate::{Answer, AnswerSpec, ChoiceOption, Exercise, Fact, Feedback, Grading, Prompt, Task, choice, feedback};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Type {
    Nit,
    Tag,
    Lag,
    Station,
}

const TYPES: [Type; 4] = [Type::Nit, Type::Tag, Type::Lag, Type::Station];

impl Type {
    fn id(self) -> &'static str {
        match self {
            Type::Nit => "nit",
            Type::Tag => "tag",
            Type::Lag => "lag",
            Type::Station => "station",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Type::Nit => "Nit (sehr eng)",
            Type::Tag => "TAG (eng und aggressiv)",
            Type::Lag => "LAG (locker und aggressiv)",
            Type::Station => "Calling Station (locker und passiv)",
        }
    }

    fn sketch(self) -> &'static str {
        match self {
            Type::Nit => "Spielt nur sehr starke Hände.",
            Type::Tag => "Spielt wenige Hände, die aber mit Raise.",
            Type::Lag => "Spielt viele Hände und erhöht oft.",
            Type::Station => "Spielt viele Hände, callt aber meist nur.",
        }
    }

    fn counter(self) -> (&'static str, &'static str) {
        match self {
            Type::Nit => ("steal", "Öfter seine Blinds stehlen, gegen seine Raises folden"),
            Type::Tag => ("standard", "Solide spielen, ohne große Anpassung"),
            Type::Lag => ("trap", "Weiter callen und mit starken Händen zurückschlagen"),
            Type::Station => ("value", "Viele Value-Bets, kaum bluffen"),
        }
    }

    fn why(self) -> &'static str {
        match self {
            Type::Nit => "Er foldet fast alles. Wenn er doch erhöht, hat er eine starke Hand.",
            Type::Tag => "Er macht wenig falsch. Große Anpassungen kosten hier eher.",
            Type::Lag => "Er erhöht auch mit schwachen Händen. Starke Hände bekommen von ihm viel Geld.",
            Type::Station => "Er foldet selten. Bluffs gehen nicht durch, gute Hände werden bezahlt.",
        }
    }

    /// VPIP range of the type, in percent: below 15 a nit, below 27 a tight player.
    fn band(self) -> (f64, f64) {
        match self {
            Type::Nit => (0.0, 15.0),
            Type::Tag => (15.0, 27.0),
            Type::Lag | Type::Station => (27.0, 100.0),
        }
    }
}

/// The type by rule of thumb, `None` in between: very tight is a nit; tight with most
/// hands raised a TAG; loose with most hands raised a LAG; loose with few raised a station.
pub fn classify(vpip: f64, pfr: f64) -> Option<Type> {
    let aggression = pfr / vpip.max(1e-9);
    if vpip < 15.0 {
        Some(Type::Nit)
    } else if vpip < 27.0 && aggression >= 0.7 {
        Some(Type::Tag)
    } else if vpip >= 28.0 && aggression >= 0.6 {
        Some(Type::Lag)
    } else if vpip >= 30.0 && aggression <= 0.35 {
        Some(Type::Station)
    } else {
        None
    }
}

/// Wilson 95 % interval of a share `p` (0–100) measured over `n` hands, in percent.
pub fn interval(p: f64, n: u32) -> (f64, f64) {
    let (p, n, z) = (p / 100.0, n as f64, 1.96f64);
    let mid = (p + z * z / (2.0 * n)) / (1.0 + z * z / n);
    let half = z / (1.0 + z * z / n) * (p * (1.0 - p) / n + z * z / (4.0 * n * n)).sqrt();
    (100.0 * (mid - half).max(0.0), 100.0 * (mid + half).min(1.0))
}

/// Whether the interval stays inside the type's VPIP range, `None` too close to call.
fn trusted(t: Type, vpip: f64, hands: u32) -> Option<bool> {
    let (lo, hi) = interval(vpip, hands);
    let (a, b) = t.band();
    let edges = [a, b].into_iter().filter(|e| (1.0..99.0).contains(e));
    if edges.flat_map(|e| [lo - e, hi - e]).any(|d| d.abs() < 1.0) {
        return None;
    }
    Some(lo >= a && hi <= b)
}

fn stats(t: Type, rng: &mut Rng8) -> (f64, f64, f64) {
    let (vpip, pfr, three): (f64, f64, f64) = match t {
        Type::Nit => {
            let v = rng.random_range(7.0..13.5f64);
            (v, v * rng.random_range(0.55..0.9), rng.random_range(1.0..3.0))
        }
        Type::Tag => {
            let v = rng.random_range(17.0..24.0f64);
            (v, v - rng.random_range(1.0..4.0), rng.random_range(5.0..8.0))
        }
        Type::Lag => {
            let v = rng.random_range(30.0..40.0f64);
            (v, v * rng.random_range(0.7..0.85), rng.random_range(9.0..14.0))
        }
        Type::Station => (rng.random_range(38.0..55.0), rng.random_range(4.0..10.0), rng.random_range(1.0..4.0)),
    };
    (vpip.round(), pfr.round(), three.round())
}

const SMALL: [u32; 5] = [8, 12, 20, 30, 45];
const LARGE: [u32; 5] = [150, 300, 600, 1200, 2500];

pub fn generate(level: u8, rng: &mut Rng8) -> Task {
    loop {
        let t = *deal::pick(rng, &TYPES);
        let (vpip, pfr, three_bet) = stats(t, rng);
        if classify(vpip, pfr) != Some(t) {
            continue;
        }
        let few = level == 3 && rng.random_bool(0.5);
        let hands = if level < 3 { *deal::pick(rng, &LARGE[2..]) } else if few { *deal::pick(rng, &SMALL) } else { *deal::pick(rng, &LARGE) };
        match trusted(t, vpip, hands) {
            Some(ok) if ok == !few => {}
            _ => continue,
        }
        let shown = format!(
            "VPIP {}, PFR {}, 3-Bet {} aus {} Händen",
            pct0(vpip),
            pct0(pfr),
            pct0(three_bet),
            fmt::chips(hands as f64)
        );
        let (ask, question, options) = match level {
            1 => (
                "type",
                format!("Dein Gegner hat im HUD {shown}. Welcher Spielertyp ist er?"),
                TYPES.iter().map(|t| ChoiceOption::new(t.id(), t.label())).collect(),
            ),
            2 => (
                "counter",
                format!("Dein Gegner hat im HUD {shown}. Was ist gegen ihn der beste Plan?"),
                TYPES.iter().map(|t| ChoiceOption::new(t.counter().0, t.counter().1)).collect(),
            ),
            _ => {
                let mut o: Vec<ChoiceOption> = TYPES.iter().map(|t| ChoiceOption::new(t.id(), t.label())).collect();
                o.push(ChoiceOption::new("few", "Noch zu wenig Hände"));
                ("trust", format!("Dein Gegner hat im HUD {shown}. Was kannst du über ihn sagen?"), o)
            }
        };
        return Task {
            question,
            prompt: Prompt::HudRead { vpip, pfr, three_bet, hands, ask: ask.into() },
            answer: AnswerSpec::Choice { options },
            grading: Grading::Formel,
            model_note: None,
        };
    }
}

/// "35 %": HUD numbers are whole percent.
fn pct0(x: f64) -> String {
    format!("{} %", fmt::num(x, 0))
}

/// "etwa 4 von 10 Händen".
fn of_ten(x: f64) -> String {
    let n = (x / 10.0).round().max(0.0) as u32;
    match n {
        0 => "weniger als 1 von 10 Händen".into(),
        10 => "fast jede Hand".into(),
        n => format!("etwa {n} von 10 Händen"),
    }
}

pub fn grade(ex: &Exercise, answer: &Answer) -> Result<Feedback, String> {
    let Prompt::HudRead { vpip, pfr, three_bet, hands, ask } = &ex.prompt else { unreachable!() };
    let t = classify(*vpip, *pfr).ok_or("Diese Werte passen zu keinem Typ")?;
    let picked = choice(answer)?;
    let (truth, text) = match ask.as_str() {
        "counter" => t.counter(),
        "trust" if trusted(t, *vpip, *hands) == Some(false) => ("few", "Noch zu wenig Hände"),
        _ => (t.id(), t.label()),
    };
    let known = match ask.as_str() {
        "counter" => TYPES.iter().any(|x| x.counter().0 == picked),
        _ => picked == "few" || TYPES.iter().any(|x| x.id() == picked),
    };
    if !known {
        return Err(format!("Unbekannte Antwort '{picked}'"));
    }
    let (lo, hi) = interval(*vpip, *hands);
    let mut facts = vec![
        Fact::new(&format!("VPIP {}", pct0(*vpip)), format!("Er spielt {} freiwillig.", of_ten(*vpip))),
        Fact::new(&format!("PFR {}", pct0(*pfr)), format!("Mit {} erhöht er vor dem Flop.", of_ten(*pfr))),
        Fact::new(&format!("3-Bet {}", pct0(*three_bet)), "So oft erhöht er gegen einen Raise noch einmal."),
        Fact::new("Spielertyp", format!("{}. {}", t.label(), t.sketch())),
    ];
    if ask != "type" {
        facts.push(Fact::new("Gegenzug", format!("{}. {}", t.counter().1, t.why())));
    }
    facts.push(
        Fact::new("Stichprobe", format!("VPIP aus {} Händen liegt mit 95 % Sicherheit zwischen {} und {}.", fmt::chips(*hands as f64), pct0(lo), pct0(hi)))
            .with("Wilson-Intervall, 95 %"),
    );
    if truth == "few" {
        facts.push(Fact::new(
            "Darum noch offen",
            "In dieser Spanne liegen mehrere Spielertypen. Spiel normal, bis mehr Hände da sind.",
        ));
    }
    let correct = picked == truth;
    Ok(feedback(correct, if correct { 1.0 } else { 0.0 }, Answer::Choice(truth.into()), text.into(), facts))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_of_thumb() {
        assert_eq!(classify(10.0, 7.0), Some(Type::Nit));
        assert_eq!(classify(21.0, 18.0), Some(Type::Tag));
        assert_eq!(classify(35.0, 27.0), Some(Type::Lag));
        assert_eq!(classify(45.0, 7.0), Some(Type::Station));
        assert_eq!(classify(24.0, 8.0), None);
    }

    /// Known Wilson intervals; more hands make them narrower.
    #[test]
    fn wilson_interval() {
        let (lo, hi) = interval(50.0, 100);
        assert!((lo - 40.38).abs() < 0.01 && (hi - 59.62).abs() < 0.01, "{lo} {hi}");
        let (lo, hi) = interval(20.0, 10);
        assert!((lo - 5.67).abs() < 0.01 && (hi - 50.98).abs() < 0.01, "{lo} {hi}");
        let narrow = interval(30.0, 1000);
        let wide = interval(30.0, 50);
        assert!(narrow.1 - narrow.0 < wide.1 - wide.0);
    }

    /// Generated numbers always belong to one type, and level 3 asks both kinds of samples.
    #[test]
    fn generated_spots_are_clear() {
        let mut few = 0;
        for seed in 0..200 {
            let mut rng = crate::deal::rng(crate::Kind::HudRead, 3, seed);
            let task = generate(3, &mut rng);
            let Prompt::HudRead { vpip, pfr, hands, .. } = task.prompt else { unreachable!() };
            let t = classify(vpip, pfr).unwrap();
            let trust = trusted(t, vpip, hands).expect("never on the edge");
            few += usize::from(!trust);
        }
        assert!((40..160).contains(&few), "{few} of 200 with too few hands");
    }

    /// Type and counter-move answers follow the rules for every level.
    #[test]
    fn truths_follow_the_rules() {
        for level in 1..=3 {
            for seed in 0..50 {
                let ex = crate::generate(crate::Kind::HudRead, level, seed).unwrap();
                let Prompt::HudRead { vpip, pfr, hands, .. } = &ex.prompt else { unreachable!() };
                let t = classify(*vpip, *pfr).unwrap();
                let want = match level {
                    1 => t.id(),
                    2 => t.counter().0,
                    _ if trusted(t, *vpip, *hands) == Some(false) => "few",
                    _ => t.id(),
                };
                let (_, fb) = crate::check(crate::Kind::HudRead, level, seed, &Answer::Choice(want.into())).unwrap();
                assert!(fb.correct, "{level} {seed}");
            }
        }
    }
}
