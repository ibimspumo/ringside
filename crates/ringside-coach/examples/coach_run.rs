//! Runs the coach on one hand history file outside the app, for testing against the real
//! OpenRouter. The key comes from `OPENROUTER_API_KEY`, the model from `OPENROUTER_MODEL`.
//!
//! Without `--senden` it only prints the estimate and the anonymised request.
//!
//! cargo run -p ringside-coach --example coach_run -- <hand-history.txt> <out-dir> [--senden]

use std::time::{SystemTime, UNIX_EPOCH};

use ringside_coach::openrouter::{BASE_URL, DEFAULT_MODEL};
use ringside_coach::{Client, prepare, run};
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
        name: None,
    };
    let models = Client::new(BASE_URL, None).models()?;
    let info = models.iter().find(|m| m.id == model);
    let prepared = prepare(&row, hands, &model, info, ringside_coach::session::MAX_HANDS)?;
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
    let mut log = Vec::new();
    let result = run(
        confirmed,
        &client,
        now,
        &mut |p| {
            eprintln!(
                "Schritt {}/{} · {} · {} Tokens · {:.4} $",
                p.step,
                p.max_steps,
                p.message,
                p.prompt_tokens + p.completion_tokens,
                p.cost_usd
            );
            log.push(p.clone());
        },
        &|| false,
    );
    let outcome = match &result {
        Ok(report) => json!({"estimate": estimate, "report": report, "progress": log}),
        Err(e) => json!({"estimate": estimate, "error": e.to_string(), "progress": log}),
    };
    std::fs::write(format!("{out}/result.json"), serde_json::to_string_pretty(&outcome)?)?;
    result?;
    Ok(())
}
