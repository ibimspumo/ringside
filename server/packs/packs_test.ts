import { assertEquals, assertThrows } from "jsr:@std/assert@1";
import { addVersion, generateKeyPair, type Index, publicKeyOf, removeVersion, sign, verifySignature } from "./packs.ts";

const data = (s: string) => new TextEncoder().encode(s) as Uint8Array<ArrayBuffer>;

Deno.test("signatures verify and catch any change", async () => {
  const { privateKey, publicKey } = await generateKeyPair();
  assertEquals(await publicKeyOf(privateKey), publicKey);
  const sig = await sign(privateKey, data('{"serial":1}'));
  assertEquals(await verifySignature(publicKey, data('{"serial":1}'), sig), true);
  assertEquals(await verifySignature(publicKey, data('{"serial":2}'), sig), false);
  const other = await generateKeyPair();
  assertEquals(await verifySignature(other.publicKey, data('{"serial":1}'), sig), false);
});

Deno.test("index versions are immutable and every change bumps the serial", () => {
  const empty: Index = { format: "ringside-packs-index/1", serial: 0, updated_at: "", packs: {} };
  const files = [{ path: "a.bin", size: 1, sha256: "00" }];
  const one = addVersion(empty, "preflop", "1.0.0", files, new Date(0));
  assertEquals(one.serial, 1);
  assertThrows(() => addVersion(one, "preflop", "1.0.0", files, new Date(0)), Error, "immutable");
  assertThrows(() => addVersion(one, "Preflop", "1.0.1", files, new Date(0)));
  assertThrows(() => addVersion(one, "preflop", "1.0", files, new Date(0)));
  const two = addVersion(one, "preflop", "1.1.0", files, new Date(0));
  const three = removeVersion(two, "preflop", "1.0.0", new Date(0));
  assertEquals([three.serial, Object.keys(three.packs.preflop)], [3, ["1.1.0"]]);
  assertEquals(removeVersion(three, "preflop", "1.1.0", new Date(0)).packs, {});
  assertEquals(empty.packs, {}, "input index is not mutated");
});
