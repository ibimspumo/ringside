//! Stored preflop decisions of the hero (see [`crate::ranges`]) and the sums the range
//! charts read: how often the hero folds, calls, raises or shoves each hand class in a
//! chosen situation.

use std::collections::BTreeMap;

use rusqlite::{Transaction, params};
use serde::{Deserialize, Serialize};

use super::{Db, Error, Filter};
use crate::model::Hand;
use crate::ranges::{AnteKind, History, Move, hero_preflop};

pub(super) const SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS hero_preflop (
        hand_id INTEGER NOT NULL,
        action_index INTEGER NOT NULL,
        game TEXT NOT NULL,
        tournament_id INTEGER,
        played_at TEXT NOT NULL,
        class INTEGER NOT NULL,
        position TEXT NOT NULL,
        seat INTEGER NOT NULL,
        players INTEGER NOT NULL,
        ante TEXT NOT NULL,
        stack_bb REAL NOT NULL,
        history TEXT NOT NULL,
        vs_position TEXT,
        vs_bb REAL,
        behind_bb REAL,
        line TEXT NOT NULL,
        action TEXT NOT NULL,
        PRIMARY KEY (hand_id, action_index)
    );
    CREATE INDEX IF NOT EXISTS hero_preflop_spot ON hero_preflop(history, position, class, stack_bb);";

/// Global filter over `hero_preflop p`; binds ?1 game, ?2 since, ?3 tournament id like
/// `FILTER_SQL` does for `hands h`.
const PREFLOP_FILTER_SQL: &str =
    "(?1 IS NULL OR p.game = ?1) AND (?2 IS NULL OR p.played_at >= ?2) AND (?3 IS NULL OR p.tournament_id = ?3)";

/// Position labels a filter may name; anything else matches nothing.
const POSITIONS: [&str; 11] = ["UTG", "UTG+1", "UTG+2", "UTG+3", "UTG+4", "LJ", "HJ", "CO", "BTN", "SB", "BB"];

/// Edges of the "shortest stack behind" buckets, in BB.
pub const BEHIND_EDGES: [f64; 2] = [10.0, 25.0];
/// Stack groups of the overview are whole BB up to this; deeper stacks share the last.
pub const STACK_CAP: i64 = 60;

/// Narrows the hero's preflop decisions to one situation. Every field is optional.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct RangeFilter {
    pub history: Option<History>,
    pub position: Option<String>,
    /// The raiser (or first limper) before the hero.
    pub vs_position: Option<String>,
    /// Effective stack in BB: at least `min_bb`, below `max_bb`.
    pub min_bb: Option<f64>,
    pub max_bb: Option<f64>,
    /// Players dealt in, inclusive.
    pub min_players: Option<u8>,
    pub max_players: Option<u8>,
    /// Shortest stack still to act after the hero, in BB: at least / below.
    pub min_behind_bb: Option<f64>,
    pub max_behind_bb: Option<f64>,
    /// One hand class (`HandClass::index`), for the hand list.
    pub class: Option<u8>,
    /// One action, for the hand list.
    pub action: Option<Move>,
}

fn position_sql(col: &str, p: &Option<String>) -> Option<String> {
    let p = p.as_ref()?;
    Some(match POSITIONS.iter().find(|x| **x == p) {
        Some(x) => format!("{col} = '{x}'"),
        None => "0".into(),
    })
}

fn bound(col: &str, op: &str, x: Option<f64>) -> Option<String> {
    x.filter(|x| x.is_finite()).map(|x| format!("{col} {op} {x}"))
}

impl RangeFilter {
    /// SQL over `hero_preflop p`. Every value is checked or numeric, so it goes into the
    /// SQL directly and the caller's parameter numbering stays untouched.
    pub(super) fn predicate(&self) -> String {
        let mut parts = vec!["1".to_string()];
        if let Some(h) = self.history {
            parts.push(format!("p.history = '{}'", h.key()));
        }
        parts.extend(position_sql("p.position", &self.position));
        parts.extend(position_sql("p.vs_position", &self.vs_position));
        parts.extend(bound("p.stack_bb", ">=", self.min_bb));
        parts.extend(bound("p.stack_bb", "<", self.max_bb));
        parts.extend(bound("p.players", ">=", self.min_players.map(f64::from)));
        parts.extend(bound("p.players", "<=", self.max_players.map(f64::from)));
        parts.extend(bound("p.behind_bb", ">=", self.min_behind_bb));
        parts.extend(bound("p.behind_bb", "<", self.max_behind_bb));
        if let Some(c) = self.class {
            parts.push(format!("p.class = {c}"));
        }
        if let Some(m) = self.action {
            parts.push(format!("p.action = '{}'", m.key()));
        }
        parts.join(" AND ")
    }

    /// Restriction of `hands h` for the hand list.
    pub(super) fn hands_sql(&self) -> String {
        format!("AND h.id IN (SELECT p.hand_id FROM hero_preflop p WHERE {})", self.predicate())
    }
}

/// How many decisions share one combination of the filterable facts; the page counts
/// its filter tiles from these without asking again.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RangeGroup {
    pub history: History,
    pub position: String,
    pub vs_position: Option<String>,
    /// Whole BB, capped at [`STACK_CAP`].
    pub stack: i64,
    pub players: u8,
    /// Bucket of the shortest stack behind: 0 below 10 BB, 1 below 25, 2 deeper; None
    /// when nobody is left to act.
    pub behind: Option<u8>,
    pub count: i64,
}

/// The hero's actions per hand class in one situation, and what the selection is like.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct RangeCounts {
    pub total: i64,
    /// Per class (`HandClass::index`): fold, call, raise, all-in.
    pub cells: Vec<[i64; 4]>,
    /// Median effective stack, BB.
    pub stack_bb: Option<f64>,
    /// Most common table size, ante structure and line before the hero, with how many
    /// decisions each covers.
    pub players: Option<(u8, i64)>,
    pub ante: Option<(AnteKind, i64)>,
    pub line: Option<(String, i64)>,
}

/// Writes the hero's preflop decisions of `hand`, replacing older rows.
pub(super) fn write(tx: &Transaction, hand: &Hand) -> Result<(), Error> {
    tx.prepare_cached("DELETE FROM hero_preflop WHERE hand_id = ?1")?.execute([hand.id as i64])?;
    let mut stmt = tx.prepare_cached(
        "INSERT INTO hero_preflop (hand_id, action_index, game, tournament_id, played_at, class, position, seat, players,
             ante, stack_bb, history, vs_position, vs_bb, behind_bb, line, action)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
    )?;
    for s in hero_preflop(hand) {
        stmt.execute(params![
            hand.id as i64,
            s.action_index as i64,
            hand.game.as_str(),
            hand.tournament_id.map(|t| t as i64),
            hand.played_at,
            s.class,
            s.position,
            s.seat,
            s.players,
            s.ante.key(),
            s.stack_bb,
            s.history.key(),
            s.vs_position,
            s.vs_bb,
            s.behind_bb,
            s.line,
            s.action.key(),
        ])?;
    }
    Ok(())
}

/// The most common key; ties go to the larger key, so the answer does not depend on row order.
fn most<K: Ord>(counts: BTreeMap<K, i64>) -> Option<(K, i64)> {
    counts.into_iter().max_by_key(|(_, n)| *n)
}

impl Db {
    /// Decisions in `filter` grouped by every filterable fact.
    pub fn range_groups(&self, filter: &Filter) -> Result<Vec<RangeGroup>, Error> {
        let sql = format!(
            "SELECT p.history, p.position, p.vs_position, MIN(CAST(p.stack_bb AS INTEGER), {STACK_CAP}) AS s, p.players,
                    CASE WHEN p.behind_bb IS NULL THEN NULL WHEN p.behind_bb < {} THEN 0 WHEN p.behind_bb < {} THEN 1 ELSE 2 END AS b,
                    COUNT(*)
             FROM hero_preflop p WHERE {PREFLOP_FILTER_SQL}
             GROUP BY p.history, p.position, p.vs_position, s, p.players, b",
            BEHIND_EDGES[0], BEHIND_EDGES[1]
        );
        let mut stmt = self.conn.prepare_cached(&sql)?;
        let rows = stmt.query_map(params![filter.game(), filter.since, filter.tournament()], |r| {
            Ok((r.get::<_, String>(0)?, RangeGroup {
                history: History::Unopened,
                position: r.get(1)?,
                vs_position: r.get(2)?,
                stack: r.get(3)?,
                players: r.get(4)?,
                behind: r.get(5)?,
                count: r.get(6)?,
            }))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (history, mut g) = row?;
            if let Some(h) = History::parse(&history) {
                g.history = h;
                out.push(g);
            }
        }
        Ok(out)
    }

    /// The hero's actions per class in `filter` and `range`.
    pub fn range_counts(&self, filter: &Filter, range: &RangeFilter) -> Result<RangeCounts, Error> {
        let sql = format!(
            "SELECT p.class, p.action, p.players, p.ante, p.line, CAST(p.stack_bb * 2 AS INTEGER), COUNT(*)
             FROM hero_preflop p WHERE {PREFLOP_FILTER_SQL} AND {}
             GROUP BY 1, 2, 3, 4, 5, 6",
            range.predicate()
        );
        let mut stmt = self.conn.prepare_cached(&sql)?;
        let mut rows = stmt.query(params![filter.game(), filter.since, filter.tournament()])?;
        let mut out = RangeCounts { cells: vec![[0; 4]; ringside_engine::range::NUM_CLASSES], ..Default::default() };
        let (mut players, mut antes, mut lines) = (BTreeMap::new(), BTreeMap::new(), BTreeMap::new());
        let mut stacks: Vec<(i64, i64)> = Vec::new();
        while let Some(r) = rows.next()? {
            let n: i64 = r.get(6)?;
            let class: usize = r.get(0)?;
            let Some(action) = Move::parse(&r.get::<_, String>(1)?) else { continue };
            if let Some(cell) = out.cells.get_mut(class) {
                cell[action.index()] += n;
            }
            out.total += n;
            *players.entry(r.get::<_, u8>(2)?).or_insert(0) += n;
            *antes.entry(AnteKind::parse(&r.get::<_, String>(3)?)).or_insert(0) += n;
            *lines.entry(r.get::<_, String>(4)?).or_insert(0) += n;
            stacks.push((r.get(5)?, n));
        }
        stacks.sort_unstable();
        let mut seen = 0;
        for (half_bb, n) in stacks {
            seen += n;
            if 2 * seen >= out.total {
                out.stack_bb = Some(half_bb as f64 / 2.0);
                break;
            }
        }
        out.players = most(players);
        out.ante = most(antes);
        out.line = most(lines);
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::HandQuery;
    use crate::parse_hand;

    /// Hero on the button with `cards`, everyone before folds; `hero` is the hero's action.
    fn hand(id: u64, cards: &str, hero: &str) -> (Hand, String) {
        let raw = format!(
            "PokerStars Hand #{id}: Hold'em No Limit ($0.05/$0.10 USD) - 2024/01/15 20:30:00 CET [2024/01/15 14:30:00 ET]
Table 'T' 6-max Seat #4 is the button
Seat 1: A ($10 in chips)
Seat 2: B ($10 in chips)
Seat 3: C ($3 in chips)
Seat 4: Hero ($2.50 in chips)
Seat 5: E ($8 in chips)
Seat 6: F ($12 in chips)
E: posts small blind $0.05
F: posts big blind $0.10
*** HOLE CARDS ***
Dealt to Hero [{cards}]
A: folds
B: folds
C: folds
{hero}
E: folds
F: folds
*** SUMMARY ***
Total pot $0.15 | Rake $0"
        );
        (parse_hand(&raw).expect("test hand parses"), raw)
    }

    #[test]
    fn counts_groups_and_hands_of_a_cell() {
        let mut db = Db::open_in_memory().unwrap();
        db.insert_hands(&[
            hand(1, "Ah Kd", "Hero: raises $0.20 to $0.30"),
            hand(2, "As Kc", "Hero: folds"),
            hand(3, "7h 2c", "Hero: folds"),
            hand(4, "Ac Kh", "Hero: raises $2.40 to $2.50 and is all-in"),
        ])
        .unwrap();
        let all = Filter::default();
        let groups = db.range_groups(&all).unwrap();
        assert_eq!(groups.len(), 1);
        let g = &groups[0];
        assert_eq!((g.history, g.position.as_str(), g.stack, g.players, g.behind, g.count), (History::Unopened, "BTN", 25, 6, Some(2), 4));

        let btn = RangeFilter { history: Some(History::Unopened), position: Some("BTN".into()), ..Default::default() };
        let c = db.range_counts(&all, &btn).unwrap();
        let ako = ringside_engine::Combo::new("Ah".parse().unwrap(), "Kd".parse().unwrap()).class().index();
        assert_eq!((c.total, c.cells[ako]), (4, [1, 0, 1, 1]));
        assert_eq!((c.stack_bb, c.players, c.ante), (Some(25.0), Some((6, 4)), Some((AnteKind::None, 4))));
        assert_eq!(c.line, Some((String::new(), 4)));
        assert_eq!(db.range_counts(&all, &RangeFilter { min_bb: Some(30.0), ..btn.clone() }).unwrap().total, 0);
        assert_eq!(db.range_counts(&all, &RangeFilter { position: Some("X' OR 1".into()), ..btn.clone() }).unwrap().total, 0);

        let cell = RangeFilter { class: Some(ako as u8), ..btn };
        let ids: Vec<u64> = db.hands(&HandQuery { range: Some(cell), limit: 10, ..Default::default() }).unwrap().iter().map(|h| h.id).collect();
        assert_eq!(ids, [4, 2, 1]);
    }
}
