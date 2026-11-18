//! The index of the spot store: one row per stored solution file, whatever its format
//! and origin, under the spot's key (`ringside_store::key`). The files are written by
//! the app's store, which is the only place that reads and writes them.

use rusqlite::{OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};

use super::{Db, Error};

pub(super) const SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS spot_store (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        key TEXT NOT NULL,
        local_key TEXT NOT NULL,
        origin TEXT NOT NULL,
        format TEXT NOT NULL,
        file TEXT NOT NULL UNIQUE,
        board TEXT NOT NULL,
        input TEXT,
        exploitability_pct REAL NOT NULL,
        complete INTEGER NOT NULL,
        bytes INTEGER NOT NULL,
        solution_id INTEGER,
        created_at INTEGER NOT NULL
    );
    CREATE INDEX IF NOT EXISTS spot_store_key ON spot_store(key);
    CREATE INDEX IF NOT EXISTS spot_store_local ON spot_store(local_key);";

/// How a stored file is written.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StoreFormat {
    /// A snapshot of any spot (`ringside_solver::saved`), described in `solver_solutions`.
    Snapshot,
    /// The library format (`ringside_solver::library::format`): catalog spots and model
    /// flops from the cloud.
    Library,
}

/// Where a stored solution came from (`ringside_store::Origin`, as text).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StoreRow {
    pub id: i64,
    pub key: String,
    pub local_key: String,
    /// "own", "library" or "cloud".
    pub origin: String,
    pub format: StoreFormat,
    /// Path inside the app's data folder, with forward slashes.
    pub file: String,
    /// The board the file is stored on.
    pub board: String,
    /// The cloud input of a model flop, JSON; the library format needs it to be read.
    pub input: Option<String>,
    pub exploitability_pct: f64,
    pub complete: bool,
    pub bytes: u64,
    /// The row in `solver_solutions` of a snapshot.
    pub solution_id: Option<i64>,
    pub created_at: i64,
}

const COLUMNS: &str =
    "id, key, local_key, origin, format, file, board, input, exploitability_pct, complete, bytes, solution_id, created_at";

fn read(r: &Row) -> Result<StoreRow, Error> {
    Ok(StoreRow {
        id: r.get(0)?,
        key: r.get(1)?,
        local_key: r.get(2)?,
        origin: r.get(3)?,
        format: if r.get::<_, String>(4)? == "library" { StoreFormat::Library } else { StoreFormat::Snapshot },
        file: r.get(5)?,
        board: r.get(6)?,
        input: r.get(7)?,
        exploitability_pct: r.get(8)?,
        complete: r.get::<_, i64>(9)? != 0,
        bytes: r.get::<_, i64>(10)? as u64,
        solution_id: r.get(11)?,
        created_at: r.get(12)?,
    })
}

impl Db {
    /// Adds the row, or replaces the one of the same file; returns its id.
    pub fn store_put(&self, row: &StoreRow) -> Result<i64, Error> {
        self.conn.execute(
            "INSERT INTO spot_store (key, local_key, origin, format, file, board, input, exploitability_pct, complete, bytes,
                 solution_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT(file) DO UPDATE SET key = ?1, local_key = ?2, origin = ?3, format = ?4, board = ?6, input = ?7,
                 exploitability_pct = ?8, complete = ?9, bytes = ?10, solution_id = ?11, created_at = ?12",
            params![
                row.key,
                row.local_key,
                row.origin,
                match row.format {
                    StoreFormat::Snapshot => "snapshot",
                    StoreFormat::Library => "library",
                },
                row.file,
                row.board,
                row.input,
                row.exploitability_pct,
                row.complete as i64,
                row.bytes as i64,
                row.solution_id,
                row.created_at,
            ],
        )?;
        self.conn.query_row("SELECT id FROM spot_store WHERE file = ?1", [&row.file], |r| r.get(0))
    }

    /// Rows filed under `key` or, for callers without the model line, under `local_key`:
    /// the most exact first, the newest among equals.
    pub fn store_find(&self, key: &str, local_key: &str) -> Result<Vec<StoreRow>, Error> {
        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT {COLUMNS} FROM spot_store WHERE key = ?1 OR local_key = ?2 OR key = ?2
             ORDER BY exploitability_pct, created_at DESC, id DESC"
        ))?;
        stmt.query_map([key, local_key], read)?.collect()
    }

    /// Every row of `origin`, newest first; all rows without one.
    pub fn store_rows(&self, origin: Option<&str>) -> Result<Vec<StoreRow>, Error> {
        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT {COLUMNS} FROM spot_store WHERE ?1 IS NULL OR origin = ?1 ORDER BY created_at DESC, id DESC"
        ))?;
        stmt.query_map([origin], read)?.collect()
    }

    pub fn store_row_of_solution(&self, solution_id: i64) -> Result<Option<StoreRow>, Error> {
        self.conn
            .query_row(&format!("SELECT {COLUMNS} FROM spot_store WHERE solution_id = ?1"), [solution_id], read)
            .optional()
    }

    pub fn store_row(&self, id: i64) -> Result<Option<StoreRow>, Error> {
        self.conn.query_row(&format!("SELECT {COLUMNS} FROM spot_store WHERE id = ?1"), [id], read).optional()
    }

    pub fn store_delete_file(&self, file: &str) -> Result<(), Error> {
        self.conn.execute("DELETE FROM spot_store WHERE file = ?1", [file])?;
        Ok(())
    }

    /// Rows per origin.
    pub fn store_counts(&self) -> Result<Vec<(String, i64)>, Error> {
        let mut stmt = self.conn.prepare_cached("SELECT origin, COUNT(*) FROM spot_store GROUP BY origin ORDER BY origin")?;
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(file: &str, key: &str, local: &str, expl: f64, at: i64) -> StoreRow {
        StoreRow {
            id: 0,
            key: key.into(),
            local_key: local.into(),
            origin: "own".into(),
            format: StoreFormat::Snapshot,
            file: file.into(),
            board: "Ks7h2d".into(),
            input: None,
            exploitability_pct: expl,
            complete: false,
            bytes: 10,
            solution_id: Some(1),
            created_at: at,
        }
    }

    #[test]
    fn rows_are_found_by_key_or_by_their_local_key() {
        let db = Db::open_in_memory().unwrap();
        let a = db.store_put(&row("solutions/a.rsms", "spot:x", "local:a", 0.5, 1)).unwrap();
        let b = db.store_put(&row("solutions/b.rsms", "local:b", "local:b", 0.3, 2)).unwrap();
        let ids = |rows: Vec<StoreRow>| rows.iter().map(|r| r.id).collect::<Vec<_>>();
        assert_eq!(ids(db.store_find("spot:x", "local:zzz").unwrap()), [a]);
        assert_eq!(ids(db.store_find("spot:y", "local:a").unwrap()), [a], "a caller without the model line");
        assert_eq!(ids(db.store_find("local:b", "local:b").unwrap()), [b]);
        // The same file again replaces its row.
        let again = db.store_put(&StoreRow { origin: "cloud".into(), ..row("solutions/a.rsms", "spot:x", "local:a", 0.4, 3) }).unwrap();
        assert_eq!(again, a);
        assert_eq!(db.store_rows(Some("cloud")).unwrap().len(), 1);
        assert_eq!(db.store_counts().unwrap(), [("cloud".to_string(), 1), ("own".to_string(), 1)]);
        db.store_delete_file("solutions/a.rsms").unwrap();
        assert!(db.store_find("spot:x", "local:a").unwrap().is_empty());
    }
}
