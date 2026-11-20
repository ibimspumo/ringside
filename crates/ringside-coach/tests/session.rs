//! The tool-calling loop against a local fake OpenRouter.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ringside_coach::openrouter::ModelInfo;
use ringside_coach::report::{Source, Verdict};
use ringside_coach::session::Stage;
use ringside_coach::{ApiError, Client, CoachError, Effort, Prepared, Progress, prepare, run};
use ringside_core::model::Hand;
use ringside_core::tournament::{BuyIn, TournamentRow};
use serde_json::{Value, json};

struct Reply {
    status: u16,
    headers: &'static str,
    body: String,
    delay: Duration,
    /// Server-sent events, each written after its pause; replaces `body`.
    events: Vec<(Duration, String)>,
}

fn ok(body: Value) -> Reply {
    Reply { status: 200, headers: "", body: body.to_string(), delay: Duration::ZERO, events: vec![] }
}

fn status(status: u16, body: Value) -> Reply {
    Reply { status, headers: "", body: body.to_string(), delay: Duration::ZERO, events: vec![] }
}

fn stream(events: Vec<(Duration, String)>) -> Reply {
    Reply { status: 200, headers: "", body: String::new(), delay: Duration::ZERO, events }
}

/// Serves the scripted replies in the order the requests arrive, records every request
/// body and when the client hung up on a stream.
struct FakeServer {
    url: String,
    requests: Arc<Mutex<Vec<Value>>>,
    hung_up: Arc<Mutex<Option<Instant>>>,
}

fn answer_on(mut stream: TcpStream, r: Reply, hung_up: &Mutex<Option<Instant>>) {
    std::thread::sleep(r.delay);
    if r.events.is_empty() {
        let _ = write!(
            stream,
            "HTTP/1.1 {} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{}\r\n{}",
            r.status,
            r.body.len(),
            r.headers,
            r.body
        );
        return;
    }
    let _ = write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n"
    );
    for (pause, event) in r.events {
        std::thread::sleep(pause);
        let sent = write!(stream, "{:x}\r\n{event}\r\n", event.len()).and_then(|_| stream.flush());
        if sent.is_err() {
            *hung_up.lock().unwrap() = Some(Instant::now());
            return;
        }
    }
    let _ = write!(stream, "0\r\n\r\n");
}

fn serve(replies: Vec<Reply>) -> FakeServer {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/api/v1", listener.local_addr().unwrap());
    let requests = Arc::new(Mutex::new(Vec::new()));
    let hung_up = Arc::new(Mutex::new(None));
    let (seen, gone) = (requests.clone(), hung_up.clone());
    let replies = Arc::new(Mutex::new(replies.into_iter()));
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { return };
            let (seen, gone, replies) = (seen.clone(), gone.clone(), replies.clone());
            std::thread::spawn(move || {
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
                let reply = {
                    let mut seen = seen.lock().unwrap();
                    seen.push(serde_json::from_slice(&body).unwrap_or(Value::Null));
                    replies.lock().unwrap().next()
                };
                if let Some(r) = reply {
                    answer_on(stream, r, &gone);
                }
            });
        }
    });
    FakeServer { url, requests, hung_up }
}

const MS: fn(u64) -> Duration = Duration::from_millis;

fn data(v: Value) -> String {
    format!("data: {v}\n\n")
}

fn keep_alive() -> String {
    ": OPENROUTER PROCESSING\n\n".into()
}

fn delta(d: Value) -> String {
    data(json!({"choices": [{"index": 0, "delta": d, "finish_reason": null}]}))
}

/// A streamed answer: some reasoning, then the tool calls with their arguments in
/// pieces, keep-alive comments in between, usage at the end.
fn streamed_answer(calls: &[(&str, &str, &str)], prompt: u64, completion: u64, cost: f64) -> Vec<(Duration, String)> {
    let mut ev = vec![(MS(0), keep_alive()), (MS(20), keep_alive())];
    for piece in ["**Pot Odds**\n\nIch prüfe ", "zuerst den River-Call."] {
        ev.push((
            MS(5),
            delta(json!({"role": "assistant", "reasoning": piece, "reasoning_details": [
                {"type": "reasoning.summary", "summary": piece, "format": "openai-responses-v1", "index": 0}
            ]})),
        ));
    }
    ev.push((
        MS(5),
        delta(json!({"reasoning_details": [{"type": "reasoning.encrypted", "data": "geheim", "index": 1}]})),
    ));
    for (i, (id, name, args)) in calls.iter().enumerate() {
        ev.push((
            MS(5),
            delta(json!({"tool_calls": [{"index": i, "id": id, "type": "function", "function": {"name": name, "arguments": ""}}]})),
        ));
        let cut = [0, args.len() / 3, 2 * args.len() / 3, args.len()];
        for w in cut.windows(2) {
            ev.push((MS(5), delta(json!({"tool_calls": [{"index": i, "function": {"arguments": &args[w[0]..w[1]]}}]}))));
            ev.push((MS(0), keep_alive()));
        }
    }
    let finish = json!({"index": 0, "delta": {"content": ""}, "finish_reason": "tool_calls"});
    ev.push((MS(5), data(json!({"choices": [finish]}))));
    ev.push((
        MS(5),
        data(json!({"choices": [finish], "usage": {"prompt_tokens": prompt, "completion_tokens": completion,
            "completion_tokens_details": {"reasoning_tokens": completion / 2}, "cost": cost}})),
    ));
    ev.push((MS(0), "data: [DONE]\n\n".into()));
    ev
}

fn row() -> TournamentRow {
    TournamentRow {
        id: 77,
        site: Default::default(),
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
        payouts: Vec::new(),
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
        reasoning: true,
    }
}

fn prepared() -> Prepared {
    prepare(&row(), hands(), "test/modell", Some(&model()), 40, Effort::High).unwrap()
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
                tool_call("c3", "push_fold_nash", r#"{"stacks_bb": [0.2, 10], "position": "SB", "hand": "AA"}"#),
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
    assert!(tool_results[2]["content"].as_str().unwrap().contains("Stack muss Ante und Blind decken"));
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
        Reply { headers: "Retry-After: 0\r\n", ..status(429, json!({})) },
        ok(json!({"error": {"code": 502, "message": "upstream"}})),
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
    let server = serve(vec![status(404, json!({"error": {"message": "No endpoints found that support tool use."}}))]);
    let client = Client::new(&server.url, Some("k".into()));
    let p = prepared();
    let token = p.estimate().token.clone();
    let e = run(p.confirm(&token).unwrap(), &client, 0, &mut quiet, &|| false).unwrap_err();
    assert_eq!(e, CoachError::Api(ApiError::NoToolSupport));

    let mut m = model();
    m.tools = false;
    assert!(prepare(&row(), hands(), "x", Some(&m), 40, Effort::High).is_err(), "schon vor dem Senden abgelehnt");
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
    let server = serve(vec![Reply { delay: Duration::from_secs(20), ..ok(json!({})) }]);
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

fn confirmed_run(client: &Client, progress: &mut dyn FnMut(&Progress), cancel: &dyn Fn() -> bool) -> Result<ringside_coach::Report, CoachError> {
    let p = prepared();
    let token = p.estimate().token.clone();
    run(p.confirm(&token).unwrap(), client, 0, progress, cancel)
}

#[test]
fn streamed_tool_calls_are_put_together() {
    let report = report_args("H2", "26,09 %");
    let server = serve(vec![
        stream(streamed_answer(
            &[("c1", "pot_odds", r#"{"call": 9, "pot_before_call": 25.5}"#), ("c2", "outs", r#"{"hero": ["Ah", "7c"]}"#)],
            1000,
            400,
            0.01,
        )),
        stream(streamed_answer(&[("c3", "report", &report)], 1500, 600, 0.02)),
    ]);
    let client = Client::new(&server.url, Some("k".into()));
    let mut seen = Vec::new();
    let r = confirmed_run(&client, &mut |p| seen.push(p.clone()), &|| false).unwrap();

    let requests = server.requests.lock().unwrap();
    assert_eq!(requests[0]["stream"], true);
    assert_eq!(requests[0]["reasoning"]["effort"], "high");
    let messages = requests[1]["messages"].as_array().unwrap();
    let assistant = messages.iter().find(|m| m["role"] == "assistant").unwrap();
    let calls = assistant["tool_calls"].as_array().unwrap();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0]["id"], "c1");
    assert_eq!(calls[0]["function"]["arguments"], r#"{"call": 9, "pot_before_call": 25.5}"#);
    assert_eq!(calls[1]["function"]["name"], "outs");
    let details = assistant["reasoning_details"].as_array().unwrap();
    assert_eq!(details.len(), 2, "Zusammenfassung in einem Stück, dazu der verschlüsselte Block");
    assert_eq!(details[0]["summary"], "**Pot Odds**\n\nIch prüfe zuerst den River-Call.");
    assert_eq!(details[1]["data"], "geheim");
    let results: Vec<&Value> = messages.iter().filter(|m| m["role"] == "tool").collect();
    assert!(results[0]["content"].as_str().unwrap().contains("26.09"));

    assert_eq!(r.spots.len(), 1);
    assert_eq!((r.meta.prompt_tokens, r.meta.completion_tokens), (2500, 1000));
    assert!((r.meta.cost_usd.unwrap() - 0.03).abs() < 1e-9);
    let last = seen.last().unwrap();
    assert_eq!(last.reasoning_tokens, 200, "abgerechnet nach dem ersten Schritt");
    assert_eq!(seen[0].stage, Stage::Thinking);
    assert!(seen.iter().any(|p| p.live_reasoning_tokens > 0));
    assert!(seen.iter().any(|p| p.stage == Stage::Calling && p.tool.as_deref() == Some("pot_odds")));
    assert!(seen.iter().any(|p| p.stage == Stage::Calling && p.message.contains("Bericht")));
    assert!(seen.iter().all(|p| p.idle_limit == Some(120)));
}

#[test]
fn keep_alive_comments_carry_a_long_step() {
    let mut events: Vec<_> = (0..12).map(|_| (MS(100), keep_alive())).collect();
    events.extend(streamed_answer(&[("c1", "report", &report_args("H2", "1"))], 10, 10, 0.0));
    let server = serve(vec![stream(events)]);
    // 1.2 s of nothing but comments, with an idle limit of 0.5 s.
    let client = Client::with_idle_limit(&server.url, Some("k".into()), MS(500));
    let mut seen = Vec::new();
    confirmed_run(&client, &mut |p| seen.push(p.clone()), &|| false).unwrap();
    let longest = seen.iter().map(|p| p.step_seconds).fold(0.0, f64::max);
    assert!(longest >= 1.1, "{longest}");
    assert!(seen.iter().all(|p| p.idle_seconds < 0.5));
    assert!(seen.len() >= 3, "Lebenszeichen während des Wartens");
}

#[test]
fn a_silent_stream_fails_after_the_idle_limit() {
    let server = serve(vec![
        stream(vec![(MS(0), keep_alive()), (Duration::from_secs(5), keep_alive())]),
        stream(streamed_answer(&[("c1", "report", &report_args("H2", "1"))], 10, 10, 0.0)),
    ]);
    let client = Client::with_idle_limit(&server.url, Some("k".into()), MS(300));
    let mut seen = Vec::new();
    let started = Instant::now();
    confirmed_run(&client, &mut |p| seen.push(p.clone()), &|| false).unwrap();
    assert!(started.elapsed() < Duration::from_secs(4), "{:?}", started.elapsed());
    let retry = seen.iter().find(|p| p.stage == Stage::Retry).expect("neuer Versuch");
    assert!(retry.message.contains("keine Daten"), "{}", retry.message);
    assert_eq!(server.requests.lock().unwrap().len(), 2);
}

#[test]
fn cancel_mid_stream_closes_the_connection() {
    let mut events = streamed_answer(&[], 0, 0, 0.0);
    events.truncate(5);
    events.extend((0..200).map(|_| (MS(50), keep_alive())));
    let server = serve(vec![stream(events)]);
    let client = Client::new(&server.url, Some("k".into()));
    let cancel = Arc::new(AtomicBool::new(false));
    let flag = cancel.clone();
    std::thread::spawn(move || {
        std::thread::sleep(MS(400));
        flag.store(true, Ordering::Relaxed);
    });
    let started = Instant::now();
    let e = confirmed_run(&client, &mut quiet, &|| cancel.load(Ordering::Relaxed)).unwrap_err();
    let cancelled_after = started.elapsed();
    assert_eq!(e, CoachError::Api(ApiError::Cancelled));
    assert!(cancelled_after < Duration::from_secs(1), "{cancelled_after:?}");
    // The stream would run 10 s; a closed connection shows well before, even on a loaded runner.
    let until = started + Duration::from_secs(8);
    while server.hung_up.lock().unwrap().is_none() && Instant::now() < until {
        std::thread::sleep(MS(20));
    }
    let hung_up = server.hung_up.lock().unwrap().expect("Verbindung bleibt offen");
    assert!(hung_up.duration_since(started) < Duration::from_secs(8));
}

#[test]
fn falls_back_without_streaming() {
    let server = serve(vec![
        status(400, json!({"error": {"code": 400, "message": "Streaming is not supported for this model"}})),
        answer(vec![tool_call("c1", "pot_odds", r#"{"call": 1, "pot_before_call": 2}"#)], 10, 10, 0.0),
        answer(vec![tool_call("c2", "report", &report_args("H2", "1"))], 10, 10, 0.0),
    ]);
    let client = Client::new(&server.url, Some("k".into()));
    let mut seen = Vec::new();
    confirmed_run(&client, &mut |p| seen.push(p.clone()), &|| false).unwrap();
    let requests = server.requests.lock().unwrap();
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[0]["stream"], true);
    assert!(requests[1]["stream"].is_null() && requests[2]["stream"].is_null(), "bleibt beim ganzen Stück");
    assert!(seen.iter().any(|p| p.message.contains("nicht streamen")));
    assert_eq!(seen.last().unwrap().idle_limit, None);
}

#[test]
fn the_stream_stops_at_the_cost_limit() {
    let limit = prepared().estimate().max_cost_usd.unwrap();
    // Visible reasoning worth twice the limit at 10 $ per million output tokens.
    let chars = (limit * 2.0 / 10e-6 * 2.8) as usize;
    let piece = "x".repeat(4000);
    let mut events = vec![(MS(0), keep_alive())];
    events.extend((0..chars / 4000 + 1).map(|_| (MS(1), delta(json!({"reasoning": piece})))));
    events.extend((0..100).map(|_| (MS(50), keep_alive())));
    let server = serve(vec![stream(events)]);
    let client = Client::new(&server.url, Some("k".into()));
    let e = confirmed_run(&client, &mut quiet, &|| false).unwrap_err();
    let CoachError::CostLimit { spent, limit: l } = e else { panic!("{e:?}") };
    assert!(spent > l && (l - limit).abs() < 1e-12);
    assert!(spent < limit * 1.2, "bricht bald nach der Grenze ab: {spent}");
    assert!(server.requests.lock().unwrap()[0]["max_tokens"].as_u64().unwrap() <= 16_000);
    let until = Instant::now() + Duration::from_secs(2);
    while server.hung_up.lock().unwrap().is_none() && Instant::now() < until {
        std::thread::sleep(MS(20));
    }
    assert!(server.hung_up.lock().unwrap().is_some(), "Verbindung bleibt offen");
}

#[test]
fn no_step_is_sent_that_the_limit_cannot_pay() {
    let limit = prepared().estimate().max_cost_usd.unwrap();
    let server = serve(vec![answer(
        vec![tool_call("c1", "pot_odds", r#"{"call": 1, "pot_before_call": 2}"#)],
        10,
        10,
        limit * 0.98,
    )]);
    let client = Client::new(&server.url, Some("k".into()));
    let e = confirmed_run(&client, &mut quiet, &|| false).unwrap_err();
    assert!(matches!(e, CoachError::CostLimit { .. }), "{e:?}");
    assert_eq!(server.requests.lock().unwrap().len(), 1, "der zweite Schritt geht nicht mehr raus");
}

#[test]
fn thoroughness_lowers_the_estimate_and_is_sent() {
    let high = prepared().estimate().cost_usd.unwrap();
    let low = prepare(&row(), hands(), "test/modell", Some(&model()), 40, Effort::Low).unwrap();
    assert_eq!(low.estimate().effort, Some(Effort::Low));
    assert!(low.estimate().cost_usd.unwrap() < high);
    let mut plain = model();
    plain.reasoning = false;
    let none = prepare(&row(), hands(), "test/modell", Some(&plain), 40, Effort::Low).unwrap();
    assert_eq!(none.estimate().effort, None);

    let server = serve(vec![answer(vec![tool_call("c1", "report", &report_args("H2", "1"))], 10, 10, 0.0)]);
    let client = Client::new(&server.url, Some("k".into()));
    let token = low.estimate().token.clone();
    run(low.confirm(&token).unwrap(), &client, 0, &mut quiet, &|| false).unwrap();
    let token = none.estimate().token.clone();
    let server2 = serve(vec![answer(vec![tool_call("c1", "report", &report_args("H2", "1"))], 10, 10, 0.0)]);
    let client2 = Client::new(&server2.url, Some("k".into()));
    run(none.confirm(&token).unwrap(), &client2, 0, &mut quiet, &|| false).unwrap();
    assert_eq!(server.requests.lock().unwrap()[0]["reasoning"]["effort"], "low");
    assert!(server2.requests.lock().unwrap()[0]["reasoning"].is_null());
}

#[test]
fn a_question_is_one_request_without_calculators_and_flags_new_figures() {
    let server = serve(vec![ok(json!({
        "choices": [{"message": {"role": "assistant", "content": "Ein Fold, weil du 4,2 BB verlierst. Mit 57 % Equity wäre es anders."}}],
        "usage": {"prompt_tokens": 300, "completion_tokens": 40, "cost": 0.0012}
    }))]);
    let client = Client::new(&server.url, Some("k".into()));
    let prepared = ringside_coach::prepare_ask("Call kostet 4,2 BB.", "Warum ist das ein Fold?", "m", None).unwrap();
    let token = prepared.estimate().token.clone();
    let answer = ringside_coach::ask(prepared.confirm(&token).unwrap(), &client, &|| false).unwrap();
    assert_eq!(answer.unchecked, vec!["57"]);
    assert!((answer.cost_usd - 0.0012).abs() < 1e-12);
    let sent = server.requests.lock().unwrap().clone();
    assert_eq!(sent.len(), 1);
    assert!(sent[0].get("tools").is_none(), "the coach gets no calculators to answer a question");
    assert_eq!(sent[0]["reasoning"]["effort"], "low");
}
