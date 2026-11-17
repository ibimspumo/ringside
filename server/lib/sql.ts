/**
 * Minimal client for Bunny Database's SQL API (libSQL "Hrana over HTTP", /v2/pipeline).
 * It only uses `fetch`, so the same code runs in the Edge Script and in the local tools.
 */

export type SqlValue = null | number | bigint | string | Uint8Array;

export interface Stmt {
  sql: string;
  args?: (SqlValue | boolean)[];
}

export interface ResultSet {
  columns: string[];
  rows: Record<string, SqlValue>[];
  affected: number;
}

export interface Sql {
  /** Runs the statements in order, each on its own (no transaction). */
  execute(stmts: Stmt[]): Promise<ResultSet[]>;
  /** Runs the statements in one transaction: all commit or none do. */
  batch(stmts: Stmt[]): Promise<ResultSet[]>;
}

export class SqlError extends Error {
  constructor(message: string, readonly code?: string, readonly status?: number) {
    super(message);
    this.name = "SqlError";
  }
}

export function pipelineUrl(databaseUrl: string): string {
  const base = databaseUrl.trim().replace(/^libsql:\/\//, "https://").replace(/\/+$/, "");
  if (!base.startsWith("https://")) throw new SqlError("database URL must be libsql:// or https://");
  return `${base}/v2/pipeline`;
}

type Wire =
  | { type: "null" }
  | { type: "integer"; value: string }
  | { type: "float"; value: number }
  | { type: "text"; value: string }
  | { type: "blob"; base64: string };

function encode(v: SqlValue | boolean): Wire {
  if (v === null) return { type: "null" };
  if (typeof v === "boolean") return { type: "integer", value: v ? "1" : "0" };
  if (typeof v === "bigint") return { type: "integer", value: v.toString() };
  if (typeof v === "number") {
    if (!Number.isFinite(v)) throw new SqlError("non-finite number");
    return Number.isSafeInteger(v) ? { type: "integer", value: String(v) } : { type: "float", value: v };
  }
  if (typeof v === "string") return { type: "text", value: v };
  return { type: "blob", base64: toBase64(v) };
}

function decode(w: Wire): SqlValue {
  switch (w.type) {
    case "null":
      return null;
    case "integer": {
      const n = BigInt(w.value);
      return n >= BigInt(Number.MIN_SAFE_INTEGER) && n <= BigInt(Number.MAX_SAFE_INTEGER) ? Number(n) : n;
    }
    case "float":
      return Number(w.value);
    case "text":
      return w.value;
    case "blob":
      return fromBase64(w.base64);
  }
}

export function toBase64(bytes: Uint8Array): string {
  let s = "";
  for (let i = 0; i < bytes.length; i += 0x8000) {
    s += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  }
  return btoa(s);
}

export function fromBase64(b64: string): Uint8Array {
  const padded = b64 + "=".repeat((4 - (b64.length % 4)) % 4);
  const s = atob(padded);
  const out = new Uint8Array(s.length);
  for (let i = 0; i < s.length; i++) out[i] = s.charCodeAt(i);
  return out;
}

interface WireResult {
  cols: { name: string | null }[];
  rows: Wire[][];
  affected_row_count: number;
}

function toResultSet(r: WireResult): ResultSet {
  const columns = r.cols.map((c, i) => c.name ?? `col${i}`);
  return {
    columns,
    rows: r.rows.map((row) => Object.fromEntries(row.map((v, i) => [columns[i], decode(v)]))),
    affected: r.affected_row_count,
  };
}

function wireStmt(s: Stmt) {
  return { sql: s.sql, args: (s.args ?? []).map(encode) };
}

export function createSql(
  databaseUrl: string,
  authToken: string,
  opts: { timeoutMs?: number; fetch?: typeof fetch } = {},
): Sql {
  const url = pipelineUrl(databaseUrl);
  const doFetch = opts.fetch ?? fetch;
  const timeoutMs = opts.timeoutMs ?? 10_000;

  async function pipeline(requests: unknown[]): Promise<{ type: string; response?: any; error?: any }[]> {
    let res: Response;
    try {
      res = await doFetch(url, {
        method: "POST",
        headers: { Authorization: `Bearer ${authToken}`, "Content-Type": "application/json" },
        body: JSON.stringify({ requests: [...requests, { type: "close" }] }),
        signal: AbortSignal.timeout(timeoutMs),
      });
    } catch (e) {
      throw new SqlError(`database unreachable: ${(e as Error).name}`, "NETWORK");
    }
    if (!res.ok) {
      const text = (await res.text()).slice(0, 300);
      throw new SqlError(`database HTTP ${res.status}: ${text}`, "HTTP", res.status);
    }
    const body = await res.json();
    return body.results;
  }

  function fail(err: any): never {
    throw new SqlError(err?.message ?? "statement failed", err?.code);
  }

  return {
    async execute(stmts) {
      const results = await pipeline(stmts.map((s) => ({ type: "execute", stmt: wireStmt(s) })));
      return stmts.map((_, i) => {
        const r = results[i];
        if (r.type !== "ok") fail(r.error);
        return toResultSet(r.response.result);
      });
    },

    async batch(stmts) {
      // Hrana batch steps with conditions: every step runs only if the previous one
      // succeeded, and the rollback only if the commit did not happen.
      const n = stmts.length;
      const steps: unknown[] = [{ stmt: { sql: "BEGIN IMMEDIATE" } }];
      stmts.forEach((s, i) => steps.push({ stmt: wireStmt(s), condition: { type: "ok", step: i } }));
      steps.push({ stmt: { sql: "COMMIT" }, condition: { type: "ok", step: n } });
      steps.push({ stmt: { sql: "ROLLBACK" }, condition: { type: "not", cond: { type: "ok", step: n + 1 } } });
      const [r] = await pipeline([{ type: "batch", batch: { steps } }]);
      if (r.type !== "ok") fail(r.error);
      const { step_results, step_errors } = r.response.result;
      for (let i = 0; i <= n + 1; i++) {
        if (step_errors[i]) fail(step_errors[i]);
      }
      return step_results.slice(1, n + 1).map(toResultSet);
    },
  };
}
