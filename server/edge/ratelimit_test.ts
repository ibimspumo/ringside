import { assertEquals } from "jsr:@std/assert@1";
import { bucket, decide, MemoryCounter, type Rule, windowStart } from "./ratelimit.ts";

const perMin: Rule = { name: "m", scope: "client", windowS: 60, max: 3 };
const perHour: Rule = { name: "h", scope: "global", windowS: 3600, max: 100 };

Deno.test("windows are aligned to their length", () => {
  assertEquals(windowStart(3599, 60), 3540);
  assertEquals(windowStart(3600, 3600), 3600);
});

Deno.test("buckets separate clients but share the global one", () => {
  assertEquals(bucket(perMin, "abc"), "m:abc");
  assertEquals(bucket(perHour, "abc"), bucket(perHour, "xyz"));
});

Deno.test("at the limit is allowed, one above is not", () => {
  assertEquals(decide([{ rule: perMin, count: 3 }], 100), { ok: true });
  assertEquals(decide([{ rule: perMin, count: 4 }], 100), { ok: false, retryAfter: 20, rule: "m" });
});

Deno.test("retry waits for the latest window that is over", () => {
  const d = decide([{ rule: perMin, count: 9 }, { rule: perHour, count: 101 }], 3610);
  assertEquals(d, { ok: false, retryAfter: 3590, rule: "h" });
});

Deno.test("retry is at least one second", () => {
  assertEquals(decide([{ rule: perMin, count: 4 }], 119.5), { ok: false, retryAfter: 1, rule: "m" });
});

Deno.test("memory counter counts per client and window", () => {
  const c = new MemoryCounter();
  assertEquals([c.hit(perMin, "a", 0), c.hit(perMin, "a", 59), c.hit(perMin, "b", 59)], [1, 2, 1]);
  assertEquals(c.hit(perMin, "a", 60), 1);
});

Deno.test("memory counter stays bounded", () => {
  const c = new MemoryCounter(2);
  c.hit(perMin, "a", 0);
  c.hit(perMin, "b", 0);
  assertEquals(c.hit(perMin, "c", 0), 1);
  assertEquals(c.hit(perMin, "a", 0), 1, "full map is dropped, not grown");
});
