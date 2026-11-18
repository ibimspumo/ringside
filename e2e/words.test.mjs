// Grammar of the strategy sentences: `node --test e2e/words.test.mjs`.
import assert from "node:assert/strict";
import { test } from "node:test";
import { betterLine, conjugate, infinitive, mixText, often, playsLine } from "../src/lib/play/words.ts";

const KINDS = ["fold", "check", "call", "limp", "bet", "raise", "all_in"];
const TIERS = [0.99, 0.9, 0.7, 0.5, 0.2, 0.07];
const WHO = ["model", "equilibrium", "solution"];
const INFINITIVES = KINDS.map(infinitive).map((v) => v.split(" ").pop());

test("every tier and move reads as a sentence", () => {
  for (const who of WHO) {
    for (const a of KINDS) {
      for (const b of KINDS) {
        if (a === b) continue;
        for (const fa of TIERS) {
          const fb = Math.min(1 - fa, fa);
          const s = playsLine(who, "KJo", [
            { kind: a, f: fa },
            { kind: b, f: fb },
          ]);
          const [verb, rest] = conjugate(a);
          // Finite verb right after the subject, never "spielt … erhöhen".
          assert.match(s, new RegExp(`^(Das Modell|Das Gleichgewicht|Die Lösung) ${verb} mit KJo hier ${often(fa)}${rest ? ` ${rest}` : ""} \\(`), s);
          assert.doesNotMatch(s, /spielt/, s);
          for (const inf of INFINITIVES) assert.ok(!s.split(" ").includes(inf), `${s} has the infinitive ${inf}`);
          if (fb >= 0.05) {
            const [v2, r2] = conjugate(b);
            const pron = who === "solution" ? "sie" : "es";
            assert.ok(s.includes(`, sonst ${v2} ${pron} ${often(fb)}${r2 ? ` ${r2}` : ""} (`), s);
          }
          assert.ok(s.endsWith(").") && !s.includes("  "), s);
        }
      }
    }
  }
});

test("the mixed case answers first and names both moves", () => {
  const s = mixText("model", "KJo", { kind: "raise", f: 0.86, ev: 0.112 }, { kind: "call", f: 0.14, ev: 0.11 });
  assert.equal(
    s,
    "Auch richtig – gleich viel wert wie Raise. Raise und Call bringen hier beide etwa +0,11 BB. Das Modell erhöht fast immer (86 %) und callt manchmal (14 %), damit es schwer zu lesen ist. Einfacher zu merken: mit KJo hier fast immer erhöhen.",
  );
  assert.match(mixText("solution", "AJs", { kind: "all_in", f: 0.7, ev: 1 }, { kind: "call", f: 0.3, ev: 1 }), /Die Lösung geht meistens all-in \(70 %\) und callt manchmal \(30 %\), damit sie/);
});

test("a costly move says the better one first", () => {
  assert.equal(betterLine("check", "bet", 1.153, 12.2), "Besser: Check – dein Bet kostet 1,15 BB (12 % des Pots).");
});
