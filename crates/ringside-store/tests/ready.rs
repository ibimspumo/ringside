//! How often the practice stock gets a spot that is solved already, measured against Ringside
//! Cloud with read requests only (no download, no upload):
//! `cargo test --release -p ringside-store --test ready -- --ignored --nocapture`.
//!
//! For each heads-up flop of a practice table the stock picks the line by how often the
//! model plays it, then a ready flop of that line if there is one. "Bisher" counts the
//! lines with a ready spot on one of the library's flops (what the stock looked at before
//! suit classes); "jetzt" the lines with a ready spot on any flop class
//! (`ringside_store::key::line_flops`), each dealt in all its suit renamings. The cloud's
//! model flops come from `/v1/solutions/flops` where the server has it, else from status
//! requests for every flop class.

use std::time::Duration;

use ringside_cloud::client::{DEFAULT_BASE_URL, KeyStatus, MAX_STATUS_KEYS, RetryPolicy};
use ringside_cloud::kinds::SpotInput;
use ringside_cloud::Client;
use ringside_engine::preflop_table::{Family, ModelLine};
use ringside_engine::pushfold_table::Ante;
use ringside_play::TableKey;
use ringside_play::pool::{parse_flop, renamings};
use ringside_solver::library::catalog;
use ringside_store::key::{catalog_key, line_flops};

/// The server allows 20 status requests a minute per client, and as many flop lists.
const GAP: Duration = Duration::from_millis(3200);
/// Lines covering this share of the table's flops are asked about; the rest is counted as
/// not ready.
const COVER: f64 = 0.95;

fn served(client: &Client, keys: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for chunk in keys.chunks(MAX_STATUS_KEYS) {
        std::thread::sleep(GAP);
        let statuses = client.statuses(chunk).expect("status request");
        out.extend(statuses.into_iter().filter(|(_, s)| matches!(s, KeyStatus::Accepted | KeyStatus::Verified)).map(|(k, _)| k));
    }
    out
}

#[test]
#[ignore = "asks Ringside Cloud"]
fn practice_gets_ready_spots() {
    let client = Client::with_policy(DEFAULT_BASE_URL, RetryPolicy { attempts: 2, timeout: Duration::from_secs(20), ..RetryPolicy::default() });
    let c = catalog();
    // Control: the library's spots of its first scenario, which packs and the cloud serve.
    let control: Vec<String> = c.ids().take(MAX_STATUS_KEYS).map(|id| catalog_key(id).split_once(':').unwrap().1.to_string()).collect();
    let n = served(&client, &control).len();
    println!("Kontrolle: {n} von {} Katalog-Spots ({}) in der Cloud ausgeliefert", control.len(), c.scenarios[0].id);
    for table in [
        TableKey { players: 6, ante: Ante::BigBlind, stack_bb: 20.0 },
        TableKey { players: 6, ante: Ante::BigBlind, stack_bb: 40.0 },
        TableKey { players: 9, ante: Ante::BigBlind, stack_bb: 20.0 },
        TableKey { players: 9, ante: Ante::BigBlind, stack_bb: 40.0 },
    ] {
        let deep = ringside_play::table::deep(table).expect("a plain table");
        let mut lines: Vec<_> = deep.scenarios.iter().collect();
        lines.sort_by(|a, b| b.reach.total_cmp(&a.reach));
        let total: f64 = lines.iter().map(|s| s.reach).sum();
        let (mut covered, mut before, mut after, mut classes, mut boards, mut endpoint) = (0.0, 0.0, 0.0, 0usize, 0usize, false);
        for sc in lines {
            if covered / total >= COVER {
                break;
            }
            covered += sc.reach;
            let model = ModelLine { family: Family::Plain, ante: table.ante, players: table.players, stack_bb: table.stack_bb, line: sc.line.clone() };
            let spot = sc.spot(&c.flops[0]);
            let flops = line_flops(&spot, Some(&model));
            let listed = SpotInput::from_model(&model, &spot).ok().and_then(|input| {
                std::thread::sleep(GAP);
                client.served_flops(&input).expect("flops request")
            });
            endpoint |= listed.is_some();
            let ask: Vec<&(String, String)> = flops.iter().filter(|(k, _)| k.starts_with("postflop:") || (listed.is_none() && k.starts_with("spot:"))).collect();
            let hashes: Vec<String> = ask.iter().map(|(k, _)| k.split_once(':').unwrap().1.to_string()).collect();
            let hits = served(&client, &hashes);
            let mut ready: Vec<String> = listed.unwrap_or_default();
            for (k, f) in ask {
                if hits.iter().any(|h| k.ends_with(h.as_str())) && !ready.contains(f) {
                    ready.push(f.clone());
                }
            }
            if ready.iter().any(|f| c.flop(f).is_some()) {
                before += sc.reach;
            }
            if !ready.is_empty() {
                after += sc.reach;
            }
            classes += ready.len();
            boards += ready.iter().filter_map(|f| parse_flop(f)).map(|f| renamings(f).len()).sum::<usize>();
        }
        println!(
            "{} Spieler, {} BB: jetzt {:.0} % der Stock-Spots fertig (bisher {:.0} %), {} fertige Flop-Klassen = {} Boards, Flop-Liste {}",
            table.players,
            table.stack_bb,
            100.0 * after / total,
            100.0 * before / total,
            classes,
            boards,
            if endpoint { "vom Server" } else { "aus Status-Abfragen (Endpunkt noch nicht da)" }
        );
    }
}

/// What the cloud has of the library, per catalog scenario.
#[test]
#[ignore = "asks Ringside Cloud"]
fn library_in_the_cloud() {
    let client = Client::with_policy(DEFAULT_BASE_URL, RetryPolicy { attempts: 2, timeout: Duration::from_secs(20), ..RetryPolicy::default() });
    let c = catalog();
    for (scenario, s) in c.scenarios.iter().enumerate() {
        let keys: Vec<String> = (0..c.flops.len())
            .map(|flop| catalog_key(ringside_solver::library::SpotId { scenario, flop }).split_once(':').unwrap().1.to_string())
            .collect();
        println!("{}: {} von {}", s.id, served(&client, &keys).len(), keys.len());
    }
}
