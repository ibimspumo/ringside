//! Every analysis that runs on PokerStars hands runs on GGPoker hands too.

use ringside_analysis::{Source, Status, analyze_hand};
use ringside_core::model::{Hand, hand_label};
use ringside_core::{parse_hand, split_hands};

const MTT: &str = include_str!("../../ringside-core/tests/fixtures/ggpoker/mtt.txt");
const CASH: &str = include_str!("../../ringside-core/tests/fixtures/ggpoker/cash.txt");
const SPECIAL: &str = include_str!("../../ringside-core/tests/fixtures/ggpoker/special.txt");
const SPIN: &str = include_str!("../../ringside-core/tests/fixtures/ggpoker/spin.txt");

fn hand(text: &str, label: &str) -> Hand {
    split_hands(text).into_iter().filter_map(|h| parse_hand(h).ok()).find(|h| hand_label(h.id) == label).expect("fixture hand")
}

fn sources(h: &Hand) -> Vec<(Status, Option<Source>, String)> {
    analyze_hand(h).decisions.into_iter().map(|d| (d.status, d.source, d.reason.unwrap_or_default())).collect()
}

#[test]
fn a_tournament_shove_is_rated_by_the_push_fold_equilibrium() {
    let shove = sources(&hand(MTT, "TM5100200301"));
    assert_eq!(shove.len(), 1, "{shove:?}");
    assert_eq!((shove[0].0, shove[0].1), (Status::Rated, Some(Source::Equilibrium)), "{shove:?}");
}

#[test]
fn every_hold_em_decision_gets_an_answer() {
    for (text, label) in [(MTT, "TM5100200288"), (CASH, "HD3100200401"), (SPECIAL, "HD3100200502"), (SPECIAL, "RC3100200610"), (SPIN, "SG3100200800"), (SPIN, "SG3100200805")] {
        let a = analyze_hand(&hand(text, label));
        assert!(!a.decisions.is_empty(), "{label}");
        for d in &a.decisions {
            assert!(d.status != Status::Unrated, "{label}: {d:?}");
        }
    }
}

#[test]
fn run_it_twice_and_cashout_do_not_change_the_rating() {
    // The same all-in with the result of a single board has the same decisions and costs.
    let twice = hand(SPECIAL, "HD3100200502");
    let mut once = twice.clone();
    once.collected = vec![("b4d2e6f8".into(), 1630)];
    let rate = |h: &Hand| analyze_hand(h).decisions.into_iter().map(|d| (d.action_index, d.rating, d.ev_loss_bb.map(|l| (l * 1000.0).round()))).collect::<Vec<_>>();
    assert_eq!(rate(&twice), rate(&once));
    let cashout = hand(SPECIAL, "RC3100200610");
    let mut kept = cashout.clone();
    kept.cashouts.clear();
    assert_eq!(rate(&cashout), rate(&kept));
}

#[test]
fn what_the_model_does_not_know_stays_unrated_or_estimated() {
    let straddle = hand(CASH, "HD3100200455");
    for (status, source, reason) in sources(&straddle) {
        assert!(status == Status::Unrated || source == Some(Source::Estimate), "{status:?} {source:?} {reason}");
    }
    let omaha = analyze_hand(&hand(SPECIAL, "OM3100200700"));
    assert!(omaha.decisions.is_empty() && omaha.job.is_none(), "Omaha is no model game");
}

