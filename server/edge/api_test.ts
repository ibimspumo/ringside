import { assert, assertEquals } from "jsr:@std/assert@1";
import { memorySql } from "../lib/sqlite_memory.ts";
import { createHandler } from "./api.ts";
import { canonicalize, solutionKey } from "./canonical.ts";
import { nodeCount } from "./kinds.ts";
import { RULES } from "./ratelimit.ts";

const BASE = "https://ringside-api.test";
const INPUT = { stack_mbb: 10000, sb_mbb: 500, ante_mbb: 0, dead_mbb: 0 };
const RESULT = { shove: Array(169).fill(1), call: Array(169).fill(0.25) };

function setup() {
  const sql = memorySql();
  let t = 1_000_000;
  const handle = createHandler({ sql, salt: "test-salt", now: () => t });
  return { sql, handle, advance: (s: number) => (t += s) };
}

function post(path: string, body: unknown, ip = "203.0.113.7", raw?: string): Request {
  return new Request(BASE + path, {
    method: "POST",
    headers: { "Content-Type": "application/json", "X-Real-IP": ip },
    body: raw ?? JSON.stringify(body),
  });
}

const get = (path: string, ip = "203.0.113.7") => new Request(BASE + path, { headers: { "X-Real-IP": ip } });
const submission = (over: Record<string, unknown> = {}) => ({
  kind: "pushfold-hu",
  engine_version: 1,
  input: INPUT,
  result: RESULT,
  ...over,
});

Deno.test("health answers without the database", async () => {
  const { handle } = setup();
  const res = await handle(get("/v1/health"));
  assertEquals(res.status, 200);
  assertEquals((await res.json()).kinds, { "pushfold-hu": [1], "pushfold-mw": [1], postflop: [1] });
});

Deno.test("submitted solutions stay hidden until verified, then are cacheable", async () => {
  const { sql, handle } = setup();
  const res = await handle(post("/v1/solutions", submission()));
  assertEquals(res.status, 202);
  const { key, status, outcome } = await res.json();
  assertEquals([status, outcome], ["submitted", "new"]);
  assertEquals(key, await solutionKey("pushfold-hu", 1, canonicalize(INPUT)), "server computes the key");

  const hidden = await handle(get(`/v1/solutions/pushfold-hu/${key}`));
  assertEquals(hidden.status, 404);
  assertEquals(hidden.headers.get("cache-control"), "no-store");

  sql.db.prepare("UPDATE solutions SET status = 'verified', exploitability = 1e-12, checked_at = 5 WHERE key = ?")
    .run(key);
  const shown = await handle(get(`/v1/solutions/pushfold-hu/${key}`));
  assertEquals(shown.status, 200);
  assert(shown.headers.get("cache-control")!.includes("immutable"));
  const body = await shown.json();
  assertEquals(body.input, INPUT);
  assertEquals(body.result, RESULT);
  assertEquals(body.exploitability, 1e-12);

  const again = await handle(post("/v1/solutions", submission({ result: { ...RESULT, call: Array(169).fill(0) } })));
  assertEquals([again.status, (await again.json()).status], [200, "verified"]);
  const kept = await (await handle(get(`/v1/solutions/pushfold-hu/${key}`))).json();
  assertEquals(kept.result, RESULT, "a verified solution is never overwritten");
});

Deno.test("a pending solution is kept, a rejected one is replaced", async () => {
  const { sql, handle } = setup();
  const { key } = await (await handle(post("/v1/solutions", submission()))).json();
  const other = { shove: Array(169).fill(0), call: Array(169).fill(0) };
  const dup = await handle(post("/v1/solutions", submission({ result: other }), "198.51.100.1"));
  assertEquals((await dup.json()).outcome, "pending");
  const stored = () => sql.db.prepare("SELECT status, result_size FROM solutions WHERE key = ?").get(key);
  assertEquals(stored()!.result_size, JSON.stringify(RESULT).length);

  sql.db.prepare("UPDATE solutions SET status = 'rejected' WHERE key = ?").run(key);
  const rep = await handle(post("/v1/solutions", submission({ result: other }), "198.51.100.1"));
  assertEquals((await rep.json()).outcome, "replaced");
  assertEquals(stored(), { status: "submitted", result_size: JSON.stringify(other).length });
  const log = sql.db.prepare("SELECT outcome FROM submissions ORDER BY id").all().map((r) => r.outcome);
  assertEquals(log, ["new", "pending", "replaced"]);
});

Deno.test("malformed submissions are rejected", async () => {
  const { handle } = setup();
  const cases: [unknown, number][] = [
    [submission({ kind: "postflop" }), 400],
    [submission({ engine_version: 2 }), 400],
    [submission({ extra: 1 }), 400],
    [submission({ input: { ...INPUT, stack_mbb: 10000.5 } }), 400],
    [submission({ input: { ...INPUT, stack_mbb: 999 } }), 400],
    [submission({ input: { ...INPUT, rake: 0 } }), 400],
    [submission({ result: { ...RESULT, shove: Array(168).fill(1) } }), 400],
    [submission({ result: { ...RESULT, call: [...Array(168).fill(0), 1.5] } }), 400],
    [submission({ result: { ...RESULT, call: [...Array(168).fill(0), "1"] } }), 400],
  ];
  for (const [body, status] of cases) {
    const res = await handle(post("/v1/solutions", body, `192.0.2.${status}`));
    assertEquals(res.status, status, JSON.stringify(body).slice(0, 120));
    await res.body?.cancel();
  }
  const noJson = await handle(post("/v1/solutions", null, "192.0.2.50", "{nope"));
  assertEquals(noJson.status, 400);
  const big = await handle(post("/v1/solutions", null, "192.0.2.51", " ".repeat(300_000)));
  assertEquals(big.status, 413);
  const wrongType = await handle(
    new Request(BASE + "/v1/solutions", { method: "POST", headers: { "X-Real-IP": "192.0.2.52" }, body: "{}" }),
  );
  assertEquals(wrongType.status, 415);
  assertEquals((await handle(get("/v1/solutions/pushfold-hu/ABC"))).status, 400);
  assertEquals((await handle(get("/v1/solutions/postflop/" + "a".repeat(64)))).status, 404);
  assertEquals((await handle(get("/v1/solutions"))).status, 405);
});

Deno.test("multiway submissions are checked against the tree of their input", async () => {
  const { handle } = setup();
  const input = {
    stacks_mbb: [10000, 10000, 10000],
    sb_mbb: 500,
    ante_mbb: 0,
    bb_ante_mbb: 0,
    dead_mbb: 0,
    max_all_in: 3,
    payouts_ppm: [],
  };
  const act = (nodes: number) => ({ act: Array.from({ length: nodes }, () => Array(169).fill(0.5)) });
  const mw = (over: Record<string, unknown>) => ({ kind: "pushfold-mw", engine_version: 1, input, result: act(6), ...over });
  const ok = await handle(post("/v1/solutions", mw({})));
  assertEquals(ok.status, 202);
  assertEquals((await ok.json()).key, await solutionKey("pushfold-mw", 1, canonicalize(input)));
  const nine = { ...input, stacks_mbb: Array(9).fill(20000) };
  const big = await handle(post("/v1/solutions", mw({ input: nine, result: act(128) }), "192.0.2.90"));
  assertEquals(big.status, 202, "a 9-player profile fits");
  await big.body?.cancel();
  const cases: unknown[] = [
    mw({ result: act(5) }),
    mw({ input: { ...input, max_all_in: 4 } }),
    mw({ input: { ...input, stacks_mbb: Array(10).fill(10000) } }),
    mw({ input: { ...input, payouts_ppm: [600000, 300000] } }),
    mw({ input: { ...input, payouts_ppm: [500000, 300000, 200000, 0] } }),
    mw({ input: { ...input, stacks_mbb: [10000, 999, 10000] } }),
  ];
  for (const [i, body] of cases.entries()) {
    const res = await handle(post("/v1/solutions", body, `192.0.2.${100 + i}`));
    assertEquals(res.status, 400, JSON.stringify(body).slice(0, 160));
    await res.body?.cancel();
  }
});

Deno.test("node counts match the engine's trees", () => {
  // Counted by ringside-engine's multiway::solve, see node_count_matches_the_engine.
  for (const [n, cap, nodes] of [[2, 2, 2], [3, 3, 6], [4, 3, 13], [6, 3, 40], [9, 3, 128]]) {
    assertEquals(nodeCount(n, cap), nodes, `${n} players, cap ${cap}`);
  }
});

Deno.test("reports are stored and validated", async () => {
  const { sql, handle } = setup();
  const ok = await handle(post("/v1/reports", { app_version: "0.3.1", error: "unknown line", hand: "Hand #1 …" }));
  assertEquals(ok.status, 202);
  assertEquals(sql.db.prepare("SELECT app_version, hand FROM reports").all().length, 1);
  const bad = await handle(post("/v1/reports", { app_version: "0.3.1; drop", error: "x", hand: "y" }));
  assertEquals(bad.status, 400);
  const missing = await handle(post("/v1/reports", { app_version: "0.3.1", error: "x" }));
  assertEquals(missing.status, 400);
});

Deno.test("rate limit answers 429 with Retry-After and spares other clients", async () => {
  const { sql, handle, advance } = setup();
  const report = { app_version: "0.3.1", error: "e", hand: "h" };
  const max = RULES.report[0].max;
  for (let i = 0; i < max; i++) assertEquals((await handle(post("/v1/reports", report))).status, 202);
  for (let i = 0; i < 5; i++) {
    const res = await handle(post("/v1/reports", report));
    assertEquals(res.status, 429);
    assert(Number(res.headers.get("retry-after")) > 0);
    await res.body?.cancel();
  }
  const global = sql.db.prepare("SELECT count FROM rate_limits WHERE bucket = 'report-global:global'").get();
  assertEquals(global!.count, max, "blocked requests do not use up the global quota");
  assertEquals((await handle(post("/v1/reports", report, "198.51.100.9"))).status, 202);
  advance(3600);
  assertEquals((await handle(post("/v1/reports", report))).status, 202, "the next window starts fresh");
});

Deno.test("global limit stops everyone", async () => {
  const { sql, handle } = setup();
  const report = { app_version: "0.3.1", error: "e", hand: "h" };
  const g = RULES.report[1];
  sql.db.prepare("INSERT INTO rate_limits VALUES ('report-global:global', ?, ?)")
    .run(Math.floor(1_000_000 / g.windowS) * g.windowS, g.max);
  const res = await handle(post("/v1/reports", report, "198.51.100.77"));
  assertEquals(res.status, 429);
  assertEquals((await res.json()).message, "too many requests (report-global)");
});
