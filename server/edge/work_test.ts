import { assert, assertEquals } from "jsr:@std/assert@1";
import { memorySql } from "../lib/sqlite_memory.ts";
import { createHandler, LEASE_S, MAX_LEASES } from "./api.ts";
import { canonicalize, solutionKey } from "./canonical.ts";
import { RULES } from "./ratelimit.ts";

const BASE = "https://ringside-api.test";
const shared = JSON.parse(
  await Deno.readTextFile(new URL("../../crates/ringside-cloud/testdata/spot.json", import.meta.url)),
);
const SPOT = shared.keys[0].input as Record<string, unknown>;

function setup() {
  const sql = memorySql();
  let t = 1_000_000;
  const handle = createHandler({ sql, salt: "test-salt", now: () => t });
  return { sql, handle, advance: (s: number) => (t += s) };
}

function post(path: string, body: unknown, ip = "203.0.113.7"): Request {
  return new Request(BASE + path, {
    method: "POST",
    headers: { "Content-Type": "application/json", "X-Real-IP": ip },
    body: JSON.stringify(body),
  });
}

/** A catalog spot in the queue, as ringside-queue seeds it. */
async function seed(sql: ReturnType<typeof memorySql>, flop: string, at = 0, kind = "postflop") {
  const input = canonicalize({ catalog: 1, scenario: "srp-btn-bb-40", flop });
  const key = await solutionKey(kind, 1, input);
  sql.db.prepare(
    "INSERT INTO work (key, kind, engine_version, input, source, priority, created_at) VALUES (?, ?, 1, ?, 'catalog', 0, ?)",
  ).run(key, kind, input, at);
  return key;
}

const next = (kinds: Record<string, number[]> = { postflop: [1], spot: [1] }) => ({ kinds });

Deno.test("a leased spot goes to nobody else until the lease runs out, then again", async () => {
  const { sql, handle, advance } = setup();
  const key = await seed(sql, "AsKhTd");
  const a = await (await handle(post("/v1/work/next", next(), "198.51.100.1"))).json();
  assertEquals(a.work.key, key);
  assertEquals(a.work.input, { catalog: 1, scenario: "srp-btn-bb-40", flop: "AsKhTd" });
  assertEquals(a.work.lease_until, 1_000_000 + LEASE_S.postflop);
  assertEquals((await (await handle(post("/v1/work/next", next(), "198.51.100.2"))).json()).work, null);

  advance(LEASE_S.postflop);
  const b = await (await handle(post("/v1/work/next", next(), "198.51.100.2"))).json();
  assertEquals(b.work.key, key, "an expired lease is handed out again");
  const row = sql.db.prepare("SELECT leases FROM work WHERE key = ?").get(key)!;
  assertEquals(row.leases, 2);
});

Deno.test("a spot with a submitted or verified solution is not handed out", async () => {
  const { sql, handle } = setup();
  const key = await seed(sql, "AsKhTd");
  for (const status of ["rejected", "submitted", "verified"]) {
    sql.db.prepare("DELETE FROM solutions").run();
    sql.db.prepare(
      "INSERT INTO solutions (key, kind, engine_version, input, result, result_size, status, submitter, submitted_at) " +
        "VALUES (?, 'postflop', 1, '{}', x'00', 1, ?, 'x', 1)",
    ).run(key, status);
    sql.db.prepare("UPDATE work SET lease_until = NULL").run();
    const got = (await (await handle(post("/v1/work/next", next()))).json()).work;
    assertEquals(got?.key ?? null, status === "rejected" ? key : null, status);
  }
});

Deno.test("only kinds the client names are handed out", async () => {
  const { sql, handle } = setup();
  await seed(sql, "AsKhTd");
  assertEquals((await (await handle(post("/v1/work/next", next({ "pushfold-mw": [1] })))).json()).work, null);
  for (const bad of [{}, { postflop: [2] }, { nope: [1] }, { postflop: 1 }]) {
    assertEquals((await handle(post("/v1/work/next", { kinds: bad }))).status, 400, JSON.stringify(bad));
  }
});

Deno.test("a client holds a bounded number of leases; more frees its oldest", async () => {
  const { sql, handle, advance } = setup();
  for (const [i, f] of ["AsKhTd", "AsKh5d", "AsQhTd", "AsQh5d", "AsJh9d"].entries()) await seed(sql, f, i);
  for (let i = 0; i < MAX_LEASES + 1; i++) {
    assert((await (await handle(post("/v1/work/next", next(), "198.51.100.1"))).json()).work);
    advance(1);
  }
  const held = sql.db.prepare("SELECT COUNT(*) AS n FROM work WHERE leased_to IS NOT NULL AND lease_until > ?")
    .get(1_000_000 + MAX_LEASES + 1)!;
  assertEquals(held.n, MAX_LEASES);
});

Deno.test("requested spots are queued once and rise with every request", async () => {
  const { sql, handle } = setup();
  const body = { kind: "spot", engine_version: 1, input: SPOT };
  const first = await (await handle(post("/v1/work/request", body))).json();
  assertEquals(first, { key: shared.keys[0].key, status: "queued" });
  await handle(post("/v1/work/request", body, "198.51.100.9"));
  const row = sql.db.prepare("SELECT source, priority, input FROM work WHERE key = ?").get(first.key)!;
  assertEquals([row.source, row.priority, row.input], ["request", 2, shared.keys[0].canonical]);
  const got = (await (await handle(post("/v1/work/next", next({ spot: [1] })))).json()).work;
  assertEquals([got.key, got.input], [first.key, SPOT], "requested spots go out first");

  sql.db.prepare("DELETE FROM work").run();
  sql.db.prepare(
    "INSERT INTO solutions (key, kind, engine_version, input, result, result_size, status, submitter, submitted_at) " +
      "VALUES (?, 'spot', 1, '{}', x'00', 1, 'verified', 'x', 1)",
  ).run(first.key);
  assertEquals((await (await handle(post("/v1/work/request", body))).json()).status, "verified");
  assertEquals(sql.db.prepare("SELECT COUNT(*) AS n FROM work").get()!.n, 0, "a solved spot is not queued");

  const catalog = { kind: "postflop", engine_version: 1, input: { catalog: 1, scenario: "srp-btn-bb-40", flop: "AsKhTd" } };
  assertEquals((await handle(post("/v1/work/request", catalog))).status, 400, "only spots can be requested");
  assertEquals((await handle(post("/v1/work/request", { ...body, input: { ...SPOT, hand_id: 1 } }))).status, 400);
});

Deno.test("work and requests have their own rate limits", async () => {
  const { handle } = setup();
  const max = RULES.work.find((r) => r.name === "work-min")!.max;
  for (let i = 0; i < max; i++) assertEquals((await handle(post("/v1/work/next", next()))).status, 200);
  const limited = await handle(post("/v1/work/next", next()));
  assertEquals(limited.status, 429);
  assert(Number(limited.headers.get("retry-after")) > 0);
  assertEquals((await handle(post("/v1/work/next", next(), "198.51.100.3"))).status, 200, "others go on");

  const body = { kind: "spot", engine_version: 1, input: SPOT };
  const perHour = RULES.request.find((r) => r.name === "request-hour")!.max;
  for (let i = 0; i < perHour; i++) assertEquals((await handle(post("/v1/work/request", body))).status, 202);
  assertEquals((await handle(post("/v1/work/request", body))).status, 429);
});

Deno.test("a shared spot is served only once the verifier has checked it", async () => {
  const { sql, handle } = setup();
  const bytes = [..."RSLS"].map((c) => c.charCodeAt(0));
  bytes.push(1, 0, 0, 4, ...[..."spot" + (SPOT.board as string)].map((c) => c.charCodeAt(0)));
  while (bytes.length % 3 !== 0) bytes.push(0);
  const data = btoa(String.fromCharCode(...bytes));
  const res = await handle(post("/v1/solutions", { kind: "spot", engine_version: 1, input: SPOT, result: { data } }));
  assertEquals(res.status, 202);
  const { key } = await res.json();
  assertEquals(key, shared.keys[0].key);
  const url = `${BASE}/v1/solutions/spot/${key}`;
  assertEquals((await handle(new Request(url))).status, 404, "submitted is not served");
  sql.db.prepare("UPDATE solutions SET status = 'rejected' WHERE key = ?").run(key);
  assertEquals((await handle(new Request(url))).status, 404, "rejected is not served");
  sql.db.prepare("UPDATE solutions SET status = 'verified', exploitability = 0.8, checked_at = 2 WHERE key = ?").run(key);
  const served = await handle(new Request(url));
  assertEquals(served.status, 200);
  assertEquals((await served.json()).result, { data });
});
