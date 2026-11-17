//! Solver solutions saved on this computer: one row per file, so the list, reuse and the
//! disk limit work without opening any file. The files themselves are written by the app.

use rusqlite::{OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};

use super::{Db, Error};

pub(super) const SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS solver_solutions (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        spot_key TEXT NOT NULL,
        title TEXT NOT NULL,
        board TEXT NOT NULL,
        oop TEXT NOT NULL,
        ip TEXT NOT NULL,
        pot_bb REAL NOT NULL,
        stack_bb REAL NOT NULL,
        ranges TEXT NOT NULL,
        sizes TEXT NOT NULL,
        exploitability_pct REAL NOT NULL,
        target_pct REAL NOT NULL,
        complete INTEGER NOT NULL,
        source TEXT NOT NULL,
        hand_id INTEGER,
        file TEXT NOT NULL,
        bytes INTEGER NOT NULL,
        pinned INTEGER NOT NULL DEFAULT 0,
        created_at INTEGER NOT NULL
    );
    CREATE INDEX IF NOT EXISTS solver_solutions_key ON solver_solutions(spot_key);";

/// Where a saved solution came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SolutionSource {
    /// Set up and solved on the solver page.
    Own,
    /// A spot from a played hand (replayer, hand analysis).
    Hand,
    /// A library spot solved again, deeper.
    Library,
    /// Solved ahead for the endless trainer.
    Training,
}

impl SolutionSource {
    fn as_str(self) -> &'static str {
        match self {
            SolutionSource::Own => "own",
            SolutionSource::Hand => "hand",
            SolutionSource::Library => "library",
            SolutionSource::Training => "training",
        }
    }

    fn parse(s: &str) -> SolutionSource {
        match s {
            "hand" => SolutionSource::Hand,
            "library" => SolutionSource::Library,
            "training" => SolutionSource::Training,
            _ => SolutionSource::Own,
        }
    }
}

/// What is known about a saved solution before its file is opened.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NewSolution {
    /// `ringside_solver::saved::spot_key`: equal for spots that solve to the same tree.
    pub spot_key: String,
    /// The spot in plain words, e.g. "BTN gegen BB, Single-Raised-Pot".
    pub title: String,
    pub board: String,
    /// Positions out of and in position.
    pub oop: String,
    pub ip: String,
    pub pot_bb: f64,
    pub stack_bb: f64,
    /// Where the ranges came from, in plain words.
    pub ranges: String,
    /// The bet sizes in plain words.
    pub sizes: String,
    pub exploitability_pct: f64,
    pub target_pct: f64,
    /// Every node is stored (river spots); otherwise deeper streets are solved again.
    pub complete: bool,
    pub source: SolutionSource,
    pub hand_id: Option<u64>,
    /// File name inside the app's solutions folder.
    pub file: String,
    pub bytes: u64,
    pub created_at: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SavedSolution {
    pub id: i64,
    #[serde(flatten)]
    pub info: NewSolution,
    pub pinned: bool,
}

const COLUMNS: &str = "id, spot_key, title, board, oop, ip, pot_bb, stack_bb, ranges, sizes, exploitability_pct,
    target_pct, complete, source, hand_id, file, bytes, pinned, created_at";

fn read(r: &Row) -> Result<SavedSolution, Error> {
    Ok(SavedSolution {
        id: r.get(0)?,
        info: NewSolution {
            spot_key: r.get(1)?,
            title: r.get(2)?,
            board: r.get(3)?,
            oop: r.get(4)?,
            ip: r.get(5)?,
            pot_bb: r.get(6)?,
            stack_bb: r.get(7)?,
            ranges: r.get(8)?,
            sizes: r.get(9)?,
            exploitability_pct: r.get(10)?,
            target_pct: r.get(11)?,
            complete: r.get::<_, i64>(12)? != 0,
            source: SolutionSource::parse(&r.get::<_, String>(13)?),
            hand_id: r.get::<_, Option<i64>>(14)?.map(|h| h as u64),
            file: r.get(15)?,
            bytes: r.get::<_, i64>(16)? as u64,
            created_at: r.get(18)?,
        },
        pinned: r.get::<_, i64>(17)? != 0,
    })
}

impl Db {
    pub fn add_solution(&self, s: &NewSolution) -> Result<i64, Error> {
        self.conn.execute(
            "INSERT INTO solver_solutions (spot_key, title, board, oop, ip, pot_bb, stack_bb, ranges, sizes,
                 exploitability_pct, target_pct, complete, source, hand_id, file, bytes, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
            params![
                s.spot_key,
                s.title,
                s.board,
                s.oop,
                s.ip,
                s.pot_bb,
                s.stack_bb,
                s.ranges,
                s.sizes,
                s.exploitability_pct,
                s.target_pct,
                s.complete as i64,
                s.source.as_str(),
                s.hand_id.map(|h| h as i64),
                s.file,
                s.bytes as i64,
                s.created_at,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Newest first.
    pub fn solutions(&self) -> Result<Vec<SavedSolution>, Error> {
        let mut stmt = self.conn.prepare_cached(&format!("SELECT {COLUMNS} FROM solver_solutions ORDER BY created_at DESC, id DESC"))?;
        stmt.query_map([], read)?.collect()
    }

    pub fn solution(&self, id: i64) -> Result<Option<SavedSolution>, Error> {
        self.conn
            .query_row(&format!("SELECT {COLUMNS} FROM solver_solutions WHERE id = ?1"), [id], read)
            .optional()
    }

    /// Saved solutions of the same spot, the most exact first.
    pub fn solutions_for_key(&self, spot_key: &str) -> Result<Vec<SavedSolution>, Error> {
        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT {COLUMNS} FROM solver_solutions WHERE spot_key = ?1 ORDER BY exploitability_pct, created_at DESC"
        ))?;
        stmt.query_map([spot_key], read)?.collect()
    }

    /// Returns false when there is no such solution.
    pub fn set_solution_pinned(&self, id: i64, pinned: bool) -> Result<bool, Error> {
        Ok(self.conn.execute("UPDATE solver_solutions SET pinned = ?2 WHERE id = ?1", params![id, pinned as i64])? > 0)
    }

    /// Removes the row and returns its file name, which the caller deletes.
    pub fn delete_solution(&self, id: i64) -> Result<Option<String>, Error> {
        let file = self.conn.query_row("SELECT file FROM solver_solutions WHERE id = ?1", [id], |r| r.get(0)).optional()?;
        self.conn.execute("DELETE FROM solver_solutions WHERE id = ?1", [id])?;
        Ok(file)
    }

    pub fn solutions_bytes(&self) -> Result<u64, Error> {
        Ok(self.conn.query_row("SELECT COALESCE(SUM(bytes), 0) FROM solver_solutions", [], |r| r.get::<_, i64>(0))? as u64)
    }

    /// The solutions to delete so that all of them fit in `max_bytes`: oldest first,
    /// pinned ones never. Pinned solutions alone may still exceed the limit.
    pub fn solutions_over_limit(&self, max_bytes: u64) -> Result<Vec<SavedSolution>, Error> {
        let mut total = self.solutions_bytes()?;
        let mut out = Vec::new();
        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT {COLUMNS} FROM solver_solutions WHERE pinned = 0 ORDER BY created_at, id"
        ))?;
        for row in stmt.query_map([], read)? {
            if total <= max_bytes {
                break;
            }
            let row = row?;
            total = total.saturating_sub(row.info.bytes);
            out.push(row);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new(key: &str, bytes: u64, at: i64, expl: f64) -> NewSolution {
        NewSolution {
            spot_key: key.into(),
            title: "BTN gegen BB".into(),
            board: "Ah8d3c".into(),
            oop: "BB".into(),
            ip: "BTN".into(),
            pot_bb: 5.5,
            stack_bb: 37.5,
            ranges: "Preflop-Modell".into(),
            sizes: "Flop 33 %".into(),
            exploitability_pct: expl,
            target_pct: 0.5,
            complete: false,
            source: SolutionSource::Hand,
            hand_id: Some(7),
            file: format!("{key}-{at}.rsms"),
            bytes,
            created_at: at,
        }
    }

    #[test]
    fn rows_round_trip_and_the_oldest_unpinned_go_first() {
        let db = Db::open_in_memory().unwrap();
        let a = db.add_solution(&new("k1", 400, 10, 0.4)).unwrap();
        let b = db.add_solution(&new("k2", 300, 20, 0.5)).unwrap();
        let c = db.add_solution(&new("k1", 200, 30, 0.3)).unwrap();
        assert_eq!(db.solution(a).unwrap().unwrap().info, new("k1", 400, 10, 0.4));
        assert_eq!(db.solutions().unwrap().iter().map(|s| s.id).collect::<Vec<_>>(), [c, b, a]);
        assert_eq!(db.solutions_for_key("k1").unwrap().iter().map(|s| s.id).collect::<Vec<_>>(), [c, a]);
        assert_eq!(db.solutions_bytes().unwrap(), 900);

        assert!(db.solutions_over_limit(900).unwrap().is_empty());
        assert_eq!(db.solutions_over_limit(500).unwrap().iter().map(|s| s.id).collect::<Vec<_>>(), [a]);
        assert!(db.set_solution_pinned(a, true).unwrap());
        assert_eq!(db.solutions_over_limit(500).unwrap().iter().map(|s| s.id).collect::<Vec<_>>(), [b, c]);
        assert_eq!(db.solutions_over_limit(0).unwrap().len(), 2, "pinned ones stay");

        assert_eq!(db.delete_solution(b).unwrap().as_deref(), Some("k2-20.rsms"));
        assert_eq!(db.delete_solution(b).unwrap(), None);
        assert!(!db.set_solution_pinned(b, true).unwrap());
    }
}
