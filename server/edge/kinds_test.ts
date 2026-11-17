import { assertEquals, assertThrows } from "jsr:@std/assert@1";
import { canonicalize, solutionKey } from "./canonical.ts";
import { InvalidError, kindSpec, POSTFLOP_CATALOG, POSTFLOP_FLOPS, POSTFLOP_SCENARIOS } from "./kinds.ts";

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

Deno.test("postflop results need a matching header and a size limit", () => {
  for (const h of shared.headers) {
    const run = () => spec.normalizeResult({ data: h.data }, h.input);
    if (h.valid) assertEquals(run(), { data: h.data }, h.what);
    else assertThrows(run, InvalidError, undefined, h.what);
  }
  const input = shared.headers[0].input;
  assertThrows(() => spec.normalizeResult({ data: "A".repeat(300_000) }, input), InvalidError);
  assertThrows(() => spec.normalizeResult({ data: shared.headers[0].data, extra: 1 }, input), InvalidError);
});
