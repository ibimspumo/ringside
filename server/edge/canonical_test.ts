import { assertEquals, assertThrows } from "jsr:@std/assert@1";
import { CanonicalError, canonicalize, solutionKey } from "./canonical.ts";

const vectors = JSON.parse(
  Deno.readTextFileSync(new URL("../../crates/ringside-cloud/testdata/canonical.json", import.meta.url)),
);

Deno.test("canonical form and key match the shared vectors", async () => {
  for (const v of vectors.valid) {
    assertEquals(canonicalize(v.input), v.canonical);
    assertEquals(await solutionKey(v.kind, v.engine_version, v.canonical), v.key);
  }
});

Deno.test("canonical form does not depend on key order", () => {
  assertEquals(canonicalize({ b: 1, a: [2, { d: 3, c: 4 }] }), canonicalize({ a: [2, { c: 4, d: 3 }], b: 1 }));
});

Deno.test("values with more than one spelling are rejected", () => {
  for (const bad of vectors.invalid) assertThrows(() => canonicalize(bad), CanonicalError);
  assertThrows(() => canonicalize(Number.NaN), CanonicalError);
  assertThrows(() => canonicalize(undefined), CanonicalError);
});

Deno.test("key changes with kind, engine version and input", async () => {
  const c = canonicalize({ stack_mbb: 10000, sb_mbb: 500, ante_mbb: 0, dead_mbb: 0 });
  const k = await solutionKey("pushfold-hu", 1, c);
  const others = [
    await solutionKey("pushfold-hu", 2, c),
    await solutionKey("pushfold-mw", 1, c),
    await solutionKey("pushfold-hu", 1, canonicalize({ stack_mbb: 10001, sb_mbb: 500, ante_mbb: 0, dead_mbb: 0 })),
  ];
  for (const o of others) if (o === k) throw new Error("key collision");
});
