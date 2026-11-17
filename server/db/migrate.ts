/**
 * Applies server/db/migrations/NNNN_*.sql to Bunny Database in order, each in one
 * transaction, and records the version plus each file's SHA-256 in `meta`.
 */
import { loadEnv, need } from "../lib/env.ts";
import { createSql, type Sql } from "../lib/sql.ts";

const HELP = `Usage: deno task migrate [--status]

Applies pending migrations from server/db/migrations to the database in .env
(BUNNY_DATABASE_URL, BUNNY_DATABASE_AUTH_TOKEN).

  --status   only show the applied version and pending files
  --help     this text

An applied migration must never change; the script stops if its checksum differs.`;

const DIR = new URL("./migrations/", import.meta.url);

export interface Migration {
  version: number;
  name: string;
  sql: string;
}

/** Statements of a migration file: `--` comments removed, split at semicolons. */
export function splitSql(text: string): string[] {
  return text
    .split("\n")
    .map((l) => l.replace(/--.*$/, ""))
    .join("\n")
    .split(";")
    .map((s) => s.trim())
    .filter((s) => s.length > 0);
}

export function readMigrations(dir: URL = DIR): Migration[] {
  const out: Migration[] = [];
  for (const e of Deno.readDirSync(dir)) {
    const m = e.name.match(/^(\d{4})_[a-z0-9_]+\.sql$/);
    if (!e.isFile || !m) continue;
    out.push({ version: Number(m[1]), name: e.name, sql: Deno.readTextFileSync(new URL(e.name, dir)) });
  }
  out.sort((a, b) => a.version - b.version);
  out.forEach((m, i) => {
    if (m.version !== i + 1) throw new Error(`migration numbers must be 1, 2, 3, …; found ${m.name}`);
  });
  return out;
}

async function sha256(text: string): Promise<string> {
  const d = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(text));
  return [...new Uint8Array(d)].map((b) => b.toString(16).padStart(2, "0")).join("");
}

async function state(sql: Sql): Promise<Map<string, string>> {
  await sql.execute([{ sql: "CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL) STRICT" }]);
  const [r] = await sql.execute([{ sql: "SELECT key, value FROM meta WHERE key = 'schema_version' OR key LIKE 'migration:%'" }]);
  return new Map(r.rows.map((row) => [String(row.key), String(row.value)]));
}

async function main() {
  const args = new Set(Deno.args);
  if (args.has("--help") || args.has("-h")) return console.log(HELP);
  const env = loadEnv();
  const sql = createSql(need(env, "BUNNY_DATABASE_URL"), need(env, "BUNNY_DATABASE_AUTH_TOKEN"));

  const meta = await state(sql);
  const current = Number(meta.get("schema_version") ?? 0);
  const migrations = readMigrations();
  for (const m of migrations.filter((m) => m.version <= current)) {
    const stored = meta.get(`migration:${m.name}`);
    if (stored && stored !== (await sha256(m.sql))) {
      throw new Error(`${m.name} was changed after it was applied; add a new migration instead`);
    }
  }
  const pending = migrations.filter((m) => m.version > current);
  console.log(`schema version ${current}, pending: ${pending.map((m) => m.name).join(", ") || "none"}`);
  if (args.has("--status")) return;

  for (const m of pending) {
    await sql.batch([
      ...splitSql(m.sql).map((s) => ({ sql: s })),
      { sql: "INSERT OR REPLACE INTO meta (key, value) VALUES ('schema_version', ?)", args: [String(m.version)] },
      { sql: "INSERT OR REPLACE INTO meta (key, value) VALUES (?, ?)", args: [`migration:${m.name}`, await sha256(m.sql)] },
    ]);
    console.log(`applied ${m.name}`);
  }
}

if (import.meta.main) await main();
