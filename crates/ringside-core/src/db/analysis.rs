//! Stored hand analysis: every decision of the hero with what Ringside's engine says
//! about it, one summary row per analysed hand and the queue of postflop spots that
//! still need a solve. The evaluation itself lives in `ringside-analysis`; this module
//! only stores, filters and sums it.

use rusqlite::{OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};

use super::{Db, Error, FILTER_SQL, Filter};
use crate::model::Street;
use crate::profile::{ProfileRow, facing_bet};

pub(super) const SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS hero_decisions (
        hand_id INTEGER NOT NULL,
        action_index INTEGER NOT NULL,
        street TEXT NOT NULL,
        spot TEXT NOT NULL,
        status TEXT NOT NULL,
        played TEXT NOT NULL,
        best TEXT,
        strategy TEXT NOT NULL DEFAULT '[]',
        played_freq REAL,
        ev_loss_bb REAL,
        rating INTEGER,
        source TEXT,
        why TEXT NOT NULL DEFAULT '',
        note TEXT NOT NULL DEFAULT '',
        reason TEXT,
        solver TEXT,
        engine TEXT NOT NULL,
        pot_bb REAL,
        reliability REAL,
        PRIMARY KEY (hand_id, action_index)
    );
    CREATE INDEX IF NOT EXISTS hero_decisions_loss ON hero_decisions(ev_loss_bb);
    CREATE TABLE IF NOT EXISTS hand_analysis (
        hand_id INTEGER PRIMARY KEY,
        engine TEXT NOT NULL,
        decisions INTEGER NOT NULL,
        rated INTEGER NOT NULL,
        pending INTEGER NOT NULL,
        worst_loss_bb REAL,
        total_loss_bb REAL NOT NULL
    );
    CREATE INDEX IF NOT EXISTS hand_analysis_worst ON hand_analysis(worst_loss_bb);
    CREATE TABLE IF NOT EXISTS postflop_queue (
        hand_id INTEGER PRIMARY KEY,
        priority REAL NOT NULL,
        marked INTEGER NOT NULL DEFAULT 0,
        job TEXT NOT NULL
    );
    CREATE INDEX IF NOT EXISTS postflop_queue_order ON postflop_queue(marked, priority);
    CREATE TABLE IF NOT EXISTS spot_practice (
        key TEXT PRIMARY KEY,
        first_at INTEGER NOT NULL,
        after_hand INTEGER NOT NULL
    );";

/// How well a decision matches the recommendation, best first: the grades of the
/// practice (`ringside_play::Grade`), stored by level.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rating {
    Best,
    /// A rarer action of a mixed strategy: as good as the main one.
    Mix,
    Good,
    Inaccuracy,
    Mistake,
    Blunder,
}

impl Rating {
    pub const ALL: [Rating; 6] = [Rating::Best, Rating::Mix, Rating::Good, Rating::Inaccuracy, Rating::Mistake, Rating::Blunder];

    pub fn level(self) -> i64 {
        self as i64
    }

    pub fn from_level(level: i64) -> Option<Rating> {
        Rating::ALL.get(usize::try_from(level).ok()?).copied()
    }
}

/// Where a recommendation comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// Push/fold equilibrium (`ringside_engine::multiway`).
    Equilibrium,
    /// Ringside's preflop model for deeper stacks.
    Model,
    /// Heads-up postflop solve.
    Solver,
    /// Equity against the likely ranges, pot odds and fold equity, for spots outside
    /// model and solver; `reliability` is how often it agreed with the solver.
    Estimate,
    /// No decision: the hero had a single legal action.
    Forced,
}

impl Source {
    pub const ALL: [Source; 5] = [Source::Equilibrium, Source::Model, Source::Solver, Source::Estimate, Source::Forced];

    pub fn key(self) -> &'static str {
        match self {
            Source::Equilibrium => "equilibrium",
            Source::Model => "model",
            Source::Solver => "solver",
            Source::Estimate => "estimate",
            Source::Forced => "forced",
        }
    }

    pub fn parse(s: &str) -> Option<Source> {
        Source::ALL.into_iter().find(|x| x.key() == s)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpotType {
    /// First in from an early or middle seat.
    Open,
    /// First in from cutoff, button or small blind.
    Steal,
    /// Re-raising an open.
    ThreeBet,
    /// Calling or folding against an open.
    VsOpen,
    /// Behind one or two limps: isolate, limp along, check in the big blind or shove.
    VsLimp,
    /// Facing a 3-bet or more.
    VsThreeBet,
    /// All-in or fold, everyone before folded.
    Shove,
    CallShove,
    /// Flop, as the preflop raiser before anyone bet.
    CBet,
    /// Flop, facing the preflop raiser's bet.
    VsCBet,
    /// Any other flop decision.
    Flop,
    Turn,
    River,
}

impl SpotType {
    pub const ALL: [SpotType; 13] = [
        SpotType::Open,
        SpotType::Steal,
        SpotType::ThreeBet,
        SpotType::VsOpen,
        SpotType::VsLimp,
        SpotType::VsThreeBet,
        SpotType::Shove,
        SpotType::CallShove,
        SpotType::CBet,
        SpotType::VsCBet,
        SpotType::Flop,
        SpotType::Turn,
        SpotType::River,
    ];

    pub fn key(self) -> &'static str {
        match self {
            SpotType::Open => "open",
            SpotType::Steal => "steal",
            SpotType::ThreeBet => "three_bet",
            SpotType::VsOpen => "vs_open",
            SpotType::VsLimp => "vs_limp",
            SpotType::VsThreeBet => "vs_three_bet",
            SpotType::Shove => "shove",
            SpotType::CallShove => "call_shove",
            SpotType::CBet => "c_bet",
            SpotType::VsCBet => "vs_c_bet",
            SpotType::Flop => "flop",
            SpotType::Turn => "turn",
            SpotType::River => "river",
        }
    }

    pub fn parse(s: &str) -> Option<SpotType> {
        SpotType::ALL.into_iter().find(|x| x.key() == s)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Rated,
    /// Waiting for a postflop solve.
    Pending,
    /// Not a decision (source [`Source::Forced`]): neither rated nor counted.
    Forced,
    /// Outside what the engine covers; `reason` says why. Only hands of other games and
    /// failures end here.
    Unrated,
}

impl Status {
    fn key(self) -> &'static str {
        match self {
            Status::Rated => "rated",
            Status::Pending => "pending",
            Status::Forced => "forced",
            Status::Unrated => "unrated",
        }
    }

    pub(crate) fn parse(s: &str) -> Status {
        match s {
            "rated" => Status::Rated,
            "pending" => Status::Pending,
            "forced" => Status::Forced,
            _ => Status::Unrated,
        }
    }
}

/// One action the recommendation considers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Choice {
    pub label: String,
    /// Share of the time the recommendation takes it with the hero's hand, 0–1.
    pub freq: f64,
    /// Expected result in BB, where the source has one.
    pub ev_bb: Option<f64>,
    pub played: bool,
    /// "fold", "check", "call", "limp", "bet", "raise" or "all_in", as in the practice.
    #[serde(default)]
    pub kind: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Decision {
    pub hand_id: u64,
    /// Index into `Hand.actions`; the replayer stops before it.
    pub action_index: usize,
    pub street: Street,
    pub spot: SpotType,
    pub status: Status,
    /// What the hero did, e.g. "Raise 2,2 BB".
    pub played: String,
    /// The action the recommendation takes most often (best EV on ties).
    pub best: Option<String>,
    pub strategy: Vec<Choice>,
    pub played_freq: Option<f64>,
    pub ev_loss_bb: Option<f64>,
    pub rating: Option<Rating>,
    pub source: Option<Source>,
    /// The verdict in plain words with the reason, e.g. "Hier wäre All-in besser gewesen …".
    pub why: String,
    /// Assumptions and where the spot differs from what was computed.
    pub note: String,
    /// Why it is not rated (yet).
    pub reason: Option<String>,
    /// JSON for opening the spot in the solver, postflop only.
    pub solver: Option<String>,
    pub engine: String,
    /// Pot at the decision, BB: grades are cut in percent of it.
    pub pot_bb: Option<f64>,
    /// For [`Source::Estimate`]: share of the calibration spots where its grade was within
    /// one step of the solver's.
    pub reliability: Option<f64>,
}

/// The analysis of one hand, ready to store.
#[derive(Clone, Debug, PartialEq)]
pub struct HandAnalysis {
    pub hand_id: u64,
    pub engine: String,
    pub decisions: Vec<Decision>,
    /// Postflop spot to solve later, as JSON, with its queue priority.
    pub job: Option<(f64, String)>,
}

/// The worst decision of a hand, shown in the hand list.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DecisionBrief {
    pub action_index: usize,
    pub street: Street,
    pub spot: SpotType,
    pub status: Status,
    pub played: String,
    pub best: Option<String>,
    pub ev_loss_bb: Option<f64>,
    pub rating: Option<Rating>,
    pub source: Option<Source>,
    pub reason: Option<String>,
    pub why: String,
    pub pot_bb: Option<f64>,
    pub reliability: Option<f64>,
}

/// Narrows the hand list to hands with a matching decision.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct DecisionFilter {
    /// This rating or worse.
    pub min_rating: Option<Rating>,
    pub min_loss_bb: Option<f64>,
    #[serde(default)]
    pub spots: Vec<SpotType>,
    pub street: Option<Street>,
    pub source: Option<Source>,
    /// Decisions not rated (yet), and hands not analysed at all.
    #[serde(default)]
    pub unrated: bool,
}

fn street_key(s: Street) -> &'static str {
    match s {
        Street::Preflop => "preflop",
        Street::Flop => "flop",
        Street::Turn => "turn",
        Street::River => "river",
    }
}

pub(super) fn parse_street(s: &str) -> Street {
    match s {
        "flop" => Street::Flop,
        "turn" => Street::Turn,
        "river" => Street::River,
        _ => Street::Preflop,
    }
}

impl DecisionFilter {
    pub fn is_empty(&self) -> bool {
        *self == DecisionFilter::default()
    }

    /// SQL over `hero_decisions d`. Every value is an enum key or a number, so they are
    /// written into the SQL directly.
    pub(super) fn predicate(&self) -> String {
        let mut parts = vec!["1".to_string()];
        if self.unrated {
            parts.push("d.status IN ('pending', 'unrated')".into());
        }
        if let Some(r) = self.min_rating {
            parts.push(format!("d.rating >= {}", r.level()));
        }
        if let Some(x) = self.min_loss_bb.filter(|x| x.is_finite()) {
            parts.push(format!("d.ev_loss_bb >= {x}"));
        }
        if !self.spots.is_empty() {
            let keys: Vec<String> = self.spots.iter().map(|s| format!("'{}'", s.key())).collect();
            parts.push(format!("d.spot IN ({})", keys.join(", ")));
        }
        if let Some(s) = self.street {
            parts.push(format!("d.street = '{}'", street_key(s)));
        }
        if let Some(s) = self.source {
            parts.push(format!("d.source = '{}'", s.key()));
        }
        parts.join(" AND ")
    }

    /// Restriction of `hands h` for the hand list.
    pub(super) fn hands_sql(&self) -> String {
        let exists = format!("EXISTS (SELECT 1 FROM hero_decisions d WHERE d.hand_id = h.id AND {})", self.predicate());
        if self.unrated {
            format!("AND ({exists} OR NOT EXISTS (SELECT 1 FROM hand_analysis a WHERE a.hand_id = h.id))")
        } else {
            format!("AND {exists}")
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct AnalysisCounts {
    pub hands: i64,
    /// Analysed with the current engine (hands without a hero count as done).
    pub analyzed: i64,
    pub decisions: i64,
    pub rated: i64,
    pub pending: i64,
    /// Hands whose postflop spot waits for a solve.
    pub queued: i64,
}

/// The analysed hands after a given hand, see [`Db::analysis_since`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct AnalysisSince {
    pub hands: i64,
    /// Rated decisions that are mistakes or worse.
    pub mistakes: i64,
    pub last_hand: Option<u64>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct SpotLoss {
    pub spot: Option<SpotType>,
    pub decisions: i64,
    pub rated: i64,
    /// Inaccuracies or worse.
    pub flawed: i64,
    pub loss_bb: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CostlyDecision {
    pub hand_id: u64,
    pub played_at: String,
    pub hero_cards: String,
    pub position: Option<String>,
    #[serde(flatten)]
    pub decision: DecisionBrief,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct AnalysisSummary {
    pub hands: i64,
    pub analyzed: i64,
    pub decisions: i64,
    pub rated: i64,
    pub loss_bb: f64,
    /// Rated decisions per rating, best first.
    pub ratings: Vec<i64>,
    pub by_source: Vec<(Source, i64)>,
    pub by_spot: Vec<SpotLoss>,
    pub costliest: Vec<CostlyDecision>,
}

/// Level from which a rating counts as flawed: inaccuracies or worse.
const FLAWED: i64 = Rating::Inaccuracy as i64;

const BRIEF_COLS: &str = "d.action_index, d.street, d.spot, d.status, d.played, d.best, d.ev_loss_bb, d.rating, d.source, d.reason, d.why,
     d.pot_bb, d.reliability";

fn brief_at(r: &rusqlite::Row, first: usize) -> rusqlite::Result<DecisionBrief> {
    Ok(DecisionBrief {
        action_index: r.get::<_, i64>(first)? as usize,
        street: parse_street(&r.get::<_, String>(first + 1)?),
        spot: SpotType::parse(&r.get::<_, String>(first + 2)?).unwrap_or(SpotType::Flop),
        status: Status::parse(&r.get::<_, String>(first + 3)?),
        played: r.get(first + 4)?,
        best: r.get(first + 5)?,
        ev_loss_bb: r.get(first + 6)?,
        rating: r.get::<_, Option<i64>>(first + 7)?.and_then(Rating::from_level),
        source: r.get::<_, Option<String>>(first + 8)?.as_deref().and_then(Source::parse),
        reason: r.get(first + 9)?,
        why: r.get(first + 10)?,
        pot_bb: r.get(first + 11)?,
        reliability: r.get(first + 12)?,
    })
}

/// Worst first: rated by loss, then pending, then unrated; forced moves last.
const WORST_ORDER: &str = "CASE d.status WHEN 'rated' THEN 0 WHEN 'pending' THEN 1 WHEN 'unrated' THEN 2 ELSE 3 END, d.ev_loss_bb DESC,
     d.rating DESC, d.action_index";

fn write_decision(tx: &Transaction, d: &Decision) -> Result<(), Error> {
    let strategy = serde_json::to_string(&d.strategy).unwrap_or_else(|_| "[]".into());
    tx.prepare_cached(
        "INSERT OR REPLACE INTO hero_decisions (hand_id, action_index, street, spot, status, played, best, strategy,
             played_freq, ev_loss_bb, rating, source, why, note, reason, solver, engine, pot_bb, reliability)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
    )?
    .execute(params![
        d.hand_id as i64,
        d.action_index as i64,
        street_key(d.street),
        d.spot.key(),
        d.status.key(),
        d.played,
        d.best,
        strategy,
        d.played_freq,
        d.ev_loss_bb,
        d.rating.map(Rating::level),
        d.source.map(Source::key),
        d.why,
        d.note,
        d.reason,
        d.solver,
        d.engine,
        d.pot_bb,
        d.reliability,
    ])?;
    Ok(())
}

/// Recomputes the summary row of `hand_id` from its decisions; forced moves are no
/// decisions and not counted.
fn write_summary(tx: &Transaction, hand_id: u64, engine: &str) -> Result<(), Error> {
    tx.prepare_cached(
        "INSERT OR REPLACE INTO hand_analysis (hand_id, engine, decisions, rated, pending, worst_loss_bb, total_loss_bb)
         SELECT ?1, ?2, COUNT(d.hand_id), COALESCE(SUM(d.status = 'rated'), 0), COALESCE(SUM(d.status = 'pending'), 0),
                MAX(d.ev_loss_bb), COALESCE(SUM(d.ev_loss_bb), 0)
         FROM (SELECT 1) LEFT JOIN hero_decisions d ON d.hand_id = ?1 AND d.status <> 'forced'",
    )?
    .execute(params![hand_id as i64, engine])?;
    Ok(())
}

impl Db {
    /// Stores analysed hands, replacing what was stored for them before. A queued
    /// postflop spot keeps its mark.
    pub fn save_analysis(&mut self, hands: &[HandAnalysis]) -> Result<(), Error> {
        let tx = self.conn.transaction()?;
        for h in hands {
            tx.prepare_cached("DELETE FROM hero_decisions WHERE hand_id = ?1")?.execute([h.hand_id as i64])?;
            for d in &h.decisions {
                write_decision(&tx, d)?;
            }
            write_summary(&tx, h.hand_id, &h.engine)?;
            match &h.job {
                Some((priority, job)) => {
                    tx.prepare_cached(
                        "INSERT INTO postflop_queue (hand_id, priority, job) VALUES (?1, ?2, ?3)
                         ON CONFLICT(hand_id) DO UPDATE SET priority = excluded.priority, job = excluded.job",
                    )?
                    .execute(params![h.hand_id as i64, priority, job])?;
                }
                None => {
                    tx.prepare_cached("DELETE FROM postflop_queue WHERE hand_id = ?1")?.execute([h.hand_id as i64])?;
                }
            }
        }
        tx.commit()
    }

    /// Hands after `after_id` that were not analysed with `engine`, by id.
    pub fn analysis_candidates(&self, engine: &str, after_id: u64, limit: usize) -> Result<Vec<(u64, String)>, Error> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT h.id, h.raw FROM hands h LEFT JOIN hand_analysis a ON a.hand_id = h.id
             WHERE h.id > ?1 AND (a.engine IS NULL OR a.engine <> ?2)
             ORDER BY h.id LIMIT ?3",
        )?;
        let rows = stmt.query_map(params![after_id as i64, engine, limit as i64], |r| Ok((r.get::<_, i64>(0)? as u64, r.get(1)?)))?;
        rows.collect()
    }

    pub fn analysis_counts(&self, engine: &str) -> Result<AnalysisCounts, Error> {
        // Reads only the small analysis rows; filtering `hands` by a column would read every raw hand.
        let hands = self.hand_count()?;
        let (analyzed, decisions, rated, pending): (i64, i64, i64, i64) = self.conn.query_row(
            "SELECT COALESCE(SUM(engine = ?1), 0), COALESCE(SUM(decisions), 0), COALESCE(SUM(rated), 0),
                    COALESCE(SUM(pending), 0) FROM hand_analysis",
            [engine],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )?;
        let queued = self.conn.query_row("SELECT COUNT(*) FROM postflop_queue", [], |r| r.get(0))?;
        Ok(AnalysisCounts { hands, analyzed, decisions, rated, pending, queued })
    }

    /// The postflop spot to solve next among the marked hands and those after `after_id`:
    /// marked hands first, then the biggest pots. `0` takes every hand, `u64::MAX` only
    /// the marked ones.
    pub fn next_postflop_job(&self, after_id: u64) -> Result<Option<(u64, String)>, Error> {
        self.conn
            .query_row(
                "SELECT hand_id, job FROM postflop_queue WHERE marked > 0 OR hand_id > ?1
                 ORDER BY marked DESC, priority DESC, hand_id LIMIT 1",
                [i64::try_from(after_id).unwrap_or(i64::MAX)],
                |r| Ok((r.get::<_, i64>(0)? as u64, r.get(1)?)),
            )
            .optional()
    }

    /// What the analysis says about the hands after `after_id`: how many with a decision
    /// of the hero are analysed with `engine`, how many of their decisions are mistakes or
    /// worse, and the highest analysed hand id.
    pub fn analysis_since(&self, engine: &str, after_id: u64) -> Result<AnalysisSince, Error> {
        let after = i64::try_from(after_id).unwrap_or(i64::MAX);
        let (hands, last) = self.conn.query_row(
            "SELECT COALESCE(SUM(engine = ?2 AND decisions > 0), 0), MAX(hand_id) FROM hand_analysis WHERE hand_id > ?1",
            params![after, engine],
            |r| Ok((r.get(0)?, r.get::<_, Option<i64>>(1)?)),
        )?;
        let mistakes = self.conn.query_row(
            "SELECT COUNT(*) FROM hero_decisions WHERE hand_id > ?1 AND status = 'rated' AND rating >= ?2",
            params![after, Rating::Mistake.level()],
            |r| r.get(0),
        )?;
        Ok(AnalysisSince { hands, mistakes, last_hand: last.map(|id| id as u64) })
    }

    /// Moves a queued hand to the front. False when nothing of it waits.
    pub fn mark_postflop(&self, hand_id: u64) -> Result<bool, Error> {
        let marked = self.conn.query_row("SELECT COALESCE(MAX(marked), 0) + 1 FROM postflop_queue", [], |r| r.get::<_, i64>(0))?;
        Ok(self.conn.execute("UPDATE postflop_queue SET marked = ?2 WHERE hand_id = ?1", params![hand_id as i64, marked])? > 0)
    }

    /// Puts a marked hand back in line by pot size. False when nothing of it waits.
    pub fn unmark_postflop(&self, hand_id: u64) -> Result<bool, Error> {
        Ok(self.conn.execute("UPDATE postflop_queue SET marked = 0 WHERE hand_id = ?1", [hand_id as i64])? > 0)
    }

    /// Puts every marked hand back in line by pot size.
    pub fn unmark_all_postflop(&self) -> Result<(), Error> {
        self.conn.execute("UPDATE postflop_queue SET marked = 0 WHERE marked > 0", []).map(|_| ())
    }

    /// Stores the solved (or failed) postflop decisions of a queued hand and takes it off
    /// the queue.
    pub fn finish_postflop(&mut self, hand_id: u64, decisions: &[Decision]) -> Result<(), Error> {
        let tx = self.conn.transaction()?;
        let engine: Option<String> = tx
            .query_row("SELECT engine FROM hand_analysis WHERE hand_id = ?1", [hand_id as i64], |r| r.get(0))
            .optional()?;
        for d in decisions {
            write_decision(&tx, d)?;
        }
        if let Some(engine) = engine {
            write_summary(&tx, hand_id, &engine)?;
        }
        tx.execute("DELETE FROM postflop_queue WHERE hand_id = ?1", [hand_id as i64])?;
        tx.commit()
    }

    /// Drops every analysis, so the next run starts over.
    pub fn clear_analysis(&self) -> Result<(), Error> {
        self.conn.execute_batch("DELETE FROM hero_decisions; DELETE FROM hand_analysis; DELETE FROM postflop_queue;")
    }

    /// Every stored decision of a hand, in action order.
    pub fn hand_decisions(&self, hand_id: u64) -> Result<Vec<Decision>, Error> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT action_index, street, spot, status, played, best, strategy, played_freq, ev_loss_bb, rating, source,
                    note, reason, solver, engine, why, pot_bb, reliability
             FROM hero_decisions WHERE hand_id = ?1 ORDER BY action_index",
        )?;
        let rows = stmt.query_map([hand_id as i64], |r| {
            Ok(Decision {
                hand_id,
                action_index: r.get::<_, i64>(0)? as usize,
                street: parse_street(&r.get::<_, String>(1)?),
                spot: SpotType::parse(&r.get::<_, String>(2)?).unwrap_or(SpotType::Flop),
                status: Status::parse(&r.get::<_, String>(3)?),
                played: r.get(4)?,
                best: r.get(5)?,
                strategy: serde_json::from_str(&r.get::<_, String>(6)?).unwrap_or_default(),
                played_freq: r.get(7)?,
                ev_loss_bb: r.get(8)?,
                rating: r.get::<_, Option<i64>>(9)?.and_then(Rating::from_level),
                source: r.get::<_, Option<String>>(10)?.as_deref().and_then(Source::parse),
                note: r.get(11)?,
                reason: r.get(12)?,
                solver: r.get(13)?,
                engine: r.get(14)?,
                why: r.get(15)?,
                pot_bb: r.get(16)?,
                reliability: r.get(17)?,
            })
        })?;
        rows.collect()
    }

    /// The worst decision of a hand among those matching `filter`.
    pub fn worst_decision(&self, hand_id: u64, filter: &DecisionFilter) -> Result<Option<DecisionBrief>, Error> {
        let sql = format!(
            "SELECT {BRIEF_COLS} FROM hero_decisions d WHERE d.hand_id = ?1 AND {} ORDER BY {WORST_ORDER} LIMIT 1",
            filter.predicate()
        );
        self.conn.prepare_cached(&sql)?.query_row([hand_id as i64], |r| brief_at(r, 0)).optional()
    }

    /// Totals for the hands of `filter`: loss per spot type and the costliest decisions.
    pub fn analysis_summary(&self, filter: &Filter, engine: &str, top: usize) -> Result<AnalysisSummary, Error> {
        let args = params![filter.game(), filter.since, filter.tournament(), engine];
        let mut out = AnalysisSummary { ratings: vec![0; Rating::ALL.len()], ..Default::default() };
        (out.hands, out.analyzed) = self.conn.prepare_cached(&format!(
            "SELECT COUNT(*), COALESCE(SUM(a.engine = ?4), 0) FROM hands h LEFT JOIN hand_analysis a ON a.hand_id = h.id
             WHERE h.hero IS NOT NULL AND {FILTER_SQL}"
        ))?
        .query_row(args, |r| Ok((r.get(0)?, r.get(1)?)))?;

        let from = format!(
            "FROM hero_decisions d JOIN hands h ON h.id = d.hand_id WHERE {FILTER_SQL} AND ?4 IS NOT NULL AND d.status <> 'forced'"
        );
        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT d.spot, COUNT(*), COALESCE(SUM(d.status = 'rated'), 0), COALESCE(SUM(d.rating >= {FLAWED}), 0),
                    COALESCE(SUM(d.ev_loss_bb), 0)
             {from} GROUP BY d.spot"
        ))?;
        let rows = stmt.query_map(args, |r| {
            Ok(SpotLoss {
                spot: SpotType::parse(&r.get::<_, String>(0)?),
                decisions: r.get(1)?,
                rated: r.get(2)?,
                flawed: r.get(3)?,
                loss_bb: r.get(4)?,
            })
        })?;
        for row in rows {
            let row = row?;
            out.decisions += row.decisions;
            out.rated += row.rated;
            out.loss_bb += row.loss_bb;
            out.by_spot.push(row);
        }
        out.by_spot.sort_by(|a, b| b.loss_bb.total_cmp(&a.loss_bb));

        let mut stmt = self.conn.prepare_cached(&format!("SELECT d.rating, COUNT(*) {from} AND d.rating IS NOT NULL GROUP BY d.rating"))?;
        for row in stmt.query_map(args, |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))? {
            let (level, n) = row?;
            if let Some(slot) = out.ratings.get_mut(level as usize) {
                *slot = n;
            }
        }
        let mut stmt = self.conn.prepare_cached(&format!("SELECT d.source, COUNT(*) {from} AND d.source IS NOT NULL AND d.status = 'rated' GROUP BY d.source"))?;
        for row in stmt.query_map(args, |r| r.get::<_, String>(0).map(|s| (s, r.get::<_, i64>(1))))? {
            let (s, n) = row?;
            if let Some(s) = Source::parse(&s) {
                out.by_source.push((s, n?));
            }
        }

        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT h.id, h.played_at, h.hero_cards, h.hero_position, {BRIEF_COLS}
             {from} AND d.rating >= {FLAWED} ORDER BY d.ev_loss_bb DESC LIMIT {top}"
        ))?;
        let rows = stmt.query_map(args, |r| {
            Ok(CostlyDecision {
                hand_id: r.get::<_, i64>(0)? as u64,
                played_at: r.get(1)?,
                hero_cards: r.get(2)?,
                position: r.get(3)?,
                decision: brief_at(r, 4)?,
            })
        })?;
        out.costliest = rows.collect::<Result<_, _>>()?;
        Ok(out)
    }
    /// Every unforced decision of the hero in the hands of `filter`, with what the playing
    /// profile needs of its hand.
    pub fn profile_rows(&self, filter: &Filter) -> Result<Vec<ProfileRow>, Error> {
        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT d.hand_id, d.action_index, h.played_at, d.spot, d.status, d.rating, d.ev_loss_bb, d.source, d.reliability,
                    d.strategy, h.hero_position, h.hero_stack_bb,
                    (SELECT COUNT(*) FROM player_hands p WHERE p.hand_id = h.id)
             FROM hero_decisions d JOIN hands h ON h.id = d.hand_id
             WHERE {FILTER_SQL} AND d.status <> 'forced'
             ORDER BY d.hand_id, d.action_index"
        ))?;
        let rows = stmt.query_map(params![filter.game(), filter.since, filter.tournament()], |r| {
            let strategy: Vec<Choice> = serde_json::from_str(&r.get::<_, String>(9)?).unwrap_or_default();
            Ok(ProfileRow {
                hand_id: r.get::<_, i64>(0)? as u64,
                action_index: r.get::<_, i64>(1)? as usize,
                played_at: r.get(2)?,
                spot: SpotType::parse(&r.get::<_, String>(3)?).unwrap_or(SpotType::Flop),
                status: Status::parse(&r.get::<_, String>(4)?),
                rating: r.get::<_, Option<i64>>(5)?.and_then(Rating::from_level),
                ev_loss_bb: r.get(6)?,
                source: r.get::<_, Option<String>>(7)?.as_deref().and_then(Source::parse),
                reliability: r.get(8)?,
                facing_bet: facing_bet(&strategy),
                position: r.get(10)?,
                stack_bb: r.get(11)?,
                players: r.get::<_, i64>(12)? as usize,
            })
        })?;
        rows.collect()
    }

    /// Remembers the first practice of a kind of spot with the newest hand stored then, so
    /// later hands show what changed. Later practice keeps the first.
    pub fn mark_spot_practiced(&self, key: &str, at: i64) -> Result<(), Error> {
        self.conn
            .execute(
                "INSERT OR IGNORE INTO spot_practice (key, first_at, after_hand) SELECT ?1, ?2, COALESCE(MAX(id), 0) FROM hands",
                params![key, at],
            )
            .map(|_| ())
    }

    /// Kinds of spot practiced, with the newest hand stored at their first practice.
    pub fn spot_practice(&self) -> Result<Vec<(String, u64)>, Error> {
        let mut stmt = self.conn.prepare_cached("SELECT key, after_hand FROM spot_practice ORDER BY key")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get::<_, i64>(1)? as u64)))?;
        rows.collect()
    }

    /// One stored decision in brief.
    pub fn decision_brief(&self, hand_id: u64, action_index: usize) -> Result<Option<DecisionBrief>, Error> {
        self.conn
            .prepare_cached(&format!("SELECT {BRIEF_COLS} FROM hero_decisions d WHERE d.hand_id = ?1 AND d.action_index = ?2"))?
            .query_row(params![hand_id as i64, action_index as i64], |r| brief_at(r, 0))
            .optional()
    }
}
