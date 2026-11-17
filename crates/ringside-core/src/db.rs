//! Local SQLite store. Raw hand text is kept so derived data can be rebuilt after upgrades.

use std::collections::BTreeMap;
use std::path::Path;

use rusqlite::{Connection, OptionalExtension, Transaction, params, params_from_iter};
use serde::{Deserialize, Serialize};

use crate::allin::hero_all_in;
use crate::model::{GameKind, Hand};
use crate::parser::parse_hand;
use crate::leaks::{self, Cell};
use crate::stats::{Counters, StatId, analyze, hand_class, position_labels};
use crate::tournament::{BuyIn, Summary, TournamentRow};

pub use rusqlite::Error;

/// Bump when derived columns or counters change; older databases are rebuilt from raw hands.
const SCHEMA_VERSION: i32 = 5;

pub struct Db {
    conn: Connection,
}

/// Restricts reports to a kind of game, a start date ("YYYY/MM/DD") and one tournament.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Filter {
    pub game: Option<GameKind>,
    pub since: Option<String>,
    pub tournament_id: Option<u64>,
}

impl Filter {
    fn game(&self) -> Option<&'static str> {
        self.game.map(GameKind::as_str)
    }

    fn tournament(&self) -> Option<i64> {
        self.tournament_id.map(|t| t as i64)
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct HandQuery {
    #[serde(flatten)]
    pub filter: Filter,
    pub player: Option<String>,
    pub showdown_only: bool,
    /// "won" or "lost", from the hero's point of view.
    pub result: Option<String>,
    pub min_pot_bb: Option<f64>,
    /// Hands of one leak-finder spot.
    #[serde(default)]
    pub spot: Option<SpotFilter>,
    pub limit: usize,
    pub offset: usize,
}

/// Hero hands of `game` with the chance for `stat` at `positions` (empty: any) and stack `band`.
/// `made` narrows to hands where the hero did it (`true`) or did not (`false`); for AF
/// it picks hands with an aggressive action or with a call after the flop.
#[derive(Debug, Clone, Deserialize)]
pub struct SpotFilter {
    pub stat: StatId,
    pub made: Option<bool>,
    #[serde(default)]
    pub positions: Vec<String>,
    pub band: Option<u8>,
    pub game: Option<GameKind>,
}

impl SpotFilter {
    fn predicate(&self) -> Option<String> {
        let (made, opp) = leaks::columns(self.stat)?;
        Some(match (self.stat, self.made) {
            (StatId::Af, Some(true)) => format!("ph.{made} > 0"),
            (StatId::Af, Some(false)) => format!("ph.{opp} > 0"),
            (StatId::Af, None) => format!("ph.{made} + ph.{opp} > 0"),
            (_, Some(true)) => format!("ph.{opp} > 0 AND ph.{made} > 0"),
            (_, Some(false)) => format!("ph.{opp} > 0 AND ph.{made} = 0"),
            (_, None) => format!("ph.{opp} > 0"),
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct HandSummary {
    pub id: u64,
    pub game: GameKind,
    pub played_at: String,
    pub table_name: String,
    pub small_blind: i64,
    pub big_blind: i64,
    pub currency: Option<String>,
    pub hero_cards: String,
    pub hero_position: Option<String>,
    pub hero_net: Option<i64>,
    pub hero_showdown: bool,
    pub pot: i64,
    pub board: String,
}

/// Cumulative hero results. Totals are exact; `points` is thinned to at most `max_points`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Results {
    pub hands: i64,
    pub net: i64,
    pub net_bb: f64,
    pub showdown_bb: f64,
    pub non_showdown_bb: f64,
    /// Result with every all-in that had all hands known counted at its expectation.
    pub ev_bb: f64,
    /// Those all-ins: how many, and what they paid more than expected (`net − EV`).
    pub all_ins: i64,
    pub luck_bb: f64,
    /// Knockout money won, per currency ("USD", "EUR", ...).
    pub bounties: BTreeMap<String, i64>,
    pub points: Vec<ResultPoint>,
}

/// Running totals after hand number `n` (1-based).
#[derive(Debug, Clone, Serialize)]
pub struct ResultPoint {
    pub n: i64,
    pub played_at: String,
    pub net_bb: f64,
    pub showdown_bb: f64,
    pub non_showdown_bb: f64,
    pub ev_bb: f64,
}

/// One answered learning exercise. `seed` identifies a generated exercise, `hand_id`
/// one from the user's hands; `answer` is the answer as JSON; `at` is Unix seconds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Attempt {
    pub kind: String,
    pub level: u8,
    pub seed: Option<u64>,
    pub hand_id: Option<u64>,
    pub answer: String,
    pub score: f64,
    pub ev_loss_bb: Option<f64>,
    pub at: i64,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct RebuildProgress {
    pub done: i64,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlayerRow {
    pub name: String,
    pub counters: Counters,
    pub net: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct GroupRow {
    /// Position label or starting hand class.
    pub key: String,
    pub counters: Counters,
    pub net: i64,
    pub net_bb: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Session {
    pub start: String,
    pub end: String,
    pub hands: i64,
    pub net: i64,
    pub net_bb: f64,
    /// Knockout money won, per currency.
    pub bounties: BTreeMap<String, i64>,
    pub tables: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ShownHand {
    pub hand_id: u64,
    pub played_at: String,
    pub cards: String,
    pub board: String,
    pub position: Option<String>,
    pub net: i64,
    pub big_blind: i64,
}

fn sums() -> String {
    Counters::FIELDS.iter().map(|f| format!("SUM(ph.{f})")).collect::<Vec<_>>().join(", ")
}

fn counters_at(row: &rusqlite::Row, first: usize) -> rusqlite::Result<Counters> {
    let values: Vec<i64> =
        (0..Counters::FIELDS.len()).map(|i| row.get::<_, Option<i64>>(first + i).map(|v| v.unwrap_or(0))).collect::<Result<_, _>>()?;
    Ok(Counters::from_slice(&values))
}

/// Joined `hands h` / `player_hands ph` restriction shared by reports; binds ?1 game, ?2 since,
/// ?3 tournament id. Queries number their own parameters from ?4 on.
const FILTER_SQL: &str =
    "(?1 IS NULL OR h.game = ?1) AND (?2 IS NULL OR h.played_at >= ?2) AND (?3 IS NULL OR h.tournament_id = ?3)";

/// Lets a date or tournament filter range-scan `player_hands` by hand id instead of reading
/// every row. The bounds only narrow the scan, however ids relate to time; `FILTER_SQL` still
/// decides which hands count. Written without `OR`, which would stop SQLite from using the key.
const HAND_RANGE_SQL: &str = "ph.hand_id BETWEEN
    MAX(CASE WHEN ?2 IS NULL THEN 0 ELSE COALESCE((SELECT MIN(id) FROM hands WHERE played_at >= ?2), 0) END,
        CASE WHEN ?3 IS NULL THEN 0 ELSE COALESCE((SELECT MIN(id) FROM hands WHERE tournament_id = ?3), 0) END)
    AND CASE WHEN ?3 IS NULL THEN 9223372036854775807
             ELSE COALESCE((SELECT MAX(id) FROM hands WHERE tournament_id = ?3), -1) END";

impl Db {
    pub fn open(path: &Path) -> Result<Self, Error> {
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Self, Error> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self, Error> {
        let counter_cols: String =
            Counters::FIELDS.iter().map(|f| format!("{f} INTEGER NOT NULL DEFAULT 0")).collect::<Vec<_>>().join(", ");
        conn.execute_batch(&format!(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             CREATE TABLE IF NOT EXISTS hands (
                 id INTEGER PRIMARY KEY,
                 game TEXT NOT NULL,
                 tournament_id INTEGER,
                 table_key TEXT NOT NULL,
                 table_name TEXT NOT NULL,
                 played_at TEXT NOT NULL,
                 small_blind INTEGER NOT NULL,
                 big_blind INTEGER NOT NULL,
                 currency TEXT,
                 hero TEXT,
                 hero_cards TEXT NOT NULL DEFAULT '',
                 hero_net INTEGER,
                 raw TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS hands_table ON hands(table_key, id);
             CREATE INDEX IF NOT EXISTS hands_time ON hands(played_at, id);
             CREATE INDEX IF NOT EXISTS hands_tournament ON hands(tournament_id);
             CREATE TABLE IF NOT EXISTS player_hands (
                 hand_id INTEGER NOT NULL,
                 player TEXT NOT NULL,
                 game TEXT NOT NULL,
                 net INTEGER NOT NULL,
                 big_blind INTEGER NOT NULL,
                 {counter_cols},
                 PRIMARY KEY (hand_id, player)
             );
             CREATE INDEX IF NOT EXISTS player_hands_player ON player_hands(player, game);
             CREATE TABLE IF NOT EXISTS player_totals (
                 player TEXT NOT NULL,
                 game TEXT NOT NULL,
                 net INTEGER NOT NULL DEFAULT 0,
                 {counter_cols},
                 PRIMARY KEY (player, game)
             );
             CREATE INDEX IF NOT EXISTS player_totals_hands ON player_totals(hands);
             CREATE TABLE IF NOT EXISTS files (path TEXT PRIMARY KEY, offset INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS tournaments (
                 id INTEGER PRIMARY KEY,
                 game TEXT NOT NULL DEFAULT '',
                 currency TEXT,
                 buy_in_prize INTEGER,
                 buy_in_bounty INTEGER,
                 buy_in_fee INTEGER,
                 hand_place INTEGER,
                 hand_prize INTEGER,
                 hand_finish_id INTEGER,
                 summary INTEGER NOT NULL DEFAULT 0,
                 entrants INTEGER,
                 started_at TEXT,
                 place INTEGER,
                 prize INTEGER,
                 bounties INTEGER,
                 extra_cost INTEGER NOT NULL DEFAULT 0,
                 name TEXT
             );
             CREATE TABLE IF NOT EXISTS learn_attempts (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 kind TEXT NOT NULL,
                 level INTEGER NOT NULL,
                 seed INTEGER,
                 hand_id INTEGER,
                 answer TEXT NOT NULL,
                 score REAL NOT NULL,
                 ev_loss_bb REAL,
                 at INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS learn_attempts_kind ON learn_attempts(kind, at);
             CREATE TABLE IF NOT EXISTS coach_reports (
                 tournament_id INTEGER PRIMARY KEY,
                 created_at INTEGER NOT NULL,
                 model TEXT NOT NULL,
                 report TEXT NOT NULL
             );"
        ))?;
        let added: &[(&str, &str, &str)] = &[
            ("hands", "hero_position", "TEXT"),
            ("hands", "hero_class", "TEXT"),
            ("hands", "hero_showdown", "INTEGER NOT NULL DEFAULT 0"),
            ("hands", "hero_bounty", "INTEGER NOT NULL DEFAULT 0"),
            ("hands", "pot", "INTEGER NOT NULL DEFAULT 0"),
            ("hands", "board", "TEXT NOT NULL DEFAULT ''"),
            ("hands", "bounty_currency", "TEXT"),
            ("hands", "hero_ev", "INTEGER"),
            ("hands", "hero_stack_bb", "REAL"),
            ("player_hands", "position", "TEXT"),
            ("player_hands", "shown", "TEXT NOT NULL DEFAULT ''"),
            ("player_hands", "bounty", "INTEGER NOT NULL DEFAULT 0"),
            ("tournaments", "name", "TEXT"),
        ];
        for (table, column, decl) in added {
            let exists: bool = conn.query_row(
                &format!("SELECT COUNT(*) FROM pragma_table_info('{table}') WHERE name = ?1"),
                [column],
                |r| r.get::<_, i64>(0).map(|n| n > 0),
            )?;
            if !exists {
                conn.execute_batch(&format!("ALTER TABLE {table} ADD COLUMN {column} {decl}"))?;
            }
        }
        let db = Db { conn };
        let empty = db.hand_count()? == 0;
        if empty && db.needs_rebuild()? {
            db.conn.execute_batch(&format!("PRAGMA user_version = {SCHEMA_VERSION}"))?;
        }
        Ok(db)
    }

    /// True after an upgrade until [`Db::rebuild_step`] has gone through all stored hands.
    pub fn needs_rebuild(&self) -> Result<bool, Error> {
        let version: i32 = self.conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        Ok(version < SCHEMA_VERSION)
    }

    /// Recomputes derived data for the next `batch` hands after the last finished one, so a
    /// large database is upgraded in small transactions with bounded memory and can resume
    /// after the app was closed. Returns progress; `done == total` when finished.
    pub fn rebuild_step(&mut self, batch: usize) -> Result<RebuildProgress, Error> {
        let after: i64 = self.setting("rebuild_after_id")?.and_then(|v| v.parse().ok()).unwrap_or(-1);
        let tx = self.conn.transaction()?;
        let rows: Vec<(i64, String)> = {
            let mut stmt = tx.prepare_cached("SELECT id, raw FROM hands WHERE id > ?1 ORDER BY id LIMIT ?2")?;
            stmt.query_map(params![after, batch as i64], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_, _>>()?
        };
        for (_, raw) in &rows {
            if let Ok(hand) = parse_hand(raw) {
                write_derived(&tx, &hand, false)?;
            }
        }
        let finished = rows.len() < batch;
        if let Some((last, _)) = rows.last() {
            set_setting_in(&tx, "rebuild_after_id", &last.to_string())?;
        }
        if finished {
            recompute_totals(&tx)?;
            tx.execute("DELETE FROM settings WHERE key = 'rebuild_after_id'", [])?;
            tx.execute_batch(&format!("PRAGMA user_version = {SCHEMA_VERSION}"))?;
        }
        tx.commit()?;
        let total = self.hand_count()?;
        let done = if finished {
            total
        } else {
            let last = rows.last().map(|r| r.0).unwrap_or(after);
            self.conn.query_row("SELECT COUNT(*) FROM hands WHERE id <= ?1", [last], |r| r.get(0))?
        };
        Ok(RebuildProgress { done, total })
    }

    /// Runs [`Db::rebuild_step`] until the upgrade is complete.
    pub fn rebuild_all(&mut self) -> Result<(), Error> {
        while self.needs_rebuild()? {
            self.rebuild_step(2000)?;
        }
        Ok(())
    }

    /// Stores hands in one transaction. Returns the hands that were new.
    pub fn insert_hands<'a>(&mut self, hands: &'a [(Hand, String)]) -> Result<Vec<&'a Hand>, Error> {
        let tx = self.conn.transaction()?;
        let mut inserted = Vec::new();
        for (hand, raw) in hands {
            let changed = tx.prepare_cached(
                "INSERT OR IGNORE INTO hands (id, game, tournament_id, table_key, table_name, played_at,
                     small_blind, big_blind, currency, hero, hero_cards, hero_net, raw)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            )?
            .execute(params![
                hand.id as i64,
                hand.game.as_str(),
                hand.tournament_id.map(|t| t as i64),
                hand.table_key(),
                hand.table_name,
                hand.played_at,
                hand.small_blind,
                hand.big_blind,
                hand.currency,
                hand.hero,
                hand.hero_cards.join(" "),
                hand.hero.as_deref().map(|h| hand.net(h)),
                raw,
            ])?;
            if changed > 0 {
                write_derived(&tx, hand, true)?;
                inserted.push(hand);
            }
        }
        tx.commit()?;
        Ok(inserted)
    }

    /// Lifetime counters per player from the running totals; constant time per player. While an
    /// upgrade is still rebuilding the totals, they are summed from the per-hand rows instead.
    pub fn counters_for(&self, players: &[String], game: Option<GameKind>) -> Result<Vec<PlayerRow>, Error> {
        if self.needs_rebuild()? {
            let filter = Filter { game, ..Default::default() };
            return players
                .iter()
                .map(|p| {
                    Ok(self.player_rows(Some(p), &filter, 1, "")?.into_iter().next().unwrap_or(PlayerRow {
                        name: p.clone(),
                        counters: Counters::default(),
                        net: 0,
                    }))
                })
                .collect();
        }
        let sql = format!(
            "SELECT SUM(net), {} FROM player_totals WHERE player = ?1 AND (?2 IS NULL OR game = ?2)",
            Counters::FIELDS.iter().map(|f| format!("SUM({f})")).collect::<Vec<_>>().join(", ")
        );
        let mut stmt = self.conn.prepare_cached(&sql)?;
        players
            .iter()
            .map(|p| {
                stmt.query_row(params![p, game.map(GameKind::as_str)], |row| {
                    Ok(PlayerRow {
                        name: p.clone(),
                        net: row.get::<_, Option<i64>>(0)?.unwrap_or(0),
                        counters: counters_at(row, 1)?,
                    })
                })
            })
            .collect()
    }

    /// A player's counters in the session `hand` belongs to: the same tournament, or for cash
    /// games the same table on the same day.
    pub fn counters_in_session(&self, player: &str, hand: &Hand) -> Result<Counters, Error> {
        let sql = format!(
            "SELECT {} FROM player_hands ph JOIN hands h ON h.id = ph.hand_id
             WHERE ph.player = ?1
               AND ((?2 IS NOT NULL AND h.tournament_id = ?2)
                 OR (?2 IS NULL AND h.table_key = ?3 AND substr(h.played_at, 1, 10) = ?4))",
            sums()
        );
        self.conn.query_row(
            &sql,
            params![player, hand.tournament_id.map(|t| t as i64), hand.table_key(), hand.played_at.get(..10).unwrap_or("")],
            |row| counters_at(row, 0),
        )
    }

    /// Players ordered by hands played, optionally filtered by a name substring. Without a date
    /// or tournament filter this reads the running totals, so it stays fast with millions of hands.
    pub fn players(&self, filter: &Filter, search: &str, limit: usize) -> Result<Vec<PlayerRow>, Error> {
        if filter.since.is_some() || filter.tournament_id.is_some() {
            return self.player_rows(None, filter, limit, search);
        }
        let sql = format!(
            "SELECT player, SUM(net), {} FROM player_totals
             WHERE (?1 IS NULL OR game = ?1) AND player LIKE ?2 ESCAPE '\\'
             GROUP BY player ORDER BY SUM(hands) DESC, player LIMIT ?3",
            Counters::FIELDS.iter().map(|f| format!("SUM({f})")).collect::<Vec<_>>().join(", ")
        );
        let mut stmt = self.conn.prepare_cached(&sql)?;
        let rows = stmt.query_map(params![filter.game(), like_pattern(search), limit as i64], |row| {
            Ok(PlayerRow { name: row.get(0)?, net: row.get(1)?, counters: counters_at(row, 2)? })
        })?;
        rows.collect()
    }

    fn player_rows(
        &self,
        player: Option<&str>,
        filter: &Filter,
        limit: usize,
        search: &str,
    ) -> Result<Vec<PlayerRow>, Error> {
        let sql = format!(
            "SELECT ph.player, SUM(ph.net), {} FROM player_hands ph JOIN hands h ON h.id = ph.hand_id
             WHERE {FILTER_SQL} AND {HAND_RANGE_SQL}
               AND (?4 IS NULL OR ph.player = ?4) AND ph.player LIKE ?5 ESCAPE '\\'
             GROUP BY ph.player ORDER BY SUM(ph.hands) DESC, ph.player LIMIT ?6",
            sums()
        );
        let mut stmt = self.conn.prepare_cached(&sql)?;
        let rows = stmt.query_map(
            params![filter.game(), filter.since, filter.tournament(), player, like_pattern(search), limit as i64],
            |row| Ok(PlayerRow { name: row.get(0)?, net: row.get(1)?, counters: counters_at(row, 2)? }),
        )?;
        rows.collect()
    }

    /// Hero results grouped by position ("BTN", "CO", ...) or by starting hand ("AKs", ...).
    pub fn hero_groups(&self, filter: &Filter, by_hand: bool) -> Result<Vec<GroupRow>, Error> {
        let key = if by_hand { "h.hero_class" } else { "ph.position" };
        let sql = format!(
            "SELECT {key}, SUM(ph.net), SUM(CAST(ph.net AS REAL) / ph.big_blind), {}
             FROM player_hands ph JOIN hands h ON h.id = ph.hand_id
             WHERE ph.player = h.hero AND {key} IS NOT NULL AND {FILTER_SQL} AND {HAND_RANGE_SQL}
             GROUP BY {key}",
            sums()
        );
        let mut stmt = self.conn.prepare_cached(&sql)?;
        let rows = stmt.query_map(params![filter.game(), filter.since, filter.tournament()], |row| {
            Ok(GroupRow { key: row.get(0)?, net: row.get(1)?, net_bb: row.get(2)?, counters: counters_at(row, 3)? })
        })?;
        rows.collect()
    }

    /// Hero hands summed per game, position and stack band, for the leak finder.
    pub fn hero_cells(&self, filter: &Filter) -> Result<Vec<Cell>, Error> {
        let sql = format!(
            "SELECT h.game, ph.position, {band}, SUM(CAST(ph.net AS REAL) / ph.big_blind),
                    SUM((CAST(ph.net AS REAL) / ph.big_blind) * (CAST(ph.net AS REAL) / ph.big_blind)), {}
             FROM player_hands ph JOIN hands h ON h.id = ph.hand_id
             WHERE ph.player = h.hero AND ph.big_blind > 0 AND {FILTER_SQL} AND {HAND_RANGE_SQL}
             GROUP BY 1, 2, 3",
            sums(),
            band = leaks::band_sql("h.hero_stack_bb"),
        );
        let mut stmt = self.conn.prepare_cached(&sql)?;
        let rows = stmt.query_map(params![filter.game(), filter.since, filter.tournament()], |r| {
            Ok(Cell {
                game: GameKind::parse(&r.get::<_, String>(0)?),
                position: r.get(1)?,
                band: r.get(2)?,
                net_bb: r.get::<_, Option<f64>>(3)?.unwrap_or(0.0),
                net_bb_sq: r.get::<_, Option<f64>>(4)?.unwrap_or(0.0),
                counters: counters_at(r, 5)?,
            })
        })?;
        rows.collect()
    }

    /// Hero results in play order, streamed so memory stays flat however many hands there are.
    pub fn hero_results(&self, filter: &Filter, max_points: usize) -> Result<Results, Error> {
        let count: i64 = self.conn.query_row(
            &format!("SELECT COUNT(*) FROM hands h WHERE h.hero_net IS NOT NULL AND {FILTER_SQL}"),
            params![filter.game(), filter.since, filter.tournament()],
            |r| r.get(0),
        )?;
        let every = (count as usize).div_ceil(max_points.max(1)).max(1) as i64;
        let sql = format!(
            "SELECT h.played_at, h.hero_net, h.big_blind, h.hero_showdown, h.hero_bounty, h.bounty_currency, h.hero_ev
             FROM hands h
             WHERE h.hero_net IS NOT NULL AND {FILTER_SQL} ORDER BY h.played_at, h.id"
        );
        let mut stmt = self.conn.prepare_cached(&sql)?;
        let mut rows = stmt.query(params![filter.game(), filter.since, filter.tournament()])?;
        let mut out = Results::default();
        while let Some(r) = rows.next()? {
            let net: i64 = r.get(1)?;
            let bb: i64 = r.get(2)?;
            let net_bb = if bb > 0 { net as f64 / bb as f64 } else { 0.0 };
            out.hands += 1;
            out.net += net;
            out.net_bb += net_bb;
            if r.get::<_, i64>(3)? != 0 {
                out.showdown_bb += net_bb;
            } else {
                out.non_showdown_bb += net_bb;
            }
            add_bounty(&mut out.bounties, r.get(4)?, r.get(5)?);
            match r.get::<_, Option<i64>>(6)? {
                Some(ev) if bb > 0 => {
                    let ev_bb = ev as f64 / bb as f64;
                    out.ev_bb += ev_bb;
                    out.luck_bb += net_bb - ev_bb;
                    out.all_ins += 1;
                }
                _ => out.ev_bb += net_bb,
            }
            if out.hands % every == 0 || out.hands == count {
                out.points.push(ResultPoint {
                    n: out.hands,
                    played_at: r.get(0)?,
                    net_bb: out.net_bb,
                    showdown_bb: out.showdown_bb,
                    non_showdown_bb: out.non_showdown_bb,
                    ev_bb: out.ev_bb,
                });
            }
        }
        Ok(out)
    }

    /// Hero sessions: hands split wherever more than `gap_minutes` pass without a hand.
    pub fn sessions(&self, filter: &Filter, gap_minutes: f64) -> Result<Vec<Session>, Error> {
        let sql = format!(
            "SELECT h.played_at,
                    julianday(replace(substr(h.played_at, 1, 10), '/', '-') || ' ' || substr(h.played_at, 12)),
                    h.hero_net, CAST(h.hero_net AS REAL) / h.big_blind, h.hero_bounty, h.table_key, h.bounty_currency
             FROM hands h WHERE h.hero_net IS NOT NULL AND {FILTER_SQL} ORDER BY h.played_at, h.id"
        );
        let mut stmt = self.conn.prepare_cached(&sql)?;
        let mut rows = stmt.query(params![filter.game(), filter.since, filter.tournament()])?;
        let mut sessions: Vec<Session> = Vec::new();
        let mut tables: Vec<std::collections::BTreeSet<String>> = Vec::new();
        let mut last_day: Option<f64> = None;
        while let Some(r) = rows.next()? {
            let at: String = r.get(0)?;
            let day: f64 = r.get::<_, Option<f64>>(1)?.unwrap_or(0.0);
            let new_session = last_day.is_none_or(|prev| (day - prev) * 24.0 * 60.0 > gap_minutes);
            last_day = Some(day);
            if new_session {
                sessions.push(Session {
                    start: at.clone(),
                    end: at.clone(),
                    hands: 0,
                    net: 0,
                    net_bb: 0.0,
                    bounties: BTreeMap::new(),
                    tables: 0,
                });
                tables.push(Default::default());
            }
            let s = sessions.last_mut().expect("pushed above");
            s.end = at;
            s.hands += 1;
            s.net += r.get::<_, i64>(2)?;
            s.net_bb += r.get::<_, f64>(3)?;
            add_bounty(&mut s.bounties, r.get(4)?, r.get(6)?);
            let t = tables.last_mut().expect("pushed above");
            t.insert(r.get(5)?);
            s.tables = t.len() as i64;
        }
        sessions.reverse();
        Ok(sessions)
    }

    /// Hands where `player` showed cards, newest first.
    pub fn shown_hands(&self, player: &str, limit: usize) -> Result<Vec<ShownHand>, Error> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT h.id, h.played_at, ph.shown, h.board, ph.position, ph.net, ph.big_blind
             FROM player_hands ph JOIN hands h ON h.id = ph.hand_id
             WHERE ph.player = ?1 AND ph.shown != '' ORDER BY h.played_at DESC, h.id DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![player, limit as i64], |r| {
            Ok(ShownHand {
                hand_id: r.get::<_, i64>(0)? as u64,
                played_at: r.get(1)?,
                cards: r.get(2)?,
                board: r.get(3)?,
                position: r.get(4)?,
                net: r.get(5)?,
                big_blind: r.get(6)?,
            })
        })?;
        rows.collect()
    }

    pub fn latest_hand_for_table(&self, table_key: &str) -> Result<Option<Hand>, Error> {
        let raw: Option<String> = self
            .conn
            .query_row(
                "SELECT raw FROM hands WHERE table_key = ?1 ORDER BY id DESC LIMIT 1",
                [table_key],
                |r| r.get(0),
            )
            .optional()?;
        Ok(raw.and_then(|r| parse_hand(&r).ok()))
    }

    pub fn hand(&self, id: u64) -> Result<Option<Hand>, Error> {
        Ok(self.hand_raw(id)?.and_then(|r| parse_hand(&r).ok()))
    }

    pub fn hand_raw(&self, id: u64) -> Result<Option<String>, Error> {
        self.conn.query_row("SELECT raw FROM hands WHERE id = ?1", [id as i64], |r| r.get(0)).optional()
    }

    pub fn hands(&self, q: &HandQuery) -> Result<Vec<HandSummary>, Error> {
        let spot = match &q.spot {
            Some(f) => match f.predicate() {
                Some(pred) => format!(
                    "AND EXISTS (SELECT 1 FROM player_hands ph WHERE ph.hand_id = h.id AND ph.player = h.hero AND {pred}
                         AND (?10 = '' OR instr(?10, ',' || ph.position || ',') > 0)
                         AND (?11 IS NULL OR {} = ?11) AND (?12 IS NULL OR h.game = ?12))",
                    leaks::band_sql("h.hero_stack_bb")
                ),
                None => String::new(),
            },
            None => String::new(),
        };
        let positions = q.spot.as_ref().filter(|f| !f.positions.is_empty()).map(|f| format!(",{},", f.positions.join(",")));
        let sql = format!(
            "SELECT h.id, h.game, h.played_at, h.table_name, h.small_blind, h.big_blind, h.currency, h.hero_cards,
                    h.hero_position, h.hero_net, h.hero_showdown, h.pot, h.board
             FROM hands h
             WHERE {FILTER_SQL}
               AND (?4 IS NULL OR h.id IN (SELECT hand_id FROM player_hands WHERE player = ?4))
               AND (?5 = 0 OR h.hero_showdown = 1)
               AND (?6 IS NULL OR (?6 = 'won' AND h.hero_net > 0) OR (?6 = 'lost' AND h.hero_net < 0))
               AND (?7 IS NULL OR CAST(h.pot AS REAL) / h.big_blind >= ?7)
               {spot}
             ORDER BY h.played_at DESC, h.id DESC LIMIT ?8 OFFSET ?9"
        );
        let mut stmt = self.conn.prepare_cached(&sql)?;
        let positions = positions.unwrap_or_default();
        let band = q.spot.as_ref().and_then(|f| f.band);
        let game = q.spot.as_ref().and_then(|f| f.game).map(GameKind::as_str);
        let (kind, tournament) = (q.filter.game(), q.filter.tournament());
        let (showdown, limit, offset) = (q.showdown_only as i64, q.limit as i64, q.offset as i64);
        // ?10–?12 only appear in the SQL with a spot filter, and SQLite rejects unused bindings.
        let mut values: Vec<&dyn rusqlite::ToSql> =
            vec![&kind, &q.filter.since, &tournament, &q.player, &showdown, &q.result, &q.min_pot_bb, &limit, &offset];
        if !spot.is_empty() {
            values.extend([&positions as &dyn rusqlite::ToSql, &band, &game]);
        }
        let rows = stmt.query_map(
            values.as_slice(),
            |r| {
                let game: String = r.get(1)?;
                Ok(HandSummary {
                    id: r.get::<_, i64>(0)? as u64,
                    game: GameKind::parse(&game).unwrap_or(GameKind::Cash),
                    played_at: r.get(2)?,
                    table_name: r.get(3)?,
                    small_blind: r.get(4)?,
                    big_blind: r.get(5)?,
                    currency: r.get(6)?,
                    hero_cards: r.get(7)?,
                    hero_position: r.get(8)?,
                    hero_net: r.get(9)?,
                    hero_showdown: r.get::<_, i64>(10)? != 0,
                    pot: r.get(11)?,
                    board: r.get(12)?,
                })
            },
        )?;
        rows.collect()
    }

    /// Stores a parsed summary file. The client rewrites the file while the tournament runs, so
    /// a later version replaces the earlier one instead of adding to it.
    pub fn upsert_summary(&self, s: &Summary) -> Result<(), Error> {
        let b = s.buy_in.as_ref();
        self.conn.execute(
            "INSERT INTO tournaments (id, game, currency, buy_in_prize, buy_in_bounty, buy_in_fee, summary,
                 entrants, started_at, place, prize, bounties, extra_cost)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT(id) DO UPDATE SET
                 game = excluded.game, currency = COALESCE(excluded.currency, currency),
                 buy_in_prize = COALESCE(excluded.buy_in_prize, buy_in_prize),
                 buy_in_bounty = COALESCE(excluded.buy_in_bounty, buy_in_bounty),
                 buy_in_fee = COALESCE(excluded.buy_in_fee, buy_in_fee),
                 summary = 1, entrants = excluded.entrants, started_at = excluded.started_at,
                 place = excluded.place, prize = excluded.prize, bounties = excluded.bounties,
                 extra_cost = excluded.extra_cost",
            params![
                s.id as i64,
                s.game,
                s.currency,
                b.map(|b| b.prize),
                b.map(|b| b.bounty),
                b.map(|b| b.fee),
                s.entrants,
                s.started_at,
                s.place,
                s.prize,
                s.bounties,
                s.extra_cost,
            ],
        )?;
        Ok(())
    }

    /// The hero's tournaments, newest first. A summary file decides place, prize and bounties;
    /// without one, the hand in which the hero left the tournament does, if it was imported.
    pub fn tournaments(&self, filter: &Filter) -> Result<Vec<TournamentRow>, Error> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT t.id, t.game, t.currency, t.buy_in_prize, t.buy_in_bounty, t.buy_in_fee, t.hand_place,
                    t.hand_prize, t.summary, t.entrants, t.place, t.prize, t.bounties, t.extra_cost,
                    COALESCE(t.started_at, h.first, ''), COALESCE(h.hands, 0), COALESCE(h.chips, 0),
                    COALESCE(h.bounty, 0), t.name, t.hand_finish_id
             FROM tournaments t
             LEFT JOIN (SELECT tournament_id, MIN(played_at) AS first, COUNT(*) AS hands,
                               SUM(hero_net) AS chips, SUM(hero_bounty) AS bounty
                        FROM hands WHERE tournament_id IS NOT NULL AND hero_net IS NOT NULL
                        GROUP BY tournament_id) h ON h.tournament_id = t.id
             WHERE (t.game != '' OR t.summary = 1)
               AND (?1 IS NULL OR COALESCE(t.started_at, h.first, '') >= ?1)
             ORDER BY 15 DESC, t.id DESC",
        )?;
        let rows = stmt.query_map(params![filter.since], |r| {
            let buy_in = match (r.get::<_, Option<i64>>(3)?, r.get::<_, Option<i64>>(4)?, r.get::<_, Option<i64>>(5)?) {
                (Some(prize), bounty, fee) => Some(BuyIn {
                    prize,
                    bounty: bounty.unwrap_or(0),
                    fee: fee.unwrap_or(0),
                    currency: r.get(2)?,
                }),
                _ => None,
            };
            let has_summary = r.get::<_, i64>(8)? != 0;
            let (place, prize): (Option<i64>, Option<i64>) = match (has_summary, r.get::<_, Option<i64>>(10)?) {
                (true, Some(place)) => (Some(place), Some(r.get::<_, Option<i64>>(11)?.unwrap_or(0))),
                // The hand may name no place, but leaving without a prize is still a result.
                _ => match r.get::<_, Option<i64>>(19)? {
                    Some(_) => (r.get(6)?, Some(r.get::<_, Option<i64>>(7)?.unwrap_or(0))),
                    None => (None, None),
                },
            };
            let bounties = r.get::<_, Option<i64>>(12)?.unwrap_or(r.get(17)?);
            let extra_cost: i64 = r.get(13)?;
            let cost = buy_in.as_ref().map(|b| b.total() + extra_cost);
            Ok(TournamentRow {
                id: r.get::<_, i64>(0)? as u64,
                game: r.get(1)?,
                currency: r.get(2)?,
                buy_in,
                cost,
                entrants: r.get(9)?,
                place,
                prize,
                bounties,
                profit: match (cost, prize) {
                    (Some(c), Some(p)) => Some(p + bounties - c),
                    _ => None,
                },
                started_at: r.get(14)?,
                hands: r.get(15)?,
                chips: r.get(16)?,
                has_summary,
                name: r.get(18)?,
            })
        })?;
        rows.collect()
    }

    /// Remembers a tournament's name as shown in its table window title. Returns true when it
    /// changed. A tournament not yet known from hands gets a row, so the name is there once
    /// its first hand arrives.
    pub fn set_tournament_name(&self, id: u64, name: &str) -> Result<bool, Error> {
        let name = name.trim();
        if name.is_empty() {
            return Ok(false);
        }
        let changed = self.conn.execute(
            "INSERT INTO tournaments (id, name) VALUES (?1, ?2)
             ON CONFLICT(id) DO UPDATE SET name = excluded.name WHERE name IS NOT excluded.name",
            params![id as i64, name],
        )?;
        Ok(changed > 0)
    }

    pub fn add_attempt(&self, a: &Attempt) -> Result<(), Error> {
        self.conn.execute(
            "INSERT INTO learn_attempts (kind, level, seed, hand_id, answer, score, ev_loss_bb, at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                a.kind,
                a.level,
                a.seed.map(|s| s as i64),
                a.hand_id.map(|h| h as i64),
                a.answer,
                a.score,
                a.ev_loss_bb,
                a.at
            ],
        )?;
        Ok(())
    }

    /// Every attempt, oldest first.
    pub fn attempts(&self) -> Result<Vec<Attempt>, Error> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT kind, level, seed, hand_id, answer, score, ev_loss_bb, at FROM learn_attempts ORDER BY at, id",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(Attempt {
                kind: r.get(0)?,
                level: r.get(1)?,
                seed: r.get::<_, Option<i64>>(2)?.map(|s| s as u64),
                hand_id: r.get::<_, Option<i64>>(3)?.map(|h| h as u64),
                answer: r.get(4)?,
                score: r.get(5)?,
                ev_loss_bb: r.get(6)?,
                at: r.get(7)?,
            })
        })?;
        rows.collect()
    }

    /// Raw text of the hero's hands that may hold a learning spot (a tournament hand, where
    /// every seat can face a push/fold decision, or hero showed cards), newest first.
    pub fn learn_candidates(&self, filter: &Filter, limit: usize, offset: usize) -> Result<Vec<String>, Error> {
        let sql = format!(
            "SELECT h.raw FROM hands h
             WHERE {FILTER_SQL} AND h.hero IS NOT NULL
               AND (h.game = 'tournament'
                    OR EXISTS (SELECT 1 FROM player_hands ph
                               WHERE ph.hand_id = h.id AND ph.player = h.hero AND ph.shown != ''))
             ORDER BY h.played_at DESC, h.id DESC LIMIT ?4 OFFSET ?5"
        );
        let mut stmt = self.conn.prepare_cached(&sql)?;
        let rows = stmt.query_map(
            params![filter.game(), filter.since, filter.tournament(), limit as i64, offset as i64],
            |r| r.get(0),
        )?;
        rows.collect()
    }

    pub fn heroes(&self) -> Result<Vec<String>, Error> {
        let mut stmt = self.conn.prepare_cached("SELECT DISTINCT hero FROM hands WHERE hero IS NOT NULL")?;
        let rows = stmt.query_map([], |r| r.get(0))?;
        rows.collect()
    }

    pub fn hand_count(&self) -> Result<i64, Error> {
        self.conn.query_row("SELECT COUNT(*) FROM hands", [], |r| r.get(0))
    }

    pub fn file_offset(&self, path: &str) -> Result<u64, Error> {
        let offset: Option<i64> =
            self.conn.query_row("SELECT offset FROM files WHERE path = ?1", [path], |r| r.get(0)).optional()?;
        Ok(offset.unwrap_or(0) as u64)
    }

    pub fn set_file_offset(&self, path: &str, offset: u64) -> Result<(), Error> {
        self.conn.execute(
            "INSERT INTO files (path, offset) VALUES (?1, ?2)
             ON CONFLICT(path) DO UPDATE SET offset = excluded.offset",
            params![path, offset as i64],
        )?;
        Ok(())
    }

    pub fn setting(&self, key: &str) -> Result<Option<String>, Error> {
        self.conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| r.get(0)).optional()
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), Error> {
        self.conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    /// Raw text of every hand of a tournament, in the order played.
    pub fn tournament_raw_hands(&self, tournament_id: u64) -> Result<Vec<String>, Error> {
        let mut stmt =
            self.conn.prepare_cached("SELECT raw FROM hands WHERE tournament_id = ?1 ORDER BY played_at, id")?;
        let rows = stmt.query_map([tournament_id as i64], |r| r.get(0))?;
        rows.collect()
    }

    /// Stores the coach's report (JSON) for a tournament, replacing an older one.
    pub fn save_coach_report(&self, tournament_id: u64, created_at: i64, model: &str, json: &str) -> Result<(), Error> {
        self.conn.execute(
            "INSERT INTO coach_reports (tournament_id, created_at, model, report) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(tournament_id) DO UPDATE SET created_at = excluded.created_at, model = excluded.model,
                                                      report = excluded.report",
            params![tournament_id as i64, created_at, model, json],
        )?;
        Ok(())
    }

    pub fn coach_report(&self, tournament_id: u64) -> Result<Option<String>, Error> {
        self.conn
            .query_row("SELECT report FROM coach_reports WHERE tournament_id = ?1", [tournament_id as i64], |r| r.get(0))
            .optional()
    }

    /// Every stored report (JSON), newest first.
    pub fn coach_reports(&self) -> Result<Vec<String>, Error> {
        let mut stmt = self.conn.prepare_cached("SELECT report FROM coach_reports ORDER BY created_at DESC, tournament_id DESC")?;
        let rows = stmt.query_map([], |r| r.get(0))?;
        rows.collect()
    }

    /// Returns whether there was a report to delete.
    pub fn delete_coach_report(&self, tournament_id: u64) -> Result<bool, Error> {
        Ok(self.conn.execute("DELETE FROM coach_reports WHERE tournament_id = ?1", [tournament_id as i64])? > 0)
    }
}

fn add_bounty(totals: &mut BTreeMap<String, i64>, amount: i64, currency: Option<String>) {
    if amount != 0 {
        *totals.entry(currency.unwrap_or_else(|| "USD".into())).or_default() += amount;
    }
}

fn like_pattern(search: &str) -> String {
    let escaped = search.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
    format!("%{escaped}%")
}

fn set_setting_in(tx: &Transaction, key: &str, value: &str) -> Result<(), Error> {
    tx.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

fn recompute_totals(tx: &Transaction) -> Result<(), Error> {
    let cols = Counters::FIELDS.join(", ");
    let sums = Counters::FIELDS.iter().map(|f| format!("SUM({f})")).collect::<Vec<_>>().join(", ");
    tx.execute("DELETE FROM player_totals", [])?;
    tx.execute(
        &format!(
            "INSERT INTO player_totals (player, game, net, {cols})
             SELECT player, game, SUM(net), {sums} FROM player_hands GROUP BY player, game"
        ),
        [],
    )?;
    Ok(())
}

/// Everything computed from a parsed hand: hero summary columns and one row per player.
/// `add_totals` adds the hand to the running per-player totals (new hands only; a rebuild
/// recomputes the totals once at the end instead).
fn write_derived(tx: &Transaction, hand: &Hand, add_totals: bool) -> Result<(), Error> {
    let positions = position_labels(hand);
    let position_of = |name: &str| positions.iter().find(|(n, _)| n == name).map(|(_, p)| *p);
    let bounty_of = |name: &str| hand.bounties_won.iter().filter(|(n, _)| n == name).map(|(_, a)| a).sum::<i64>();
    let counters = analyze(hand);
    let hero = hand.hero.as_deref();
    let hero_showdown = hero
        .and_then(|h| counters.iter().find(|(n, _)| n == h))
        .is_some_and(|(_, c)| c.wtsd > 0);
    let hero_ev = hero_all_in(hand).map(|a| a.ev.round() as i64);

    tx.prepare_cached(
        "UPDATE hands SET hero_position = ?2, hero_class = ?3, hero_showdown = ?4, hero_bounty = ?5,
             pot = ?6, board = ?7, bounty_currency = ?8, played_at = ?9, hero_net = ?10, hero_ev = ?11,
             hero_stack_bb = ?12 WHERE id = ?1",
    )?
    .execute(params![
        hand.id as i64,
        hero.and_then(position_of),
        hand_class(&hand.hero_cards),
        hero_showdown as i64,
        hero.map(bounty_of).unwrap_or(0),
        hand.total_pot,
        hand.board.join(" "),
        hand.bounty_currency,
        hand.played_at,
        hero.map(|h| hand.net(h)),
        hero_ev,
        hero.and_then(|h| effective_stack_bb(hand, h)),
    ])?;

    if let Some(tid) = hand.tournament_id {
        write_tournament(tx, tid, hand)?;
    }

    let placeholders = vec!["?"; Counters::FIELDS.len() + 8].join(", ");
    let mut stmt = tx.prepare_cached(&format!(
        "INSERT OR REPLACE INTO player_hands (hand_id, player, game, net, big_blind, position, shown, bounty, {})
         VALUES ({placeholders})",
        Counters::FIELDS.join(", ")
    ))?;
    for (player, c) in counters {
        let shown = hand.shown.iter().find(|(n, _)| *n == player).map(|(_, cards)| cards.join(" ")).unwrap_or_default();
        let mut values: Vec<rusqlite::types::Value> = vec![
            (hand.id as i64).into(),
            player.clone().into(),
            hand.game.as_str().to_string().into(),
            hand.net(&player).into(),
            hand.big_blind.into(),
            position_of(&player).map(String::from).into(),
            shown.into(),
            bounty_of(&player).into(),
        ];
        values.extend(c.to_vec().into_iter().map(Into::into));
        stmt.execute(params_from_iter(values))?;
        if add_totals {
            let cols = Counters::FIELDS.join(", ");
            let updates =
                Counters::FIELDS.iter().map(|f| format!("{f} = {f} + excluded.{f}")).collect::<Vec<_>>().join(", ");
            let mut totals = tx.prepare_cached(&format!(
                "INSERT INTO player_totals (player, game, net, {cols}) VALUES ({})
                 ON CONFLICT(player, game) DO UPDATE SET net = net + excluded.net, {updates}",
                vec!["?"; Counters::FIELDS.len() + 3].join(", ")
            ))?;
            let mut values: Vec<rusqlite::types::Value> =
                vec![player.clone().into(), hand.game.as_str().to_string().into(), hand.net(&player).into()];
            values.extend(c.to_vec().into_iter().map(Into::into));
            totals.execute(params_from_iter(values))?;
        }
    }
    Ok(())
}

/// The hero's stack against the biggest other stack dealt in, in big blinds.
fn effective_stack_bb(hand: &Hand, hero: &str) -> Option<f64> {
    if hand.big_blind <= 0 {
        return None;
    }
    let own = hand.seats.iter().find(|s| s.name == hero)?.stack;
    let other = hand.active_players().filter(|s| s.name != hero).map(|s| s.stack).max()?;
    Some(own.min(other) as f64 / hand.big_blind as f64)
}

/// What a tournament hand tells about its tournament. Idempotent, so rebuilding or importing a
/// hand twice changes nothing; values from a summary file are never overwritten.
fn write_tournament(tx: &Transaction, tid: u64, hand: &Hand) -> Result<(), Error> {
    let b = hand.buy_in.as_ref();
    tx.prepare_cached(
        "INSERT INTO tournaments (id, game, currency, buy_in_prize, buy_in_bounty, buy_in_fee)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(id) DO UPDATE SET
             game = CASE WHEN game = '' THEN excluded.game ELSE game END,
             currency = COALESCE(currency, excluded.currency),
             buy_in_prize = COALESCE(buy_in_prize, excluded.buy_in_prize),
             buy_in_bounty = COALESCE(buy_in_bounty, excluded.buy_in_bounty),
             buy_in_fee = COALESCE(buy_in_fee, excluded.buy_in_fee)",
    )?
    .execute(params![
        tid as i64,
        crate::tournament::game_label(&hand.variant),
        b.and_then(|b| b.currency.clone()),
        b.map(|b| b.prize),
        b.map(|b| b.bounty),
        b.map(|b| b.fee),
    ])?;
    let hero = hand.hero.as_deref();
    if let Some(f) = hand.finishes.iter().find(|f| Some(f.player.as_str()) == hero) {
        // After a re-entry the hero can leave twice; the later hand has the final place.
        tx.prepare_cached(
            "UPDATE tournaments SET hand_place = ?2, hand_prize = ?3, hand_finish_id = ?4,
                 currency = COALESCE(currency, ?5)
             WHERE id = ?1 AND (hand_finish_id IS NULL OR hand_finish_id <= ?4)",
        )?
        .execute(params![tid as i64, f.place, f.prize, hand.id as i64, f.currency])?;
    }
    Ok(())
}
