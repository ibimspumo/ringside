/**
 * Ringside Cloud API v1. Everything except wiring to the runtime lives here, so the tests
 * run it against an in-memory SQLite database.
 */
import type { Sql } from "../lib/sql.ts";
import { CanonicalError, canonicalize, solutionKey } from "./canonical.ts";
import { InvalidError, KINDS, kindSpec } from "./kinds.ts";
import { decide, hitStatements, MemoryCounter, pruneStatement, RULES, type Rule } from "./ratelimit.ts";

export const API_VERSION = "1.4.0";
const MAX_BODY = 256 * 1024;
const MAX_INPUT = 4 * 1024;
const MAX_HAND = 20_000;
const MAX_ERROR = 2_000;
/** Keys per status request; an app asks for all its pending spots at once. */
export const MAX_STATUS_KEYS = 200;
const MAX_NOTE = 300;
const KEEP_VERIFIED = 100_000;
/** How long the CDN may keep an accepted solution; it is withdrawn if its client turns out to cheat. */
const ACCEPTED_MAX_AGE_S = 86_400;
/** How long a computer may work on a spot before it is handed out again, by kind. */
export const LEASE_S: Readonly<Record<string, number>> = {
  "pushfold-hu": 600,
  "pushfold-mw": 1_800,
  postflop: 3 * 3_600,
  spot: 3 * 3_600,
};
/** Spots one client (one IP, so several computers behind a router) holds at once; a further request frees its oldest. */
export const MAX_LEASES = 3;
/** Kinds a client may ask the community to solve; the others are seeded by the operator. */
const REQUESTABLE = ["spot"];

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
  // A verified key never changes again, so the isolate answers for it without the database;
  // an accepted one can still be withdrawn.
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
      sql: "SELECT engine_version, input, result, status, exploitability, checked_at FROM solutions " +
        "WHERE key = ? AND kind = ? AND status IN ('accepted', 'verified')",
      args: [key, kind],
    }]);
    const row = r.rows[0];
    if (!row) throw new HttpError(404, "not_found", "no solution for this key");
    const verifiedRow = row.status === "verified";
    const body = `{"kind":${JSON.stringify(kind)},"engine_version":${Number(row.engine_version)},"key":"${key}",` +
      `"input":${row.input},"result":${await gunzip(row.result as Uint8Array<ArrayBuffer>)},` +
      `"status":"${verifiedRow ? "verified" : "accepted"}","exploitability":${JSON.stringify(row.exploitability)},` +
      `"verified_at":${verifiedRow ? Number(row.checked_at) : null}}`;
    // The key fixes kind, engine version and input; a verified result for it never changes.
    return new Response(body, {
      status: 200,
      headers: {
        "Content-Type": "application/json; charset=utf-8",
        "Cache-Control": verifiedRow ? "public, max-age=31536000, immutable" : `public, max-age=${ACCEPTED_MAX_AGE_S}`,
      },
    });
  }

  /**
   * Where each key stands: accepted (served), verified (served, re-solved by the verifier),
   * submitted (from before accepting, waits for the verifier's form check), rejected (with
   * the verifier's note) or unknown.
   */
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
    const { kind, spec, version, input, key } = await solutionInput(body);
    const normalized = await spec.normalizeResult(body.result, body.input);
    const result = JSON.stringify(normalized.result);
    if (result.length > spec.maxResultBytes) throw new InvalidError("result too large");
    const t = now();
    const allowed = "NOT EXISTS (SELECT 1 FROM banned_clients WHERE submitter = ?)";

    const [outcome, , stored] = await deps.sql.batch([
      {
        sql: "INSERT INTO submissions (key, kind, submitter, received_at, size, outcome) " +
          "SELECT ?, ?, ?, ?, ?, CASE (SELECT status FROM solutions WHERE key = ?) " +
          "WHEN 'rejected' THEN 'replaced' WHEN 'verified' THEN 'verified' WHEN 'submitted' THEN 'pending' " +
          `WHEN 'accepted' THEN 'pending' ELSE 'new' END WHERE ${allowed} RETURNING outcome`,
        args: [key, kind, client, t, result.length, key, client],
      },
      {
        sql: "INSERT INTO solutions (key, kind, engine_version, input, result, result_size, status, exploitability, " +
          `submitter, submitted_at) SELECT ?, ?, ?, ?, ?, ?, 'accepted', ?, ?, ? WHERE ${allowed} ` +
          "ON CONFLICT (key) DO UPDATE SET input = excluded.input, result = excluded.result, " +
          "result_size = excluded.result_size, status = 'accepted', exploitability = excluded.exploitability, " +
          "submitter = excluded.submitter, submitted_at = excluded.submitted_at, checked_at = NULL, " +
          "check_note = NULL WHERE solutions.status = 'rejected'",
        args: [key, kind, version, input, await gzip(result), result.length, normalized.exploitability, client, t, client],
      },
      { sql: "SELECT status FROM solutions WHERE key = ?", args: [key] },
    ]);
    if (outcome.rows.length === 0) {
      throw new HttpError(403, "banned", "this address may not submit: a solution from it failed the verifier's check");
    }
    const o = String(outcome.rows[0].outcome);
    const status = String(stored.rows[0].status);
    return json(o === "new" || o === "replaced" ? 201 : 200, { key, status, outcome: o });
  }

  /** The kind, engine version and canonical input of a body, with its key; throws on anything malformed. */
  async function solutionInput(body: Record<string, unknown>) {
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
    return { kind, spec, version, input, key: await solutionKey(kind, version, input) };
  }

  /**
   * Hands out the open spot first in line (most requested, least handed out, oldest)
   * among the kinds the client can solve, leased for the kind's time. A spot whose lease
   * ran out is open again; one with a solution that is served or waits for a check is not.
   */
  async function nextWork(req: Request, client: string): Promise<Response> {
    await limitWrites(RULES.work, client);
    const { kinds } = exactFields(await readBody(req), ["kinds"]);
    if (typeof kinds !== "object" || kinds === null || Array.isArray(kinds)) {
      throw new InvalidError("kinds must map kind names to engine versions");
    }
    const wanted: [string, number[]][] = [];
    for (const [kind, versions] of Object.entries(kinds)) {
      const spec = kindSpec(kind);
      if (!spec) throw new InvalidError(`unknown kind ${kind.slice(0, 40)}`);
      if (!Array.isArray(versions) || versions.length < 1 || versions.some((v) => !spec.engineVersions.includes(v))) {
        throw new InvalidError(`engine versions of ${kind} must be among ${spec.engineVersions.join(", ")}`);
      }
      wanted.push([kind, versions as number[]]);
    }
    if (wanted.length === 0) throw new InvalidError("kinds must name at least one kind");
    const t = now();
    const filter = wanted.map(([, vs]) => `(w.kind = ? AND w.engine_version IN (${vs.map(() => "?").join(",")}))`)
      .join(" OR ");
    const filterArgs = wanted.flatMap(([k, vs]) => [k, ...vs]);
    const leaseCase = `CASE kind ${Object.keys(LEASE_S).map(() => "WHEN ? THEN ?").join(" ")} ELSE 3600 END`;
    const leaseArgs = Object.entries(LEASE_S).flat();
    const [, picked] = await deps.sql.batch([
      // Leases beyond the client's share go back to the queue, oldest first.
      {
        sql: "UPDATE work SET lease_until = NULL, leased_to = NULL WHERE key IN (SELECT key FROM work " +
          "WHERE leased_to = ? AND lease_until > ? ORDER BY lease_until DESC LIMIT -1 OFFSET ?)",
        args: [client, t, MAX_LEASES - 1],
      },
      {
        sql: `UPDATE work SET lease_until = ? + ${leaseCase}, leased_to = ?, leases = leases + 1 ` +
          "WHERE key = (SELECT w.key FROM work w WHERE (" + filter + ") " +
          "AND (w.lease_until IS NULL OR w.lease_until <= ?) " +
          "AND NOT EXISTS (SELECT 1 FROM solutions s WHERE s.key = w.key AND s.status != 'rejected') " +
          "ORDER BY w.priority DESC, w.leases, w.created_at LIMIT 1) " +
          "AND (lease_until IS NULL OR lease_until <= ?) " +
          "RETURNING key, kind, engine_version, input, lease_until",
        args: [t, ...leaseArgs, client, ...filterArgs, t, t],
      },
    ]);
    const row = picked.rows[0];
    if (!row) return json(200, { work: null });
    return new Response(
      `{"work":{"key":"${row.key}","kind":${JSON.stringify(row.kind)},"engine_version":${
        Number(row.engine_version)
      },"input":${row.input},"lease_until":${Number(row.lease_until)}}}`,
      { status: 200, headers: { "Content-Type": "application/json; charset=utf-8", "Cache-Control": "no-store" } },
    );
  }

  /** A client stopped before it solved its spot: the lease goes back, if it holds it. */
  async function releaseWork(req: Request, client: string): Promise<Response> {
    await limitWrites(RULES.release, client);
    const { key } = exactFields(await readBody(req), ["key"]);
    if (typeof key !== "string" || !/^[0-9a-f]{64}$/.test(key)) throw new InvalidError("key must be a solution key");
    const [r] = await deps.sql.batch([{
      sql: "UPDATE work SET lease_until = NULL, leased_to = NULL, leases = max(leases - 1, 0) " +
        "WHERE key = ? AND leased_to = ? AND lease_until > ?",
      args: [key, client, now()],
    }]);
    return json(200, { key, status: r.affected > 0 ? "released" : "not_leased" });
  }

  /** A client looked a spot up in vain: queue it for the community, or count one more request. */
  async function requestWork(req: Request, client: string): Promise<Response> {
    await limitWrites(RULES.request, client);
    const body = exactFields(await readBody(req), ["kind", "engine_version", "input"]);
    const { kind, version, input, key } = await solutionInput(body);
    if (!REQUESTABLE.includes(kind)) throw new InvalidError(`only ${REQUESTABLE.join(", ")} can be requested`);
    const [r] = await deps.sql.batch([
      {
        sql: "SELECT status FROM solutions WHERE key = ? AND status != 'rejected'",
        args: [key],
      },
      {
        sql: "INSERT INTO work (key, kind, engine_version, input, source, priority, created_at) " +
          "SELECT ?, ?, ?, ?, 'request', 1, ? " +
          "WHERE NOT EXISTS (SELECT 1 FROM solutions WHERE key = ? AND status != 'rejected') " +
          "ON CONFLICT (key) DO UPDATE SET priority = priority + 1",
        args: [key, kind, version, input, now(), key],
      },
    ]);
    const solved = r.rows[0];
    return json(202, { key, status: solved ? String(solved.status) : "queued" });
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
      if (path === "/v1/work/next") {
        if (method !== "POST") throw new HttpError(405, "method_not_allowed", "use POST");
        return await nextWork(req, client);
      }
      if (path === "/v1/work/release") {
        if (method !== "POST") throw new HttpError(405, "method_not_allowed", "use POST");
        return await releaseWork(req, client);
      }
      if (path === "/v1/work/request") {
        if (method !== "POST") throw new HttpError(405, "method_not_allowed", "use POST");
        return await requestWork(req, client);
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
