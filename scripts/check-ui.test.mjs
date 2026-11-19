// Own test of the UI ratchet: runs it on a small fixture tree and reads its findings.
import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const script = fileURLToPath(new URL("./check-ui.mjs", import.meta.url));

function findings(files) {
  const root = mkdtempSync(join(tmpdir(), "check-ui-"));
  mkdirSync(join(root, "app"));
  writeFileSync(join(root, "theme.css"), ".btn { color: red; }\n");
  for (const [name, text] of Object.entries(files)) {
    mkdirSync(dirname(join(root, name)), { recursive: true });
    writeFileSync(join(root, name), text);
  }
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

test("time spans are written by the one formatter", () => {
  const out = findings({
    "app/A.svelte": '<script>\n  const t = `${m} Min.`;\n</script>\n<p>{h} h</p>\n<p>noch {x} Std.</p>\n',
    "app/b.ts": "export const eta = (s: number) => `${(s / 3600).toFixed(1)} h`;\n",
    "ui/format.ts": "export const d = (m: number) => `${m} Min.`;\n",
    "app/C.svelte": "<p>Minimum {x} Minuten, {name} hätte</p>\n",
  });
  const lines = out.filter((l) => l.includes("[time-format]")).map((l) => l.split("  ")[0]);
  assert.deepEqual(lines.sort(), ["app/A.svelte:2", "app/A.svelte:4", "app/A.svelte:5", "app/b.ts:1", "app/b.ts:1"]);
});

test("toFixed is display text in markup and before a unit, not in paths and styles", () => {
  const out = findings({
    "app/A.svelte": '<script>\n  const d = `M${x.toFixed(1)},${y.toFixed(1)}`;\n  const k = `${(v / 1000).toFixed(1)}k`;\n</script>\n<p>{share.toFixed(1)} %</p>\n<i style="width:{w.toFixed(2)}%"></i>\n',
    "app/b.ts": "export const s = (v: number) => `${v.toFixed(2)} BB`;\nexport const c = (p: number) => `${p.toFixed(1)}%`;\n",
  });
  const lines = out.filter((l) => l.includes("[raw-number]")).map((l) => l.split("  ")[0]);
  assert.deepEqual(lines.sort(), ["app/A.svelte:3", "app/A.svelte:5", "app/A.svelte:6", "app/b.ts:1"]);
});

test("no capitals, not even in table column heads", () => {
  const out = findings({
    "app/A.svelte": style("text-transform: uppercase;"),
    "ui/B.svelte": style("text-transform: uppercase;"),
    "ui/SortHeader.svelte": style("text-transform: uppercase;"),
    "app/C.svelte": style("text-transform: none;"),
  });
  assert.deepEqual(out.filter((l) => l.includes("[caps-label]")).map((l) => l.split("  ")[0]).sort(), ["app/A.svelte:3", "ui/B.svelte:3", "ui/SortHeader.svelte:3"]);
});

test("state titles: no full stop, loading ends on an ellipsis", () => {
  const out = findings({
    "app/A.svelte": '<EmptyState title="Noch keine Hände" />\n<EmptyState tone="loading" title="Spieler werden geladen …" />\n<EmptyState title={x} />\n',
    "app/B.svelte": '<EmptyState\n  tone="error"\n  title="Das ging schief."\n/>\n<EmptyState tone="loading" title="Lädt" />\n',
  });
  assert.deepEqual(out.filter((l) => l.includes("[state-title]")).map((l) => l.split("  ")[0]), ["app/B.svelte:1", "app/B.svelte:5"]);
});
