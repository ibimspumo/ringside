//! Runs the coach on one hand history file outside the app, for testing against the real
//! OpenRouter. The key comes from `OPENROUTER_API_KEY`, the model from `OPENROUTER_MODEL`,
//! the thoroughness from `OPENROUTER_EFFORT` (high, medium, low).
//!
//! Without `--senden` it only prints the estimate and the anonymised request.
//!
//! cargo run -p ringside-coach --example coach_run -- <hand-history.txt> <out-dir> [--senden]

use std::time::{SystemTime, UNIX_EPOCH};

use ringside_coach::openrouter::{BASE_URL, DEFAULT_MODEL};
use ringside_coach::{Client, Effort, prepare, run};
use ringside_core::tournament::TournamentRow;
use ringside_core::{parse_hand, split_hands};
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let (Some(path), Some(out)) = (args.get(1), args.get(2)) else {
        return Err("Aufruf: coach_run <hand-history.txt> <ausgabe-ordner> [--senden]".into());
    };
    let send = args.iter().any(|a| a == "--senden");
    let model = std::env::var("OPENROUTER_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.into());
    let effort: Effort = match std::env::var("OPENROUTER_EFFORT") {
        Ok(e) => serde_json::from_value(json!(e)).map_err(|_| "OPENROUTER_EFFORT: high, medium oder low")?,
        Err(_) => Effort::default(),
    };
    let text = std::fs::read_to_string(path)?;
    let hands: Vec<_> = split_hands(&text).into_iter().filter_map(|h| parse_hand(h).ok()).collect();
    let first = hands.first().ok_or("keine Hände")?;
    let hero = first.hero.clone().unwrap_or_default();
    let finish = hands.iter().flat_map(|h| &h.finishes).find(|f| f.player == hero);
    let row = TournamentRow {
        id: first.tournament_id.unwrap_or(0),
        game: first.variant.clone(),
        currency: first.buy_in.as_ref().and_then(|b| b.currency.clone()),
        buy_in: first.buy_in.clone(),
        cost: None,
        entrants: None,
        place: finish.and_then(|f| f.place),
        prize: finish.map(|f| f.prize.unwrap_or(0)),
        bounties: 0,
        profit: None,
        started_at: first.played_at.clone(),
        hands: hands.len() as i64,
        chips: 0,
        has_summary: false,
        payouts: Vec::new(),
        name: None,
    };
    let models = Client::new(BASE_URL, None).models()?;
    let info = models.iter().find(|m| m.id == model);
    let prepared = prepare(&row, hands, &model, info, ringside_coach::session::MAX_HANDS, effort)?;
    std::fs::create_dir_all(out)?;
    std::fs::write(format!("{out}/request.json"), serde_json::to_string_pretty(prepared.messages())?)?;
    let estimate = prepared.estimate().clone();
    eprintln!("{}", serde_json::to_string_pretty(&estimate)?);
    if !send {
        eprintln!("Nur geschätzt. Mit --senden wird die Analyse wirklich gesendet.");
        return Ok(());
    }
    let key = std::env::var("OPENROUTER_API_KEY").map_err(|_| "OPENROUTER_API_KEY fehlt")?;
    let client = Client::new(BASE_URL, Some(key));
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
    let confirmed = prepared.confirm(&estimate.token)?;
    let mut log: Vec<ringside_coach::Progress> = Vec::new();
    let mut longest_idle = 0.0f64;
    let result = run(
        confirmed,
        &client,
        now,
        &mut |p| {
            longest_idle = longest_idle.max(p.idle_seconds);
            if log.last().is_none_or(|l| l.message != p.message || l.step != p.step) {
                eprintln!(
                    "Schritt {}/{} · {:.0} s (gesamt {:.0} s) · {} · live {}+{} Tokens · {} Tokens · {:.4} $",
                    p.step,
                    p.max_steps,
                    p.step_seconds,
                    p.total_seconds,
                    p.message,
                    p.live_reasoning_tokens,
                    p.live_output_tokens,
                    p.prompt_tokens + p.completion_tokens,
                    p.cost_usd
                );
            }
            log.push(p.clone());
        },
        &|| false,
    );
    let mut steps: Vec<f64> = Vec::new();
    for p in &log {
        if steps.len() < p.step {
            steps.resize(p.step, 0.0);
        }
        steps[p.step - 1] = steps[p.step - 1].max(p.step_seconds);
    }
    let timing = json!({"step_seconds": steps, "longest_idle_seconds": longest_idle, "events": log.len()});
    eprintln!("{timing}");
    let outcome = match &result {
        Ok(report) => json!({"estimate": estimate, "timing": timing, "report": report}),
        Err(e) => json!({"estimate": estimate, "timing": timing, "error": e.to_string()}),
    };
    std::fs::write(format!("{out}/result.json"), serde_json::to_string_pretty(&outcome)?)?;
    result?;
    Ok(())
}
