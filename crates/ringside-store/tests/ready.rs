//! How often the practice stock gets a spot that is solved already, measured against Ringside
//! Cloud with status requests only (no download, no upload):
//! `cargo test --release -p ringside-store --test ready -- --ignored --nocapture`.
//!
//! For each heads-up flop of a practice table the stock picks the line by how often the
//! model plays it, then a flop. It is served without a solve when the line has a verified
//! spot on one of the library's flops. Before the store only lines that are exactly a
//! catalog scenario could be served from the library.

use std::time::Duration;

use ringside_cloud::client::{DEFAULT_BASE_URL, KeyStatus, MAX_STATUS_KEYS, RetryPolicy};
use ringside_cloud::kinds::SpotInput;
use ringside_cloud::Client;
use ringside_engine::preflop_table::{Family, ModelLine};
use ringside_engine::pushfold_table::Ante;
use ringside_play::TableKey;
use ringside_solver::library::catalog;
use ringside_store::key::{catalog_id, catalog_key, on_flop, spot_kind_key};

/// The server allows 20 status requests a minute per client.
const GAP: Duration = Duration::from_millis(3200);
/// Lines covering this share of the table's flops are asked about; the rest is counted as
/// not ready.
const COVER: f64 = 0.95;

#[test]
#[ignore = "asks Ringside Cloud"]
fn practice_gets_ready_spots() {
    let client = Client::with_policy(DEFAULT_BASE_URL, RetryPolicy { attempts: 2, timeout: Duration::from_secs(20), ..RetryPolicy::default() });
    let c = catalog();
    // Control: the library's spots of its first scenario, which packs and the cloud serve.
    let control: Vec<String> = c.ids().take(MAX_STATUS_KEYS).map(|id| catalog_key(id).split_once(':').unwrap().1.to_string()).collect();
    let served = client.statuses(&control).expect("status request").iter().filter(|(_, s)| *s == KeyStatus::Verified).count();
    println!("Kontrolle: {served} von {} Katalog-Spots ({}) geprüft in der Cloud", control.len(), c.scenarios[0].id);
    for table in [
        TableKey { players: 6, ante: Ante::BigBlind, stack_bb: 40.0 },
        TableKey { players: 9, ante: Ante::BigBlind, stack_bb: 40.0 },
        TableKey { players: 9, ante: Ante::BigBlind, stack_bb: 20.0 },
    ] {
        let deep = ringside_play::table::deep(table).expect("a plain table");
        let mut lines: Vec<_> = deep.scenarios.iter().collect();
        lines.sort_by(|a, b| b.reach.total_cmp(&a.reach));
        let total: f64 = lines.iter().map(|s| s.reach).sum();
        let (mut covered, mut before, mut after, mut ready_flops, mut asked, mut catalog_lines) = (0.0, 0.0, 0.0, 0usize, 0usize, 0usize);
        for sc in lines {
            if covered / total >= COVER {
                break;
            }
            covered += sc.reach;
            let model = ModelLine { family: Family::Plain, ante: table.ante, players: table.players, stack_bb: table.stack_bb, line: sc.line.clone() };
            let base = SpotInput::from_model(&model, &sc.spot(&c.flops[0])).ok();
            let mut catalog_line = false;
            let keys: Vec<String> = c
                .flops
                .iter()
                .filter_map(|f| match catalog_id(&sc.spot(f)) {
                    Some(id) => {
                        catalog_line = true;
                        Some(catalog_key(id))
                    }
                    None => spot_kind_key(&on_flop(base.as_ref()?, f).ok()?).ok(),
                })
                .map(|k| k.split_once(':').map_or(k.clone(), |(_, h)| h.to_string()))
                .collect();
            if keys.is_empty() {
                continue;
            }
            catalog_lines += catalog_line as usize;
            let mut verified = 0;
            for chunk in keys.chunks(MAX_STATUS_KEYS) {
                std::thread::sleep(GAP);
                asked += 1;
                let statuses = client.statuses(chunk).expect("status request");
                verified += statuses.iter().filter(|(_, s)| *s == KeyStatus::Verified).count();
            }
            if verified > 0 {
                after += sc.reach;
                if catalog_line {
                    before += sc.reach;
                }
            }
            ready_flops += verified;
        }
        println!(
            "{} Spieler, {} BB: {:.0} % der Stock-Spots fertig (vorher höchstens {:.0} %), {} geprüfte Flops in {} Linien ({} davon Katalog-Linien), {} Abfragen",
            table.players,
            table.stack_bb,
            100.0 * after / total,
            100.0 * before / total,
            ready_flops,
            deep.scenarios.len(),
            catalog_lines,
            asked
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
        std::thread::sleep(GAP);
        let verified = client.statuses(&keys).expect("status request").iter().filter(|(_, s)| *s == KeyStatus::Verified).count();
        println!("{}: {verified} von {}", s.id, keys.len());
    }
}
