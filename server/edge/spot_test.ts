import { assert, assertEquals, assertThrows } from "jsr:@std/assert@1";
import { canonicalize, solutionKey } from "./canonical.ts";
import { canonicalFlop, InvalidError, kindSpec } from "./kinds.ts";

interface Shared {
  keys: { input: Record<string, unknown>; canonical: string; key: string }[];
  flops: [string, string][];
}

const shared: Shared = JSON.parse(
  await Deno.readTextFile(new URL("../../crates/ringside-cloud/testdata/spot.json", import.meta.url)),
);
const spec = kindSpec("spot")!;

Deno.test("the same spot in the plain and the limps family has two keys", () => {
  const [plain, limps] = shared.keys;
  assertEquals([plain.input.family, limps.input.family], ["plain", "limps"]);
  assertEquals({ ...plain.input, family: "limps" }, limps.input);
  assert(plain.key !== limps.key);
});

Deno.test("spot keys match Rust", async () => {
  for (const k of shared.keys) {
    spec.checkInput(k.input);
    assertEquals(canonicalize(k.input), k.canonical);
    assertEquals(await solutionKey("spot", 1, k.canonical), k.key);
  }
});

Deno.test("every suit renaming of a flop has the canonical flop of Rust", () => {
  for (const [flop, canonical] of shared.flops) assertEquals(canonicalFlop(flop), canonical, flop);
  const suits = ["s", "h", "d", "c"];
  const seen = new Set<string>();
  for (const a of suits) for (const b of suits) for (const c of suits) seen.add(canonicalFlop(`K${a}7${b}2${c}`));
  assertEquals([...seen].sort(), ["Ks7h2d", "Ks7h2h", "Ks7h2s", "Ks7s2h", "Ks7s2s"], "64 suitings, five classes");
});

Deno.test("spot inputs off the model's shape are refused", () => {
  const good = shared.keys[0].input;
  const bad: Record<string, unknown>[] = [
    { ...good, board: "Kh7c2h" },
    { ...good, model: 1 },
    { ...good, family: "other" },
    { ...good, players: 7 },
    { ...good, family: "plain", players: 4 },
    { ...good, stack_bb: 41 },
    { ...good, line: [] },
    { ...good, line: [[9, "raise", 2200]] },
    { ...good, line: [[5, "fold", 0]] },
    { ...good, sizes: [[75, 33], [], [], [], [], []] },
    { ...good, sizes: [[10, 20, 33, 50, 75], [], [], [], [], []] },
    { ...good, pot_cbb: 99 },
    { ...good, hand_id: 260000000002 },
    { ...good, player: "Hero" },
    { ...good, played_at: 1763000000 },
  ];
  for (const b of bad) assertThrows(() => spec.checkInput(b), InvalidError, undefined, JSON.stringify(b));
});

Deno.test("a stored spot must carry the spot header with the input's board", () => {
  const input = shared.keys[0].input;
  const head = (catalog: number, scenario: string, flop: string) => {
    const bytes = [..."RSLS"].map((c) => c.charCodeAt(0));
    bytes.push(1, catalog & 255, catalog >> 8, scenario.length, ...[...scenario + flop].map((c) => c.charCodeAt(0)));
    while (bytes.length % 3 !== 0) bytes.push(0);
    return btoa(String.fromCharCode(...bytes));
  };
  spec.normalizeResult({ data: head(0, "spot", input.board as string) }, input);
  assertThrows(() => spec.normalizeResult({ data: head(1, "spot", input.board as string) }, input), InvalidError);
  assertThrows(() => spec.normalizeResult({ data: head(0, "srp-btn-bb-40", input.board as string) }, input), InvalidError);
  assertThrows(() => spec.normalizeResult({ data: head(0, "spot", "AsKs2h") }, input), InvalidError);
});
