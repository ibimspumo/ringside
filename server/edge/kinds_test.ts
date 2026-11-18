import { assertAlmostEquals, assertEquals, assertRejects, assertThrows } from "jsr:@std/assert@1";
import { canonicalize, solutionKey } from "./canonical.ts";
import {
  checkHeader,
  InvalidError,
  kindSpec,
  POSTFLOP_CATALOG,
  POSTFLOP_FLOPS,
  POSTFLOP_SCENARIOS,
  POSTFLOP_TOLERANCE,
} from "./kinds.ts";
import { deflateRaw, storedSpot } from "./stored_testutil.ts";

interface Shared {
  catalog: number;
  scenarios: string[];
  flops: string[];
  hashes: { input: unknown; canonical: string; solution_hash: string }[];
  headers: { what: string; input: unknown; data: string; valid: boolean }[];
}

const shared: Shared = JSON.parse(
  await Deno.readTextFile(new URL("../../crates/ringside-cloud/testdata/postflop.json", import.meta.url)),
);
const spec = kindSpec("postflop")!;

Deno.test("postflop catalog matches the shared vectors", () => {
  assertEquals(POSTFLOP_CATALOG, shared.catalog);
  assertEquals([...POSTFLOP_SCENARIOS], shared.scenarios);
  assertEquals([...POSTFLOP_FLOPS], shared.flops);
});

Deno.test("postflop keys match Rust", async () => {
  for (const k of shared.hashes) {
    spec.checkInput(k.input);
    const canonical = canonicalize(k.input);
    assertEquals(canonical, k.canonical);
    assertEquals(await solutionKey("postflop", 1, canonical), k.solution_hash);
  }
});

Deno.test("postflop accepts catalog spots only", () => {
  const ok = { catalog: 1, scenario: shared.scenarios[0], flop: shared.flops[0] };
  spec.checkInput(ok);
  for (const bad of [
    { ...ok, catalog: 2 },
    { ...ok, scenario: "srp-btn-bb-41" },
    { ...ok, flop: "AhKdTc" },
    { ...ok, extra: 1 },
  ]) {
    assertThrows(() => spec.checkInput(bad), InvalidError);
  }
});

Deno.test("postflop results need a header for their input", () => {
  for (const h of shared.headers) {
    const run = () => checkHeader(Uint8Array.from(atob(h.data), (c) => c.charCodeAt(0)), h.input as never);
    if (h.valid) run();
    else assertThrows(run, Error, undefined, h.what);
  }
});

Deno.test("a well-formed stored spot is accepted with the exploitability it reports", async () => {
  const input = { catalog: 1, scenario: "srp-btn-bb-40", flop: "AsKhTd" };
  const data = await storedSpot(input.scenario, input.flop, { exploitability: 0.44 });
  const n = await spec.normalizeResult({ data }, input);
  assertEquals(n.result, { data });
  assertAlmostEquals(n.exploitability!, 0.44, 1e-6);
});

Deno.test("every violation of the stored spot's form is refused", async () => {
  const input = { catalog: 1, scenario: "srp-btn-bb-40", flop: "AsKhTd" };
  const spot = (o = {}) => storedSpot(input.scenario, input.flop, o);
  const nan = new Float32Array([NaN])[0];
  const cases: [string, unknown][] = [
    ["extra field", { data: await spot(), extra: 1 }],
    ["no data", {}],
    ["empty data", { data: "" }],
    ["not base64", { data: "!!!!" }],
    ["too large", { data: "A".repeat(300_000) }],
    ["other flop", { data: await storedSpot(input.scenario, "AsKh5d") }],
    ["other scenario", { data: await storedSpot("srp-co-bb-40", input.flop) }],
    ["other catalog", { data: await spot({ catalog: 2 }) }],
    ["exploitability above the tolerance", { data: await spot({ exploitability: POSTFLOP_TOLERANCE * 1.01 }) }],
    ["negative exploitability", { data: await spot({ exploitability: -0.1 }) }],
    ["exploitability not a number", { data: await spot({ exploitability: nan }) }],
    ["infinite exploitability", { data: await spot({ exploitability: Infinity }) }],
    ["nothing after the header", { data: await spot({ tail: new Uint8Array() }) }],
    ["body does not inflate", { data: await spot({ tail: new Uint8Array([0xff, 0xff, 0xff, 0xff]) }) }],
    ["body inflates to nothing", { data: await spot({ tail: await deflateRaw(new Uint8Array()) }) }],
    ["body inflates beyond the limit", { data: await spot({ tail: await deflateRaw(new Uint8Array(5 << 20)) }) }],
  ];
  for (const [what, result] of cases) {
    await assertRejects(() => spec.normalizeResult(result, input), InvalidError, undefined, what);
  }
});

Deno.test("shared spots report within their own tolerance", async () => {
  const spot = kindSpec("spot")!;
  const input = { board: "AsKhTd" };
  const ok = await storedSpot("spot", "AsKhTd", { catalog: 0, exploitability: 1.5 });
  assertEquals((await spot.normalizeResult({ data: ok }, input)).result, { data: ok });
  const high = await storedSpot("spot", "AsKhTd", { catalog: 0, exploitability: 2.5 });
  await assertRejects(() => spot.normalizeResult({ data: high }, input), InvalidError);
});
