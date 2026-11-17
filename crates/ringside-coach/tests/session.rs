//! The tool-calling loop against a local fake OpenRouter.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ringside_coach::openrouter::ModelInfo;
use ringside_coach::report::{Source, Verdict};
use ringside_coach::{ApiError, Client, CoachError, Prepared, Progress, prepare, run};
use ringside_core::model::Hand;
use ringside_core::tournament::{BuyIn, TournamentRow};
use serde_json::{Value, json};

struct Reply {
    status: u16,
    headers: &'static str,
    body: String,
    delay: Duration,
}

fn ok(body: Value) -> Reply {
    Reply { status: 200, headers: "", body: body.to_string(), delay: Duration::ZERO }
}

/// Serves the scripted replies in order and records every request body.
struct FakeServer {
    url: String,
    requests: Arc<Mutex<Vec<Value>>>,
}

fn serve(replies: Vec<Reply>) -> FakeServer {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/api/v1", listener.local_addr().unwrap());
    let requests = Arc::new(Mutex::new(Vec::new()));
    let seen = requests.clone();
    std::thread::spawn(move || {
        let mut replies = replies.into_iter();
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut length = 0;
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                    break;
                }
                if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = v.trim().parse().unwrap();
                }
            }
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            seen.lock().unwrap().push(serde_json::from_slice(&body).unwrap_or(Value::Null));
            let Some(r) = replies.next() else { return };
            std::thread::sleep(r.delay);
            let _ = write!(
                stream,
                "HTTP/1.1 {} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{}\r\n{}",
                r.status,
                r.body.len(),
                r.headers,
                r.body
            );
        }
    });
    FakeServer { url, requests }
}

fn row() -> TournamentRow {
    TournamentRow {
        id: 77,
        game: "Hold'em No Limit".into(),
        currency: Some("USD".into()),
        buy_in: Some(BuyIn { prize: 490, bounty: 0, fee: 60, currency: Some("USD".into()) }),
        cost: Some(550),
        entrants: Some(180),
        place: Some(23),
        prize: Some(0),
        bounties: 0,
        profit: Some(-550),
        started_at: "2026/11/16 20:00:00".into(),
        hands: 3,
        chips: 0,
        has_summary: true,
        name: Some("Geheimer Turniername".into()),
    }
}

fn hand(id: u64, body: &str) -> Hand {
    let text = format!(
        "PokerStars Hand #{id}: Tournament #77, $4.90+$0.60 USD Hold'em No Limit - Level V (50/100) - 2026/11/16 20:{:02}:00 CET [2026/11/16 14:00:00 ET]
Table '77 1' 6-max Seat #1 is the button
Seat 1: echterName (3000 in chips)
Seat 2: GegnerEins (800 in chips)
Seat 3: GegnerZwei (5000 in chips)
Seat 4: GegnerDrei (4000 in chips)
GegnerEins: posts small blind 50
GegnerZwei: posts big blind 100
*** HOLE CARDS ***
Dealt to echterName [Ah 7c]
{body}
*** SUMMARY ***
Total pot 1000 | Rake 0
",
        id % 60
    );
    ringside_core::parse_hand(&text).unwrap()
}

fn hands() -> Vec<Hand> {
    vec![
        hand(101, "GegnerDrei: folds\nechterName: folds\nGegnerEins: folds\nGegnerZwei collected 150 from pot"),
        hand(
            102,
            "GegnerDrei: raises 200 to 300\nechterName: calls 300\nGegnerEins: folds\nGegnerZwei: folds\n*** FLOP *** [2c 3d 9h]\nGegnerDrei: bets 400\nechterName: calls 400\n*** TURN *** [2c 3d 9h] [Ks]\nGegnerDrei: checks\nechterName: checks\n*** RIVER *** [2c 3d 9h Ks] [4d]\nGegnerDrei: bets 900\nechterName: folds\nUncalled bet (900) returned to GegnerDrei\nGegnerDrei collected 1550 from pot",
        ),
        hand(
            103,
            "GegnerDrei: folds\nechterName: raises 200 to 300\nGegnerEins: raises 500 to 800 and is all-in\nGegnerZwei: folds\nechterName: calls 500\n*** FLOP *** [2c 3d 9h]\n*** TURN *** [2c 3d 9h] [Ks]\n*** RIVER *** [2c 3d 9h Ks] [4d]\n*** SHOW DOWN ***\nechterName: shows [Ah 7c]\nGegnerEins: shows [Qs Qd]\nGegnerEins collected 1700 from pot",
        ),
    ]
}

fn model() -> ModelInfo {
    ModelInfo {
        id: "test/modell".into(),
        name: "Testmodell".into(),
        prompt_per_m: Some(2.0),
        completion_per_m: Some(10.0),
        context_length: Some(128_000),
        tools: true,
        reasoning: false,
    }
}

fn prepared() -> Prepared {
    prepare(&row(), hands(), "test/modell", Some(&model()), 40).unwrap()
}

fn tool_call(id: &str, name: &str, args: &str) -> Value {
    json!({"id": id, "type": "function", "function": {"name": name, "arguments": args}})
}

fn answer(calls: Vec<Value>, prompt: u64, completion: u64, cost: f64) -> Reply {
    ok(json!({
        "choices": [{"message": {"role": "assistant", "content": null, "tool_calls": calls}, "finish_reason": "tool_calls"}],
        "usage": {"prompt_tokens": prompt, "completion_tokens": completion, "cost": cost}
    }))
}

fn report_args(hand: &str, value: &str) -> String {
    json!({
        "summary": "Solide gespielt, ein teurer Call.",
        "spots": [{
            "hand": hand,
            "action": "Call am River",
            "verdict": "fehler",
            "better": "Fold",
            "why": "Du brauchst mehr Equity, als du hast.",
            "numbers": [
                {"label": "Benötigte Equity", "value": value, "source": "berechnet"},
                {"label": "Erfundene Zahl", "value": "37,3 %", "source": "berechnet"},
                {"label": "Gegner-Range", "value": "eng", "source": "einschätzung"}
            ],
            "concept": "pot_odds"
        }],
        "focus": ["Pot Odds am River"]
    })
    .to_string()
}

fn quiet(_: &Progress) {}

#[test]
fn prepare_anonymises_and_estimates() {
    let p = prepared();
    let e = p.estimate();
    assert_eq!(e.hands_total, 3);
    assert_eq!(e.hands_sent, 2);
    assert_eq!(e.hands_without_decision, 1);
    assert!(e.spots >= 1, "das All-in mit offenen Karten");
    assert!(e.cost_usd.unwrap() > 0.0 && e.max_cost_usd.unwrap() > e.cost_usd.unwrap());
    let sent = serde_json::to_string(p.messages()).unwrap();
    for secret in ["echterName", "GegnerEins", "Geheimer Turniername", "#77", "101", "102", "103", "Table"] {
        assert!(!sent.contains(secret), "{secret} würde gesendet");
    }
    assert!(sent.contains("Fakt von Ringside: All-in mit offenen Karten"), "{sent}");
}

#[test]
fn nothing_is_sent_without_the_confirmed_estimate() {
    let server = serve(vec![]);
    let p = prepared();
    assert!(matches!(p.confirm(""), Err(CoachError::NotConfirmed)));
    let p = prepared();
    assert!(matches!(p.confirm("0000000000000000"), Err(CoachError::NotConfirmed)));
    // `run` only takes a confirmed request, so these two never reached the network.
    std::thread::sleep(Duration::from_millis(100));
    assert!(server.requests.lock().unwrap().is_empty());
}

#[test]
fn full_loop_with_tool_errors_and_a_corrected_report() {
    let server = serve(vec![
        answer(
            vec![
                tool_call("c1", "pot_odds", r#"{"call": 9, "pot_before_call": 25.5}"#),
                tool_call("c2", "equity", "{kaputt"),
                tool_call("c3", "push_fold_nash", r#"{"stack_bb": 0.2, "hand": "AA"}"#),
            ],
            1000,
            200,
            0.01,
        ),
        answer(vec![tool_call("c4", "report", &report_args("H9", "26,1 %"))], 1500, 300, 0.02),
        answer(vec![tool_call("c5", "report", &report_args("h1", "26,09 %"))], 1800, 400, 0.03),
    ]);
    let client = Client::new(&server.url, Some("sk-or-test".into()));
    let token = prepared().estimate().token.clone();
    let p = prepared();
    assert!(p.confirm(&token).is_err(), "Token gehört zu einer anderen Vorbereitung");
    let p = prepared();
    let token = p.estimate().token.clone();
    let mut stages = Vec::new();
    let report =
        run(p.confirm(&token).unwrap(), &client, 1_700_000_000, &mut |p| stages.push(p.clone()), &|| false).unwrap();

    let requests = server.requests.lock().unwrap();
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[0]["model"], "test/modell");
    assert_eq!(requests[0]["tools"].as_array().unwrap().len(), 6);
    let tool_results: Vec<&Value> =
        requests[1]["messages"].as_array().unwrap().iter().filter(|m| m["role"] == "tool").collect();
    assert_eq!(tool_results.len(), 3);
    assert!(tool_results[0]["content"].as_str().unwrap().contains("26.09"));
    assert!(tool_results[1]["content"].as_str().unwrap().contains("kein gültiges JSON"));
    assert!(tool_results[2]["content"].as_str().unwrap().contains("stack_bb"));
    let fix = requests[2]["messages"].as_array().unwrap().last().unwrap()["content"].as_str().unwrap().to_string();
    assert!(fix.contains("H9"), "{fix}");

    assert_eq!(report.tournament_id, 77);
    assert_eq!(report.spots.len(), 1);
    let spot = &report.spots[0];
    assert_eq!(spot.hand_id, 102);
    assert_eq!(spot.verdict, Verdict::Fehler);
    assert_eq!(spot.hero_cards, vec!["Ah", "7c"]);
    assert_eq!(spot.board, vec!["2c", "3d", "9h", "Ks", "4d"]);
    assert_eq!(spot.numbers[0].source, Source::Berechnet);
    assert_eq!(spot.numbers[1].source, Source::Einschaetzung, "nicht berechnete Zahl wird herabgestuft");
    assert_eq!(report.meta.downgraded, 1);
    assert_eq!(report.meta.prompt_tokens, 4300);
    assert_eq!(report.meta.completion_tokens, 900);
    assert!((report.meta.cost_usd.unwrap() - 0.06).abs() < 1e-9);
    assert_eq!(report.meta.steps, 3);
    assert_eq!(report.meta.tool_calls, 5);
    assert!(stages.iter().any(|s| s.tool.as_deref() == Some("pot_odds")));
}

#[test]
fn retries_rate_limits_and_server_errors() {
    let server = serve(vec![
        Reply { status: 429, headers: "Retry-After: 0\r\n", body: "{}".into(), delay: Duration::ZERO },
        Reply {
            status: 200,
            headers: "",
            body: json!({"error": {"code": 502, "message": "upstream"}}).to_string(),
            delay: Duration::ZERO,
        },
        answer(vec![tool_call("c1", "report", &report_args("H1", "1"))], 10, 10, 0.0),
    ]);
    let client = Client::new(&server.url, Some("k".into()));
    let p = prepared();
    let token = p.estimate().token.clone();
    let started = Instant::now();
    let report = run(p.confirm(&token).unwrap(), &client, 0, &mut quiet, &|| false).unwrap();
    assert_eq!(report.spots.len(), 1);
    assert_eq!(server.requests.lock().unwrap().len(), 3);
    assert!(started.elapsed() < Duration::from_secs(10));
}

#[test]
fn model_without_tools_is_refused() {
    let server = serve(vec![Reply {
        status: 404,
        headers: "",
        body: json!({"error": {"message": "No endpoints found that support tool use."}}).to_string(),
        delay: Duration::ZERO,
    }]);
    let client = Client::new(&server.url, Some("k".into()));
    let p = prepared();
    let token = p.estimate().token.clone();
    let e = run(p.confirm(&token).unwrap(), &client, 0, &mut quiet, &|| false).unwrap_err();
    assert_eq!(e, CoachError::Api(ApiError::NoToolSupport));

    let mut m = model();
    m.tools = false;
    assert!(prepare(&row(), hands(), "x", Some(&m), 40).is_err(), "schon vor dem Senden abgelehnt");
}

#[test]
fn a_model_that_never_reports_stops() {
    let text =
        || ok(json!({"choices": [{"message": {"role": "assistant", "content": "Hier meine Gedanken."}}], "usage": {}}));
    let server = serve(vec![text(), text()]);
    let client = Client::new(&server.url, Some("k".into()));
    let p = prepared();
    let token = p.estimate().token.clone();
    let e = run(p.confirm(&token).unwrap(), &client, 0, &mut quiet, &|| false).unwrap_err();
    assert_eq!(e, CoachError::NoReport);
    let requests = server.requests.lock().unwrap();
    let nudge = requests[1]["messages"].as_array().unwrap().last().unwrap().clone();
    assert_eq!(nudge["role"], "user");
}

#[test]
fn stops_at_the_confirmed_cost_limit() {
    let server =
        serve(vec![answer(vec![tool_call("c1", "pot_odds", r#"{"call": 1, "pot_before_call": 2}"#)], 10, 10, 50.0)]);
    let client = Client::new(&server.url, Some("k".into()));
    let p = prepared();
    let token = p.estimate().token.clone();
    let e = run(p.confirm(&token).unwrap(), &client, 0, &mut quiet, &|| false).unwrap_err();
    assert!(matches!(e, CoachError::CostLimit { .. }), "{e:?}");
}

#[test]
fn cancel_takes_effect_while_waiting() {
    let server = serve(vec![Reply { status: 200, headers: "", body: "{}".into(), delay: Duration::from_secs(20) }]);
    let client = Client::new(&server.url, Some("k".into()));
    let p = prepared();
    let token = p.estimate().token.clone();
    let cancel = Arc::new(AtomicBool::new(false));
    let flag = cancel.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(300));
        flag.store(true, Ordering::Relaxed);
    });
    let started = Instant::now();
    let e = run(p.confirm(&token).unwrap(), &client, 0, &mut quiet, &|| cancel.load(Ordering::Relaxed)).unwrap_err();
    assert_eq!(e, CoachError::Api(ApiError::Cancelled));
    assert!(started.elapsed() < Duration::from_secs(3));
}
