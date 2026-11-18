import { assert, assertEquals } from "jsr:@std/assert@1";
import { memorySql } from "../lib/sqlite_memory.ts";
import { createHandler, FLOPS_CACHE_S } from "./api.ts";
import { RULES } from "./ratelimit.ts";
import { storedSpot } from "./stored_testutil.ts";

const BASE = "https://ringside-api.test";
const shared = JSON.parse(
  await Deno.readTextFile(new URL("../../crates/ringside-cloud/testdata/spot.json", import.meta.url)),
);
const LINE: Record<string, unknown> = shared.keys[0].input;

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

async function share(handle: (r: Request) => Promise<Response>, input: Record<string, unknown>): Promise<string> {
  const data = await storedSpot("spot", input.board as string, { catalog: 0 });
  const res = await handle(post("/v1/solutions", { kind: "spot", engine_version: 1, input, result: { data } }));
  assertEquals(res.status, 201, await res.clone().text());
  return (await res.json()).key;
}

async function flops(handle: (r: Request) => Promise<Response>, input: Record<string, unknown>): Promise<string[]> {
  const res = await handle(post("/v1/solutions/flops", { kind: "spot", engine_version: 1, input }));
  assertEquals(res.status, 200, await res.clone().text());
  return (await res.json()).flops;
}

Deno.test("the flops of a line list every served flop of that line and nothing else", async () => {
  const { sql, handle } = setup();
  await share(handle, LINE);
  const rejected = await share(handle, { ...LINE, board: "AsKs2h" });
  await share(handle, { ...LINE, board: "7s6h2d" });
  await share(handle, { ...LINE, board: "QsJh4d", pot_cbb: 600 });
  sql.db.prepare("UPDATE solutions SET status = 'rejected' WHERE key = ?").run(rejected);
  // Asked with any flop of the line, also one that has no solution.
  assertEquals(await flops(handle, { ...LINE, board: "AsAh2d" }), ["7s6h2d", "Ks7h2s"]);
  assertEquals(await flops(handle, { ...LINE, board: "QsJh4d", pot_cbb: 600 }), ["QsJh4d"]);
  assertEquals(await flops(handle, { ...LINE, stack_cbb: 3000 }), []);
});

Deno.test("the flops of a line are kept for a while, then asked again", async () => {
  const { handle, advance } = setup();
  await share(handle, LINE);
  assertEquals(await flops(handle, LINE), ["Ks7h2s"]);
  await share(handle, { ...LINE, board: "7s6h2d" });
  assertEquals(await flops(handle, LINE), ["Ks7h2s"], "from the isolate's cache");
  advance(FLOPS_CACHE_S);
  assertEquals(await flops(handle, LINE), ["7s6h2d", "Ks7h2s"]);
});

Deno.test("flops are listed for model spots only, with a valid input and their own rate limit", async () => {
  const { handle } = setup();
  for (
    const body of [
      { kind: "postflop", engine_version: 1, input: { catalog: 1, scenario: "srp-btn-bb-20", flop: "AsKs2h" } },
      { kind: "spot", engine_version: 1, input: { ...LINE, board: "Kh7c2h" } },
      { kind: "spot", engine_version: 1, input: { ...LINE, player: "Hero" } },
      { kind: "spot", engine_version: 1 },
    ]
  ) {
    const res = await handle(post("/v1/solutions/flops", body));
    assertEquals(res.status, 400, JSON.stringify(body));
    await res.body?.cancel();
  }
  const get = await handle(new Request(BASE + "/v1/solutions/flops", { headers: { "X-Real-IP": "203.0.113.7" } }));
  assertEquals(get.status, 405);
  await get.body?.cancel();
  for (let i = 0; i < RULES.flops[0].max - 4; i++) await flops(handle, LINE);
  const res = await handle(post("/v1/solutions/flops", { kind: "spot", engine_version: 1, input: LINE }));
  assertEquals(res.status, 429);
  assert(Number(res.headers.get("retry-after")) > 0);
  await res.body?.cancel();
  assertEquals((await flops(handle, LINE).catch(() => null)), null, "still limited");
  const other = await handle(post("/v1/solutions/flops", { kind: "spot", engine_version: 1, input: LINE }, "198.51.100.9"));
  assertEquals(other.status, 200, "another client is not limited");
  await other.body?.cancel();
});
