//! The browser demo shows real exercises of the solved kinds: a few seeds per level with
//! the feedback for every answer, written to `src/lib/learn/drills.json`. The test fails
//! when the file is stale; `RINGSIDE_WRITE_DEMO=1 cargo test -p ringside-learn --test demo`
//! writes it again.

use ringside_learn::{Answer, AnswerSpec, Kind, check, generate};
use serde_json::{Map, Value, json};

const KINDS: [Kind; 7] = [Kind::BountyOdds, Kind::IcmCall, Kind::OpenFold, Kind::OpenFoldShort, Kind::RiverBet, Kind::StackDepth, Kind::HudRead];
const SEEDS: u64 = 3;

/// Four decimals are plenty for a picture and keep the file small.
fn round(v: Value) -> Value {
    match v {
        Value::Number(n) if n.is_f64() => {
            let x = n.as_f64().unwrap();
            json!((x * 1e4).round() / 1e4)
        }
        Value::Array(a) => Value::Array(a.into_iter().map(round).collect()),
        Value::Object(o) => Value::Object(o.into_iter().map(|(k, v)| (k, round(v))).collect()),
        v => v,
    }
}

fn drills() -> Value {
    let mut out = Map::new();
    for kind in KINDS {
        let mut levels = Vec::new();
        for level in 1..=3 {
            let mut list = Vec::new();
            for seed in 0..SEEDS {
                let ex = generate(kind, level, seed).unwrap();
                let answers: Vec<(String, Answer)> = match &ex.answer {
                    AnswerSpec::Choice { options } => options.iter().map(|o| (o.id.clone(), Answer::Choice(o.id.clone()))).collect(),
                    // The demo scores numbers itself against the truth of this feedback.
                    _ => vec![("number".into(), Answer::Number(f64::MIN_POSITIVE))],
                };
                let mut feedback = Map::new();
                for (id, a) in answers {
                    let (_, fb) = check(kind, level, seed, &a).unwrap();
                    feedback.insert(id, serde_json::to_value(fb).unwrap());
                }
                list.push(json!({ "exercise": ex, "feedback": feedback }));
            }
            levels.push(Value::Array(list));
        }
        out.insert(kind.id().into(), Value::Array(levels));
    }
    round(Value::Object(out))
}

#[test]
fn demo_drills_are_current() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../src/lib/learn/drills.json");
    let text = serde_json::to_string(&drills()).unwrap() + "\n";
    if std::env::var_os("RINGSIDE_WRITE_DEMO").is_some() {
        std::fs::write(path, &text).unwrap();
        return;
    }
    let Ok(stored) = std::fs::read_to_string(path) else { return };
    assert!(stored == text, "src/lib/learn/drills.json is stale: RINGSIDE_WRITE_DEMO=1 cargo test -p ringside-learn --test demo");
}
