use ringside_engine::{
    CardSet, EquityResult, HandClass, IcmResult, Method, PushFoldParams, PushFoldResult, Range,
    equity, icm, solve_push_fold,
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

/// serde_json's default float parser may be off by one ulp, so numbers are compared
/// with a tolerance.
fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            let (x, y) = (x.as_f64().unwrap(), y.as_f64().unwrap());
            (x - y).abs() <= 1e-12 * x.abs().max(1.0)
        }
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(x, y)| same(x, y))
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).is_some_and(|w| same(v, w)))
        }
        _ => a == b,
    }
}

fn round_trip<T: Serialize + DeserializeOwned>(value: &T) {
    let text = serde_json::to_string(value).unwrap();
    let back: T = serde_json::from_str(&text).unwrap();
    let (json, again) = (
        serde_json::to_value(value).unwrap(),
        serde_json::to_value(&back).unwrap(),
    );
    assert!(same(&json, &again), "{json} vs {again}");
}

#[test]
fn results_serialize_losslessly() {
    let players = [
        Range::parse("AKs").unwrap(),
        Range::parse("QQ:0.5, JJ").unwrap(),
    ];
    let eq: EquityResult = equity(
        &players,
        CardSet::parse("Ah7c2d").unwrap(),
        CardSet::EMPTY,
        Method::default(),
    )
    .unwrap();
    round_trip(&eq);
    let ic: IcmResult = icm(&[3000.0, 2000.0, 1000.0], &[50.0, 30.0, 20.0]).unwrap();
    round_trip(&ic);
    let pf: PushFoldResult = solve_push_fold(&PushFoldParams::new(8.0)).unwrap();
    round_trip(&pf);
    round_trip(&players[1]);
    round_trip(&"AKo".parse::<HandClass>().unwrap());
    assert_eq!(
        serde_json::to_string(&players[1]).unwrap(),
        "\"QQ:0.5, JJ\""
    );
}
