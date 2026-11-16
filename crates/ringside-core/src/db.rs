//! Local SQLite store. Raw hand text is kept so the replayer and future stats can re-parse.

use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params, params_from_iter};
use serde::Serialize;

use crate::model::{GameKind, Hand};
use crate::parser::parse_hand;
use crate::stats::{Counters, analyze};

pub use rusqlite::Error;

pub struct Db {
    conn: Connection,
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
    pub hero_net: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResultPoint {
    pub hand_id: u64,
    pub net: i64,
    pub big_blind: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlayerRow {
    pub name: String,
    pub counters: Counters,
    pub net: i64,
}

fn game_filter(game: Option<GameKind>) -> Option<&'static str> {
    game.map(GameKind::as_str)
}

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
             CREATE TABLE IF NOT EXISTS files (path TEXT PRIMARY KEY, offset INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);"
        ))?;
        Ok(Db { conn })
    }

    /// Stores hands in one transaction. Returns the hands that were new.
    pub fn insert_hands<'a>(&mut self, hands: &'a [(Hand, String)]) -> Result<Vec<&'a Hand>, Error> {
        let tx = self.conn.transaction()?;
        let mut inserted = Vec::new();
        {
            let mut hand_stmt = tx.prepare_cached(
                "INSERT OR IGNORE INTO hands (id, game, tournament_id, table_key, table_name, played_at,
                     small_blind, big_blind, currency, hero, hero_cards, hero_net, raw)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            )?;
            let placeholders = vec!["?"; Counters::FIELDS.len() + 5].join(", ");
            let mut player_stmt = tx.prepare_cached(&format!(
                "INSERT OR IGNORE INTO player_hands (hand_id, player, game, net, big_blind, {})
                 VALUES ({placeholders})",
                Counters::FIELDS.join(", ")
            ))?;
            for (hand, raw) in hands {
                let hero_net = hand.hero.as_deref().map(|h| hand.net(h));
                let changed = hand_stmt.execute(params![
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
                    hero_net,
                    raw,
                ])?;
                if changed == 0 {
                    continue;
                }
                for (player, counters) in analyze(hand) {
                    let mut values: Vec<rusqlite::types::Value> = vec![
                        (hand.id as i64).into(),
                        player.clone().into(),
                        hand.game.as_str().to_string().into(),
                        hand.net(&player).into(),
                        hand.big_blind.into(),
                    ];
                    values.extend(counters.to_vec().into_iter().map(Into::into));
                    player_stmt.execute(params_from_iter(values))?;
                }
                inserted.push(hand);
            }
        }
        tx.commit()?;
        Ok(inserted)
    }

    pub fn counters_for(&self, players: &[String], game: Option<GameKind>) -> Result<Vec<PlayerRow>, Error> {
        players
            .iter()
            .map(|p| {
                Ok(self.player_rows(Some(p), game, 1, "")?.into_iter().next().unwrap_or(PlayerRow {
                    name: p.clone(),
                    counters: Counters::default(),
                    net: 0,
                }))
            })
            .collect()
    }

    /// Players ordered by hands played, optionally filtered by a name substring.
    pub fn players(&self, game: Option<GameKind>, search: &str, limit: usize) -> Result<Vec<PlayerRow>, Error> {
        self.player_rows(None, game, limit, search)
    }

    fn player_rows(
        &self,
        player: Option<&str>,
        game: Option<GameKind>,
        limit: usize,
        search: &str,
    ) -> Result<Vec<PlayerRow>, Error> {
        let sums: String = Counters::FIELDS.iter().map(|f| format!("SUM({f})")).collect::<Vec<_>>().join(", ");
        let sql = format!(
            "SELECT player, SUM(net), {sums} FROM player_hands
             WHERE (?1 IS NULL OR player = ?1) AND (?2 IS NULL OR game = ?2) AND player LIKE ?3 ESCAPE '\\'
             GROUP BY player ORDER BY SUM(hands) DESC LIMIT ?4"
        );
        let mut stmt = self.conn.prepare_cached(&sql)?;
        let escaped = search.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
        let pattern = format!("%{escaped}%");
        let rows = stmt.query_map(params![player, game_filter(game), pattern, limit as i64], |row| {
            let values: Vec<i64> =
                (0..Counters::FIELDS.len()).map(|i| row.get(i + 2)).collect::<Result<_, _>>()?;
            Ok(PlayerRow { name: row.get(0)?, net: row.get(1)?, counters: Counters::from_slice(&values) })
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
        let raw: Option<String> =
            self.conn.query_row("SELECT raw FROM hands WHERE id = ?1", [id as i64], |r| r.get(0)).optional()?;
        Ok(raw.and_then(|r| parse_hand(&r).ok()))
    }

    pub fn hand_raw(&self, id: u64) -> Result<Option<String>, Error> {
        self.conn.query_row("SELECT raw FROM hands WHERE id = ?1", [id as i64], |r| r.get(0)).optional()
    }

    pub fn hands(
        &self,
        game: Option<GameKind>,
        player: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<HandSummary>, Error> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT id, game, played_at, table_name, small_blind, big_blind, currency, hero_cards, hero_net
             FROM hands
             WHERE (?1 IS NULL OR game = ?1)
               AND (?2 IS NULL OR id IN (SELECT hand_id FROM player_hands WHERE player = ?2))
             ORDER BY played_at DESC, id DESC LIMIT ?3 OFFSET ?4",
        )?;
        let rows = stmt.query_map(
            params![game_filter(game), player, limit as i64, offset as i64],
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
                    hero_net: r.get(8)?,
                })
            },
        )?;
        rows.collect()
    }

    /// Hero result per hand in play order, for the results graph.
    pub fn hero_results(&self, game: Option<GameKind>) -> Result<Vec<ResultPoint>, Error> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT id, hero_net, big_blind FROM hands
             WHERE hero_net IS NOT NULL AND (?1 IS NULL OR game = ?1)
             ORDER BY played_at, id",
        )?;
        let rows = stmt.query_map(params![game_filter(game)], |r| {
            Ok(ResultPoint { hand_id: r.get::<_, i64>(0)? as u64, net: r.get(1)?, big_blind: r.get(2)? })
        })?;
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
