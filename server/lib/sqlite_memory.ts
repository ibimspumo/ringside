/** `Sql` on an in-memory SQLite database with the real migrations applied, for tests. */
import { DatabaseSync } from "node:sqlite";
import { readMigrations } from "../db/migrate.ts";
import type { ResultSet, Sql, SqlValue, Stmt } from "./sql.ts";

export function memorySql(): Sql & { db: DatabaseSync } {
  const db = new DatabaseSync(":memory:");
  for (const m of readMigrations()) db.exec(m.sql);

  function run(s: Stmt): ResultSet {
    const args = (s.args ?? []).map((a) => (typeof a === "boolean" ? (a ? 1 : 0) : a));
    const stmt = db.prepare(s.sql);
    if (stmt.columns().length > 0) {
      const rows = stmt.all(...(args as never[])) as Record<string, SqlValue>[];
      return { columns: stmt.columns().map((c) => c.name), rows: rows.map((r) => ({ ...r })), affected: 0 };
    }
    const r = stmt.run(...(args as never[]));
    return { columns: [], rows: [], affected: Number(r.changes) };
  }

  return {
    db,
    execute(stmts) {
      return Promise.resolve(stmts.map(run));
    },
    batch(stmts) {
      db.exec("BEGIN IMMEDIATE");
      try {
        const out = stmts.map(run);
        db.exec("COMMIT");
        return Promise.resolve(out);
      } catch (e) {
        db.exec("ROLLBACK");
        return Promise.reject(e);
      }
    },
  };
}
