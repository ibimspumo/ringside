//! Local SQLite store. Raw hand text is kept so derived data can be rebuilt after upgrades.

use std::collections::BTreeMap;
use std::path::Path;

use rusqlite::{Connection, OptionalExtension, Transaction, params, params_from_iter};
use serde::{Deserialize, Serialize};

use crate::model::{GameKind, Hand};
use crate::parser::parse_hand;
use crate::stats::{Counters, analyze, hand_class, position_labels};

pub use rusqlite::Error;

/// Bump when derived columns or counters change; older databases are rebuilt from raw hands.
const SCHEMA_VERSION: i32 = 2;

pub struct Db {
    conn: Connection,
}

/// Restricts reports to a kind of game and a start date ("YYYY/MM/DD").
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Filter {
    pub game: Option<GameKind>,
    pub since: Option<String>,
}

impl Filter {
    fn game(&self) -> Option<&'static str> {
        self.game.map(GameKind::as_str)
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
    pub limit: usize,
    pub offset: usize,
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

/// Joined `hands h` / `player_hands ph` restriction shared by reports; binds ?1 game, ?2 since.
const FILTER_SQL: &str = "(?1 IS NULL OR h.game = ?1) AND (?2 IS NULL OR h.played_at >= ?2)";

/// Lets a date filter range-scan `player_hands` by hand id instead of reading every row. The
/// smallest id played since the date is a lower bound however ids relate to time, so this
/// only narrows the scan; `FILTER_SQL` still decides which hands count.
const SINCE_RANGE_SQL: &str =
    "(?2 IS NULL OR ph.hand_id >= (SELECT COALESCE(MIN(id), 0) FROM hands WHERE played_at >= ?2))";

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
             CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);"
        ))?;
        let added: &[(&str, &str, &str)] = &[
            ("hands", "hero_position", "TEXT"),
            ("hands", "hero_class", "TEXT"),
            ("hands", "hero_showdown", "INTEGER NOT NULL DEFAULT 0"),
            ("hands", "hero_bounty", "INTEGER NOT NULL DEFAULT 0"),
            ("hands", "pot", "INTEGER NOT NULL DEFAULT 0"),
            ("hands", "board", "TEXT NOT NULL DEFAULT ''"),
            ("hands", "bounty_currency", "TEXT"),
            ("player_hands", "position", "TEXT"),
            ("player_hands", "shown", "TEXT NOT NULL DEFAULT ''"),
            ("player_hands", "bounty", "INTEGER NOT NULL DEFAULT 0"),
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
            let filter = Filter { game, since: None };
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
    /// filter this reads the running totals, so it stays fast with millions of hands.
    pub fn players(&self, filter: &Filter, search: &str, limit: usize) -> Result<Vec<PlayerRow>, Error> {
        if filter.since.is_some() {
            return self.player_rows(None, filter, limit, search);
        }
        let sql = format!(
            "SELECT player, SUM(net), {} FROM player_totals
             WHERE (?1 IS NULL OR game = ?1) AND player LIKE ?2 ESCAPE '\\'
             GROUP BY player ORDER BY SUM(hands) DESC LIMIT ?3",
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
             WHERE {FILTER_SQL} AND {SINCE_RANGE_SQL}
               AND (?3 IS NULL OR ph.player = ?3) AND ph.player LIKE ?4 ESCAPE '\\'
             GROUP BY ph.player ORDER BY SUM(ph.hands) DESC LIMIT ?5",
            sums()
        );
        let mut stmt = self.conn.prepare_cached(&sql)?;
        let rows = stmt.query_map(
            params![filter.game(), filter.since, player, like_pattern(search), limit as i64],
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
             WHERE ph.player = h.hero AND {key} IS NOT NULL AND {FILTER_SQL}
             GROUP BY {key}",
            sums()
        );
        let mut stmt = self.conn.prepare_cached(&sql)?;
        let rows = stmt.query_map(params![filter.game(), filter.since], |row| {
            Ok(GroupRow { key: row.get(0)?, net: row.get(1)?, net_bb: row.get(2)?, counters: counters_at(row, 3)? })
        })?;
        rows.collect()
    }

    /// Hero results in play order, streamed so memory stays flat however many hands there are.
    pub fn hero_results(&self, filter: &Filter, max_points: usize) -> Result<Results, Error> {
        let count: i64 = self.conn.query_row(
            &format!("SELECT COUNT(*) FROM hands h WHERE h.hero_net IS NOT NULL AND {FILTER_SQL}"),
            params![filter.game(), filter.since],
            |r| r.get(0),
        )?;
        let every = (count as usize).div_ceil(max_points.max(1)).max(1) as i64;
        let sql = format!(
            "SELECT h.played_at, h.hero_net, h.big_blind, h.hero_showdown, h.hero_bounty, h.bounty_currency FROM hands h
             WHERE h.hero_net IS NOT NULL AND {FILTER_SQL} ORDER BY h.played_at, h.id"
        );
        let mut stmt = self.conn.prepare_cached(&sql)?;
        let mut rows = stmt.query(params![filter.game(), filter.since])?;
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
            if out.hands % every == 0 || out.hands == count {
                out.points.push(ResultPoint {
                    n: out.hands,
                    played_at: r.get(0)?,
                    net_bb: out.net_bb,
                    showdown_bb: out.showdown_bb,
                    non_showdown_bb: out.non_showdown_bb,
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
        let mut rows = stmt.query(params![filter.game(), filter.since])?;
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
        let sql = format!(
            "SELECT h.id, h.game, h.played_at, h.table_name, h.small_blind, h.big_blind, h.currency, h.hero_cards,
                    h.hero_position, h.hero_net, h.hero_showdown, h.pot, h.board
             FROM hands h
             WHERE {FILTER_SQL}
               AND (?3 IS NULL OR h.id IN (SELECT hand_id FROM player_hands WHERE player = ?3))
               AND (?4 = 0 OR h.hero_showdown = 1)
               AND (?5 IS NULL OR (?5 = 'won' AND h.hero_net > 0) OR (?5 = 'lost' AND h.hero_net < 0))
               AND (?6 IS NULL OR CAST(h.pot AS REAL) / h.big_blind >= ?6)
             ORDER BY h.played_at DESC, h.id DESC LIMIT ?7 OFFSET ?8"
        );
        let mut stmt = self.conn.prepare_cached(&sql)?;
        let rows = stmt.query_map(
            params![
                q.filter.game(),
                q.filter.since,
                q.player,
                q.showdown_only as i64,
                q.result,
                q.min_pot_bb,
                q.limit as i64,
                q.offset as i64
            ],
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

    tx.prepare_cached(
        "UPDATE hands SET hero_position = ?2, hero_class = ?3, hero_showdown = ?4, hero_bounty = ?5,
             pot = ?6, board = ?7, bounty_currency = ?8 WHERE id = ?1",
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
    ])?;

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
