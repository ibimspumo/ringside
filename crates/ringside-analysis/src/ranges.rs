//! The model's range for a situation of the hero's range charts, and how the hero's
//! real frequencies compare with it.
//!
//! The situation is what most of the selected decisions share: table size, effective
//! stack (median), ante structure and the line before the hero. The model stands in at
//! its nearest precomputed configuration, like in the hand analysis; the notes say what
//! differs. Short stacks facing no raise, or an open shove, use the push/fold
//! equilibrium instead.

use ringside_core::db::RangeCounts;
use ringside_core::ranges::{AnteKind, History, Move};
use ringside_engine::multiway;
use ringside_engine::preflop::{Kind, Step};
use ringside_engine::preflop_table::{Ante, STACKS};
use ringside_engine::pushfold_table;
use ringside_engine::range::NUM_CLASSES;
use ringside_engine::HandClass;
use serde::Serialize;

use crate::fmt::bb;
use crate::seats::{ante_name, nearest};
use crate::{preflop, pushfold};

/// Below this effective stack the push/fold equilibrium always stands in for a
/// first-in decision; up to [`PUSH_FOLD_MAX_BB`] when the hero mostly shoves.
const PUSH_FOLD_BELOW_BB: f64 = 13.0;
const PUSH_FOLD_MAX_BB: f64 = 20.0;
/// A class counts as "missing" or "too much" from this difference in play frequency.
const GAP: f64 = 0.5;
const LISTED: usize = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelSource {
    /// Ringside's preflop model.
    Model,
    /// Push/fold equilibrium.
    PushFold,
}

/// The model's strategy at the situation, per class.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ModelRange {
    pub source: ModelSource,
    pub players: usize,
    pub stack_bb: f64,
    pub ante: Ante,
    /// Per class: fold, call, raise, all-in shares summing to 1; `None` when the model
    /// never gets here with the class (an earlier own action drops it).
    pub cells: Vec<Option<[f64; 4]>>,
    /// Raise and all-in sizes the model uses here, BB (0 when it has none).
    pub raise_to_bb: f64,
    pub all_in_bb: f64,
    /// Where the configuration differs from the hands, in plain words.
    pub notes: Vec<String>,
}

/// The hero against the model in plain numbers.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Comparison {
    /// Share of the hero's hands played (not folded), percent.
    pub you_pct: f64,
    /// Share of all hands the model plays here, weighted by combos, percent.
    pub model_pct: f64,
    /// Hands the model plays and the hero folds, biggest gap first, grouped ("A2o–A7o").
    pub missing: Vec<String>,
    /// Hands the hero plays and the model folds.
    pub extra: Vec<String>,
}

fn ante(a: AnteKind) -> Ante {
    match a {
        AnteKind::None => Ante::None,
        AnteKind::Classic => Ante::Classic,
        AnteKind::BigBlind => Ante::BigBlind,
    }
}

/// Seat labels at a real table of `n` players in preflop order, as the HUD names them.
fn real_names(n: usize) -> Vec<&'static str> {
    if n == 2 { vec!["BTN", "BB"] } else { multiway::position_names(n) }
}

fn bucket(kind: Kind) -> Move {
    match kind {
        Kind::Fold => Move::Fold,
        Kind::Check | Kind::Call | Kind::Limp => Move::Call,
        Kind::Raise => Move::Raise,
        Kind::AllIn => Move::AllIn,
    }
}

/// One step of a stored line ("CO:r") as (real seat, kind letter).
fn parse_line(line: &str, names: &[&str]) -> Result<Vec<(usize, char)>, String> {
    line.split_whitespace()
        .map(|s| {
            let (label, kind) = s.rsplit_once(':').ok_or("Linie unlesbar.")?;
            let seat = names.iter().position(|n| *n == label).ok_or(format!("{label} gibt es bei {} Spielern nicht.", names.len()))?;
            Ok((seat, kind.chars().next().unwrap_or('?')))
        })
        .collect()
}

/// What the situation is: the common facts of the selected decisions.
pub struct Situation<'a> {
    pub position: &'a str,
    pub history: Option<History>,
    pub counts: &'a RangeCounts,
}

/// The model's range for the situation, or why there is none.
pub fn model_range(s: &Situation) -> Result<ModelRange, String> {
    let c = s.counts;
    let (n, n_count) = c.players.ok_or("Keine Hände in dieser Auswahl.")?;
    let n = n as usize;
    let stack = c.stack_bb.ok_or("Keine Hände in dieser Auswahl.")?;
    let (ante_kind, _) = c.ante.ok_or("Keine Hände in dieser Auswahl.")?;
    let (line, line_count) = c.line.clone().unwrap_or_default();
    let names = real_names(n);
    let hero = names.iter().position(|x| *x == s.position).ok_or(format!("{} gibt es bei {n} Spielern nicht.", s.position))?;
    let steps = parse_line(&line, &names)?;
    let ante = ante(ante_kind);

    let mut notes = Vec::new();
    if n_count < c.total {
        notes.push(format!("{n} Spieler am Tisch: so war es bei {} % deiner Hände hier", pct(n_count, c.total)));
    }
    if line_count < c.total {
        notes.push(format!("diese Vorgeschichte hatten {} % deiner Hände hier", pct(line_count, c.total)));
    }

    let played = c.total - c.cells.iter().map(|x| x[Move::Fold.index()]).sum::<i64>();
    let shoves: i64 = c.cells.iter().map(|x| x[Move::AllIn.index()]).sum();
    let open_shove = matches!(steps.as_slice(), [(_, 'a'), rest @ ..] if rest.iter().all(|s| s.1 == 'c'));
    let push_fold = (steps.is_empty() && (stack < PUSH_FOLD_BELOW_BB || (stack <= PUSH_FOLD_MAX_BB && 2 * shoves >= played)))
        || (open_shove && stack <= PUSH_FOLD_MAX_BB);
    let mut range = if push_fold {
        push_fold_range(n, hero, &steps, stack, ante)?
    } else {
        model(n, hero, &steps, stack, ante)?
    };
    range.notes.splice(0..0, notes);
    Ok(range)
}

fn pct(part: i64, total: i64) -> i64 {
    if total == 0 { 0 } else { (100.0 * part as f64 / total as f64).round() as i64 }
}

fn depth_note(table: f64, real: f64) -> Option<String> {
    ((table - real).abs() >= 0.5).then(|| format!("nächste vorgerechnete Tiefe {} statt {} (typische Tiefe deiner Hände)", bb(table), bb(real)))
}

/// Table sizes the range comparison offers; the tables hold smaller ones too.
const COMPARED: [usize; 3] = [6, 8, 9];

fn model(n: usize, hero: usize, steps: &[(usize, char)], stack_real: f64, ante: Ante) -> Result<ModelRange, String> {
    let players = *COMPARED.iter().find(|&&p| p >= n).ok_or("Für diese Tischgröße gibt es kein Preflop-Modell.")?;
    let stack = nearest(&STACKS, stack_real);
    let sol = preflop::model(ante, players, stack).ok_or("Diese Modell-Konfiguration ist nicht vorgerechnet.")?;
    let seat = |real: usize| players - n + real;
    let mut line: Vec<Step> = Vec::new();
    for &(real, k) in steps {
        let v = sol.node(seat(real), &line).ok_or("Diese Vorgeschichte kommt im Preflop-Modell nicht vor.")?;
        let want = match k {
            'l' => Kind::Limp,
            'c' => Kind::Call,
            'a' => Kind::AllIn,
            _ => Kind::Raise,
        };
        let actions = &sol.nodes[v].actions;
        let action = actions
            .iter()
            .find(|a| a.kind == want)
            .or_else(|| match want {
                Kind::AllIn => actions.iter().find(|a| a.kind == Kind::Raise),
                Kind::Raise => actions.iter().find(|a| a.kind == Kind::AllIn),
                Kind::Limp => actions.iter().find(|a| a.kind == Kind::Call),
                _ => None,
            })
            .ok_or("Diese Vorgeschichte kommt im Preflop-Modell nicht vor (etwa ein dritter Limper).")?;
        line.push(Step { seat: seat(real), action: *action });
    }
    let v = sol.node(seat(hero), &line).ok_or("Diese Situation kommt im Preflop-Modell nicht vor.")?;
    let node = &sol.nodes[v];
    let cells = (0..NUM_CLASSES)
        .map(|class| {
            let reach: f64 = node.freq.iter().map(|f| f[class]).sum();
            (reach > 0.0).then(|| {
                let mut out = [0.0; 4];
                for (a, x) in node.actions.iter().enumerate() {
                    out[bucket(x.kind).index()] += node.freq[a][class] / reach;
                }
                out
            })
        })
        .collect();
    let size = |k: Kind| node.actions.iter().find(|a| a.kind == k).map_or(0.0, |a| a.to_bb);
    let mut notes = vec![format!("Preflop-Modell: {players} Spieler, alle mit {}, {}", bb(stack), ante_name(ante))];
    match players - n {
        0 => {}
        1 => notes.push(format!("am Tisch {n} Spieler: der erste Platz des Modells gilt als gefoldet")),
        k => notes.push(format!("am Tisch {n} Spieler: die ersten {k} Plätze des Modells gelten als gefoldet")),
    }
    notes.extend(depth_note(stack, stack_real));
    Ok(ModelRange {
        source: ModelSource::Model,
        players,
        stack_bb: stack,
        ante,
        cells,
        raise_to_bb: size(Kind::Raise),
        all_in_bb: size(Kind::AllIn),
        notes,
    })
}

fn push_fold_range(n: usize, hero: usize, steps: &[(usize, char)], stack_real: f64, ante: Ante) -> Result<ModelRange, String> {
    if !(pushfold_table::MIN_PLAYERS..=pushfold_table::MAX_PLAYERS).contains(&n) {
        return Err("Für diese Tischgröße gibt es keine Push/Fold-Tabelle.".into());
    }
    let stack = nearest(pushfold_table::stacks(ante), stack_real);
    let sol = pushfold::table(ante, n, stack).ok_or("Diese Push/Fold-Tabelle ist nicht vorgerechnet.")?;
    let shover = steps.first().map(|s| s.0);
    let callers: Vec<usize> = steps.iter().skip(1).map(|s| s.0).collect();
    let v = sol.node(hero, shover, &callers).ok_or("Diesen Spot gibt es im Push/Fold-Modell nicht (z. B. zu viele Caller).")?;
    let act = if shover.is_some() { Move::Call } else { Move::AllIn };
    let cells = sol.nodes[v]
        .act
        .iter()
        .map(|&f| {
            let mut out = [0.0; 4];
            out[act.index()] = f;
            out[Move::Fold.index()] = 1.0 - f;
            Some(out)
        })
        .collect();
    let mut notes = vec![format!("Push/Fold-Gleichgewicht (nur All-in oder Fold): {n} Spieler, alle mit {}, {}", bb(stack), ante_name(ante))];
    notes.extend(depth_note(stack, stack_real));
    Ok(ModelRange {
        source: ModelSource::PushFold,
        players: n,
        stack_bb: stack,
        ante,
        cells,
        raise_to_bb: 0.0,
        all_in_bb: stack,
        notes,
    })
}

/// Share of a cell that is not folded.
pub fn play(cell: &[f64; 4]) -> f64 {
    1.0 - cell[Move::Fold.index()]
}

/// "AKs", "T9o", "77".
pub fn class_name(class: HandClass) -> String {
    const RANKS: &[u8] = b"23456789TJQKA";
    let (hi, lo) = class.ranks();
    let (h, l) = (RANKS[hi as usize] as char, RANKS[lo as usize] as char);
    match (class.is_pair(), class.is_suited()) {
        (true, _) => format!("{h}{l}"),
        (false, true) => format!("{h}{l}s"),
        (false, false) => format!("{h}{l}o"),
    }
}

/// Classes as short groups: neighbours that differ only in the kicker (or pairs next to
/// each other) become "A2o–A7o"; the order of `classes` decides the order of groups.
pub fn group_classes(classes: &[HandClass]) -> Vec<String> {
    let family = |c: HandClass| -> (u8, u8) {
        let (hi, _) = c.ranks();
        if c.is_pair() { (0, 0) } else { (if c.is_suited() { 1 } else { 2 }, hi) }
    };
    let low = |c: HandClass| if c.is_pair() { c.ranks().0 } else { c.ranks().1 };
    let mut done = vec![false; classes.len()];
    let mut out = Vec::new();
    for i in 0..classes.len() {
        if done[i] {
            continue;
        }
        let f = family(classes[i]);
        let mut members: Vec<(u8, usize)> =
            (i..classes.len()).filter(|&j| !done[j] && family(classes[j]) == f).map(|j| (low(classes[j]), j)).collect();
        members.sort_unstable();
        // Runs of neighbouring ranks within the family; the first run holding `i` is taken.
        let mut runs: Vec<Vec<(u8, usize)>> = Vec::new();
        for m in members {
            match runs.last_mut() {
                Some(run) if run.last().is_some_and(|l| l.0 + 1 == m.0) => run.push(m),
                _ => runs.push(vec![m]),
            }
        }
        let run = runs.into_iter().find(|r| r.iter().any(|m| m.1 == i)).unwrap_or_default();
        for m in &run {
            done[m.1] = true;
        }
        let (first, last) = (classes[run[0].1], classes[run[run.len() - 1].1]);
        out.push(if run.len() == 1 { class_name(first) } else { format!("{}–{}", class_name(first), class_name(last)) });
    }
    out
}

/// The hero's frequencies against the model's.
pub fn compare(counts: &RangeCounts, model: &ModelRange) -> Comparison {
    let you_played = counts.total - counts.cells.iter().map(|x| x[Move::Fold.index()]).sum::<i64>();
    let (mut weight, mut model_play) = (0.0, 0.0);
    for (i, cell) in model.cells.iter().enumerate() {
        if let Some(cell) = cell {
            let w = HandClass::from_index(i).combo_count() as f64;
            weight += w;
            model_play += w * play(cell);
        }
    }
    // Biggest expected difference in hands played first.
    let mut missing: Vec<(f64, HandClass)> = Vec::new();
    let mut extra: Vec<(f64, HandClass)> = Vec::new();
    for (i, (mine, theirs)) in counts.cells.iter().zip(&model.cells).enumerate() {
        let n: i64 = mine.iter().sum();
        if n == 0 {
            continue;
        }
        let you = 1.0 - mine[Move::Fold.index()] as f64 / n as f64;
        let model = theirs.as_ref().map_or(0.0, play);
        let gap = model - you;
        let class = HandClass::from_index(i);
        if gap >= GAP {
            missing.push((gap * n as f64, class));
        } else if -gap >= GAP {
            extra.push((-gap * n as f64, class));
        }
    }
    let top = |mut list: Vec<(f64, HandClass)>| {
        list.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        let classes: Vec<HandClass> = list.into_iter().take(LISTED).map(|x| x.1).collect();
        group_classes(&classes)
    };
    Comparison {
        you_pct: if counts.total > 0 { 100.0 * you_played as f64 / counts.total as f64 } else { 0.0 },
        model_pct: if weight > 0.0 { 100.0 * model_play / weight } else { 0.0 },
        missing: top(missing),
        extra: top(extra),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn class(s: &str) -> HandClass {
        HandClass::all().find(|c| class_name(*c) == s).unwrap()
    }

    #[test]
    fn groups_neighbouring_kickers_and_pairs() {
        let list: Vec<HandClass> = ["A5o", "A2o", "K9o", "A3o", "Q9s", "A4o", "55", "44", "A7o"].iter().map(|s| class(s)).collect();
        assert_eq!(group_classes(&list), ["A2o–A5o", "K9o", "Q9s", "44–55", "A7o"]);
    }

    fn counts(total_per_class: i64, played: impl Fn(&str) -> bool) -> RangeCounts {
        let cells = (0..NUM_CLASSES)
            .map(|i| {
                let name = class_name(HandClass::from_index(i));
                if played(&name) { [0, 0, total_per_class, 0] } else { [total_per_class, 0, 0, 0] }
            })
            .collect();
        RangeCounts {
            total: total_per_class * NUM_CLASSES as i64,
            cells,
            stack_bb: Some(30.0),
            players: Some((6, total_per_class * NUM_CLASSES as i64)),
            ante: Some((AnteKind::Classic, 1)),
            line: Some((String::new(), total_per_class * NUM_CLASSES as i64)),
        }
    }

    #[test]
    fn button_open_against_the_model() {
        let c = counts(2, |name| name.starts_with('A') || name == "KK" || name == "QQ");
        let s = Situation { position: "BTN", history: Some(History::Unopened), counts: &c };
        let m = model_range(&s).unwrap();
        assert_eq!((m.source, m.players, m.stack_bb, m.ante), (ModelSource::Model, 6, 30.0, Ante::Classic));
        assert!(m.cells.iter().all(Option::is_some), "everyone reaches a first-in spot");
        let cmp = compare(&c, &m);
        assert!(cmp.model_pct > 30.0 && cmp.model_pct < 80.0, "{}", cmp.model_pct);
        assert!(cmp.you_pct < cmp.model_pct);
        assert!(!cmp.missing.is_empty() && cmp.missing.len() <= LISTED);
        // Aces are all played by the hero, so none of them can be missing.
        assert!(cmp.missing.iter().all(|g| !g.starts_with('A')), "{:?}", cmp.missing);
    }

    #[test]
    fn short_stacks_get_the_push_fold_chart_and_facing_an_open_the_model() {
        let mut c = counts(1, |name| name == "AA");
        c.stack_bb = Some(8.0);
        let s = Situation { position: "SB", history: Some(History::Unopened), counts: &c };
        assert_eq!(model_range(&s).unwrap().source, ModelSource::PushFold);

        c.stack_bb = Some(40.0);
        c.line = Some(("CO:r".into(), c.total));
        let m = model_range(&Situation { position: "BB", history: Some(History::VsOpen), counts: &c }).unwrap();
        assert_eq!(m.source, ModelSource::Model);
        assert!(m.cells[class("AA").index()].unwrap()[Move::Fold.index()] < 0.01);
        assert!(m.cells[class("72o").index()].unwrap()[Move::Fold.index()] > 0.9);

        c.line = Some(("UTG+2:r".into(), c.total));
        let err = model_range(&Situation { position: "BB", history: Some(History::VsOpen), counts: &c }).unwrap_err();
        assert!(err.contains("UTG+2"), "{err}");
    }
}
