// Own test of the UI ratchet: runs it on a small fixture tree and reads its findings.
import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const script = fileURLToPath(new URL("./check-ui.mjs", import.meta.url));

function findings(files) {
  const root = mkdtempSync(join(tmpdir(), "check-ui-"));
  mkdirSync(join(root, "app"));
  writeFileSync(join(root, "theme.css"), ".btn { color: red; }\n");
  for (const [name, text] of Object.entries(files)) writeFileSync(join(root, name), text);
  const baseline = join(root, "baseline.json");
  const run = spawnSync(process.execPath, [script, "--root", root, "--baseline", baseline, "--init", "--list"], { encoding: "utf8" });
  assert.equal(run.status, 0, run.stderr);
  return run.stdout.split("\n").filter((l) => l.includes("["));
}

const style = (body) => `<div class="x"></div>\n<style>\n  .x {\n    ${body}\n  }\n</style>\n`;

test("motion tokens are not free motion", () => {
  const out = findings({
    "app/A.svelte": style("transition: opacity var(--dur) var(--ease-enter);"),
    "app/B.svelte": style("animation: grow var(--dur-slow) var(--ease-standard) both;"),
    "app/C.svelte": style("transition: transform var(--dur-fast) var(--ease-exit), color var(--dur) var(--ease-spring);"),
  });
  assert.deepEqual(out.filter((l) => l.includes("[motion]")), []);
});

test("keyword curves, linear and literal durations are free motion", () => {
  const out = findings({
    "app/A.svelte": style("transition: opacity 0.2s ease-out;"),
    "app/B.svelte": style("animation: spin var(--dur-loop) linear infinite;"),
    "app/C.svelte": style("transition: color var(--dur) ease;"),
    "app/D.svelte": style("transition: color 200ms var(--ease-standard);"),
    "app/E.svelte": style("transition: color var(--dur);"),
  });
  assert.equal(out.filter((l) => l.includes("[motion]")).length, 5, out.join("\n"));
});

test("poker words in markup text need a Term", () => {
  const out = findings({
    "app/A.svelte": '<p>Dein VPIP ist hoch.</p>\n<p>Deine <Term id="range">Range</Term> passt.</p>\n',
    "app/B.svelte": '<script>\n  const x = "Equity";\n</script>\n<p>{x} und {"EV"}</p>\n<span title="3-Bet">Erhöhen</span>\n',
  });
  assert.deepEqual(out.filter((l) => l.includes("[jargon]")).map((l) => l.split("  ")[0]), ["app/A.svelte:1"]);
});

test("old words count as display text, not as names of files or components", () => {
  const out = findings({
    "app/A.svelte": '<script>\n  import Leaks from "./Leaks.svelte";\n  const t = "Deine Leaks";\n</script>\n<Leaks />\n<Panel sub="EV-Verlust je Hand" />\n<Term tip="x">Leak</Term>\n',
    "app/b.ts": 'export const label = `3 Leaks`;\nimport x from "../Leaks.svelte";\n',
  });
  const lines = out.filter((l) => l.includes("[old-word]")).map((l) => l.split("  ")[0]);
  assert.deepEqual(lines.sort(), ["app/A.svelte:3", "app/A.svelte:6", "app/A.svelte:7", "app/b.ts:1"]);
});
