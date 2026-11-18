/**
 * Ringside Cloud API v1. Everything except wiring to the runtime lives here, so the tests
 * run it against an in-memory SQLite database.
 */
import type { Sql } from "../lib/sql.ts";
import { CanonicalError, canonicalize, solutionKey } from "./canonical.ts";
import { InvalidError, KINDS, kindSpec } from "./kinds.ts";
import { decide, hitStatements, MemoryCounter, pruneStatement, RULES, type Rule } from "./ratelimit.ts";

export const API_VERSION = "1.0.0";
const MAX_BODY = 256 * 1024;
const MAX_INPUT = 4 * 1024;
const MAX_HAND = 20_000;
const MAX_ERROR = 2_000;
/** Keys per status request; an app asks for all its pending spots at once. */
export const MAX_STATUS_KEYS = 200;
const MAX_NOTE = 300;
const KEEP_VERIFIED = 100_000;

export interface Deps {
  sql: Sql;
  /** Secret salt for hashing client IPs. */
  salt: string;
  /** Unix seconds. */
  now?: () => number;
  /** Background work after the response (runtime's waitUntil). */
  background?: (p: Promise<unknown>) => void;
  /** Chance per write request to prune old rate-limit rows. */
  pruneChance?: number;
}

class HttpError extends Error {
  constructor(readonly status: number, readonly code: string, message: string, readonly headers: HeadersInit = {}) {
    super(message);
  }
}

function json(status: number, body: unknown, headers: HeadersInit = {}): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json; charset=utf-8", "Cache-Control": "no-store", ...headers },
  });
}

/**
 * The client address as set by the CDN. Bunny forwards the visitor's IP to the script in
 * `X-Real-IP`; `X-Forwarded-For` is not used because a client can put any value into it.
 */
export function clientIp(req: Request): string {
  return req.headers.get("x-real-ip")?.trim() || "unknown";
}

async function hmacHex(secret: string, data: string): Promise<string> {
  const key = await crypto.subtle.importKey(
    "raw",
    new TextEncoder().encode(secret),
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["sign"],
  );
  const sig = await crypto.subtle.sign("HMAC", key, new TextEncoder().encode(data));
  return [...new Uint8Array(sig)].slice(0, 16).map((b) => b.toString(16).padStart(2, "0")).join("");
}

async function readBody(req: Request): Promise<unknown> {
  if (!(req.headers.get("content-type") ?? "").toLowerCase().startsWith("application/json")) {
    throw new HttpError(415, "unsupported_media_type", "Content-Type must be application/json");
  }
  const declared = Number(req.headers.get("content-length") ?? "0");
  if (declared > MAX_BODY) throw new HttpError(413, "too_large", `body larger than ${MAX_BODY} bytes`);
  const chunks: Uint8Array[] = [];
  let size = 0;
  if (req.body) {
    for await (const chunk of req.body) {
      size += chunk.length;
      if (size > MAX_BODY) throw new HttpError(413, "too_large", `body larger than ${MAX_BODY} bytes`);
      chunks.push(chunk);
    }
  }
  const bytes = new Uint8Array(size);
  let off = 0;
  for (const c of chunks) {
    bytes.set(c, off);
    off += c.length;
  }
  try {
    return JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes));
  } catch {
    throw new HttpError(400, "invalid_json", "body is not valid UTF-8 JSON");
  }
}

function exactFields(x: unknown, keys: string[]): Record<string, unknown> {
  if (typeof x !== "object" || x === null || Array.isArray(x)) throw new InvalidError("body must be an object");
  const got = Object.keys(x).sort();
  const want = [...keys].sort();
  if (got.length !== want.length || got.some((k, i) => k !== want[i])) {
    throw new InvalidError(`body must have exactly the fields ${want.join(", ")}`);
  }
  return x as Record<string, unknown>;
}

async function gzip(text: string): Promise<Uint8Array> {
  const stream = new Blob([text]).stream().pipeThrough(new CompressionStream("gzip"));
  return new Uint8Array(await new Response(stream).arrayBuffer());
}

async function gunzip(bytes: Uint8Array<ArrayBuffer>): Promise<string> {
  const stream = new Blob([bytes]).stream().pipeThrough(new DecompressionStream("gzip"));
  return await new Response(stream).text();
}

export function createHandler(deps: Deps): (req: Request) => Promise<Response> {
  const now = deps.now ?? (() => Math.floor(Date.now() / 1000));
  const reads = new MemoryCounter();
  const pruneChance = deps.pruneChance ?? 0.01;
  // A verified key never changes again, so the isolate answers for it without the database.
  const verified = new Set<string>();

  async function limitWrites(rules: readonly Rule[], client: string) {
    const t = now();
    const clientRules = rules.filter((r) => r.scope === "client");
    const globalRules = rules.filter((r) => r.scope === "global");
    // Global counters are only touched once the client is within its own limits, so one
    // client hammering the API cannot use up everybody's quota.
    for (const group of [clientRules, globalRules]) {
      const results = await deps.sql.execute(hitStatements(group, client, t));
      const d = decide(group.map((rule, i) => ({ rule, count: Number(results[i].rows[0].count) })), t);
      if (!d.ok) {
        throw new HttpError(429, "rate_limited", `too many requests (${d.rule})`, {
          "Retry-After": String(d.retryAfter),
        });
      }
    }
    if (deps.background && Math.random() < pruneChance) {
      deps.background(deps.sql.execute([pruneStatement(t)]).catch(() => {}));
    }
  }

  async function getSolution(kind: string, key: string, client: string): Promise<Response> {
    if (!kindSpec(kind)) throw new HttpError(404, "unknown_kind", "unknown kind");
    if (!/^[0-9a-f]{64}$/.test(key)) throw new HttpError(400, "invalid_key", "key must be 64 lowercase hex digits");
    const t = now();
    const d = decide(RULES.read.map((rule) => ({ rule, count: reads.hit(rule, client, t) })), t);
    if (!d.ok) {
      throw new HttpError(429, "rate_limited", `too many requests (${d.rule})`, { "Retry-After": String(d.retryAfter) });
    }
    const [r] = await deps.sql.execute([{
      sql: "SELECT engine_version, input, result, exploitability, checked_at FROM solutions " +
        "WHERE key = ? AND kind = ? AND status = 'verified'",
      args: [key, kind],
    }]);
    const row = r.rows[0];
    if (!row) throw new HttpError(404, "not_found", "no verified solution for this key");
    const body = `{"kind":${JSON.stringify(kind)},"engine_version":${Number(row.engine_version)},"key":"${key}",` +
      `"input":${row.input},"result":${await gunzip(row.result as Uint8Array<ArrayBuffer>)},` +
      `"exploitability":${JSON.stringify(row.exploitability)},"verified_at":${Number(row.checked_at)}}`;
    // The key fixes kind, engine version and input; a verified result for it never changes.
    return new Response(body, {
      status: 200,
      headers: {
        "Content-Type": "application/json; charset=utf-8",
        "Cache-Control": "public, max-age=31536000, immutable",
      },
    });
  }

  /** Where each key stands: verified, submitted (waits for the verifier), rejected (with the verifier's note) or unknown. */
  async function statuses(req: Request, client: string): Promise<Response> {
    const t = now();
    const d = decide(RULES.status.map((rule) => ({ rule, count: reads.hit(rule, client, t) })), t);
    if (!d.ok) {
      throw new HttpError(429, "rate_limited", `too many requests (${d.rule})`, { "Retry-After": String(d.retryAfter) });
    }
    const { keys } = exactFields(await readBody(req), ["keys"]);
    if (!Array.isArray(keys) || keys.length < 1 || keys.length > MAX_STATUS_KEYS) {
      throw new InvalidError(`keys must be a list of 1 to ${MAX_STATUS_KEYS} keys`);
    }
    if (keys.some((k) => typeof k !== "string" || !/^[0-9a-f]{64}$/.test(k))) {
      throw new InvalidError("every key must be 64 lowercase hex digits");
    }
    const out: Record<string, { status: string; note?: string }> = {};
    const ask = [...new Set(keys as string[])].filter((k) => {
      if (verified.has(k)) out[k] = { status: "verified" };
      return !verified.has(k);
    });
    if (ask.length > 0) {
      const [r] = await deps.sql.execute([{
        sql: `SELECT key, status, check_note FROM solutions WHERE key IN (${ask.map(() => "?").join(",")})`,
        args: ask,
      }]);
      for (const row of r.rows) {
        const key = String(row.key);
        const status = String(row.status);
        out[key] = status === "rejected" && row.check_note
          ? { status, note: String(row.check_note).slice(0, MAX_NOTE) }
          : { status };
        if (status === "verified") {
          if (verified.size >= KEEP_VERIFIED) verified.clear();
          verified.add(key);
        }
      }
      for (const k of ask) out[k] ??= { status: "unknown" };
    }
    return json(200, { statuses: out });
  }

  async function submitSolution(req: Request, client: string): Promise<Response> {
    await limitWrites(RULES.submit, client);
    const body = exactFields(await readBody(req), ["kind", "engine_version", "input", "result"]);
    const kind = String(body.kind);
    const spec = typeof body.kind === "string" ? kindSpec(kind) : undefined;
    if (!spec) throw new InvalidError(`kind must be one of ${Object.keys(KINDS).join(", ")}`);
    const version = body.engine_version;
    if (typeof version !== "number" || !spec.engineVersions.includes(version)) {
      throw new InvalidError(`engine_version must be one of ${spec.engineVersions.join(", ")}`);
    }
    const input = canonicalize(body.input);
    if (input.length > MAX_INPUT) throw new InvalidError("input too large");
    spec.checkInput(body.input);
    const result = JSON.stringify(spec.normalizeResult(body.result, body.input));
    if (result.length > spec.maxResultBytes) throw new InvalidError("result too large");
    const key = await solutionKey(kind, version, input);
    const t = now();

    const [outcome] = await deps.sql.batch([
      {
        sql: "INSERT INTO submissions (key, kind, submitter, received_at, size, outcome) " +
          "SELECT ?, ?, ?, ?, ?, CASE (SELECT status FROM solutions WHERE key = ?) " +
          "WHEN 'rejected' THEN 'replaced' WHEN 'submitted' THEN 'pending' WHEN 'verified' THEN 'verified' " +
          "ELSE 'new' END RETURNING outcome",
        args: [key, kind, client, t, result.length, key],
      },
      {
        sql: "INSERT INTO solutions (key, kind, engine_version, input, result, result_size, status, submitter, " +
          "submitted_at) VALUES (?, ?, ?, ?, ?, ?, 'submitted', ?, ?) " +
          "ON CONFLICT (key) DO UPDATE SET input = excluded.input, result = excluded.result, " +
          "result_size = excluded.result_size, status = 'submitted', exploitability = NULL, " +
          "submitter = excluded.submitter, submitted_at = excluded.submitted_at, checked_at = NULL, " +
          "check_note = NULL WHERE solutions.status = 'rejected'",
        args: [key, kind, version, input, await gzip(result), result.length, client, t],
      },
    ]);
    const o = String(outcome.rows[0].outcome);
    return o === "verified"
      ? json(200, { key, status: "verified" })
      : json(202, { key, status: "submitted", outcome: o });
  }

  async function report(req: Request, client: string): Promise<Response> {
    await limitWrites(RULES.report, client);
    const body = exactFields(await readBody(req), ["app_version", "error", "hand"]);
    const { app_version, error, hand } = body;
    if (typeof app_version !== "string" || !/^[0-9A-Za-z.+-]{1,32}$/.test(app_version)) {
      throw new InvalidError("app_version must be a version string");
    }
    if (typeof error !== "string" || error.length < 1 || error.length > MAX_ERROR) {
      throw new InvalidError(`error must be a string of 1 to ${MAX_ERROR} characters`);
    }
    if (typeof hand !== "string" || hand.length < 1 || hand.length > MAX_HAND) {
      throw new InvalidError(`hand must be a string of 1 to ${MAX_HAND} characters`);
    }
    await deps.sql.execute([{
      sql: "INSERT INTO reports (received_at, app_version, error, hand, submitter) VALUES (?, ?, ?, ?, ?)",
      args: [now(), app_version, error, hand, client],
    }]);
    return json(202, { status: "received" });
  }

  return async function handle(req: Request): Promise<Response> {
    try {
      const path = new URL(req.url).pathname;
      const method = req.method;
      if (path === "/v1/health") {
        if (method !== "GET" && method !== "HEAD") throw new HttpError(405, "method_not_allowed", "use GET");
        return json(200, {
          status: "ok",
          api: API_VERSION,
          kinds: Object.fromEntries(Object.entries(KINDS).map(([k, s]) => [k, s.engineVersions])),
        });
      }
      const client = await hmacHex(deps.salt, clientIp(req));
      if (path === "/v1/solutions/status") {
        if (method !== "POST") throw new HttpError(405, "method_not_allowed", "use POST");
        return await statuses(req, client);
      }
      const sol = path.match(/^\/v1\/solutions\/([a-z0-9-]{1,40})\/([^/]+)$/);
      if (sol) {
        if (method !== "GET" && method !== "HEAD") throw new HttpError(405, "method_not_allowed", "use GET");
        return await getSolution(sol[1], sol[2], client);
      }
      if (path === "/v1/solutions") {
        if (method !== "POST") throw new HttpError(405, "method_not_allowed", "use POST");
        return await submitSolution(req, client);
      }
      if (path === "/v1/reports") {
        if (method !== "POST") throw new HttpError(405, "method_not_allowed", "use POST");
        return await report(req, client);
      }
      throw new HttpError(404, "not_found", "no such endpoint");
    } catch (e) {
      if (e instanceof HttpError) return json(e.status, { error: e.code, message: e.message }, e.headers);
      if (e instanceof InvalidError || e instanceof CanonicalError) {
        return json(400, { error: "invalid", message: e.message });
      }
      console.error("internal error:", e instanceof Error ? `${e.name}: ${e.message}` : String(e));
      return json(500, { error: "internal", message: "internal error" });
    }
  };
}
