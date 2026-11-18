#!/usr/bin/env node
// Contrast of the 13×13 grid labels (src/lib/ui/RangeGrid.svelte). For every fill colour and
// fill height it composites what lies under the label (cell, fill, faded fill, the label plate)
// from the tokens in theme.css and requires WCAG 4.5:1.
//
//   node scripts/check-contrast.mjs [--theme <file>] [--table]

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const args = process.argv.slice(2);
const at = args.indexOf("--theme");
const repo = join(fileURLToPath(import.meta.url), "..", "..");
const themePath = at >= 0 ? args[at + 1] : join(repo, "src", "lib", "theme.css");
const MIN = 4.5;

/** Every custom property of the :root blocks; later definitions win. */
function tokens(css) {
  const out = new Map();
  for (const block of css.replace(/\/\*[\s\S]*?\*\//g, "").matchAll(/:root\s*\{([^}]*)\}/g)) {
    for (const m of block[1].matchAll(/(--[\w-]+)\s*:\s*([^;]+);/g)) out.set(m[1], m[2].trim());
  }
  return out;
}

const vars = tokens(readFileSync(themePath, "utf8"));

function resolve(value, depth = 0) {
  const v = value.trim();
  const m = /^var\((--[\w-]+)\)$/.exec(v);
  if (!m) return v;
  if (depth > 10 || !vars.has(m[1])) throw new Error(`Token ${m[1]} fehlt in theme.css`);
  return resolve(vars.get(m[1]), depth + 1);
}

/** [r, g, b, a] with r–b in 0–255 and a in 0–1. */
function rgba(value) {
  const v = resolve(value);
  let m = /^#([0-9a-f]{3,8})$/i.exec(v);
  if (m) {
    let h = m[1];
    if (h.length <= 4) h = [...h].map((c) => c + c).join("");
    const n = (i) => parseInt(h.slice(i, i + 2), 16);
    return [n(0), n(2), n(4), h.length === 8 ? n(6) / 255 : 1];
  }
  m = /^rgba?\(([^)]+)\)$/i.exec(v);
  if (m) {
    const [r, g, b, a = "1"] = m[1].split(/[\s,/]+/).filter(Boolean);
    return [Number(r), Number(g), Number(b), Number(a)];
  }
  throw new Error(`Farbe nicht lesbar: ${value} → ${v}`);
}

const over = (top, bottom) => {
  const a = top[3];
  return [0, 1, 2].map((i) => top[i] * a + bottom[i] * (1 - a)).concat(1);
};
const withAlpha = (c, a) => [c[0], c[1], c[2], c[3] * a];

function luminance([r, g, b]) {
  const ch = (x) => {
    const s = x / 255;
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * ch(r) + 0.7152 * ch(g) + 0.0722 * ch(b);
}
function ratio(a, b) {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

const bg = rgba("var(--bg)");
const cell = over(rgba("var(--grid-cell)"), bg);
const cellOut = over(rgba("var(--grid-cell-out)"), bg);
const plate = rgba("var(--grid-plate)");
const label = rgba("var(--grid-label)");
const labelOut = rgba("var(--grid-label-out)");
const dim = Number(resolve("var(--grid-dim)"));

const FILL_TOKENS = [...vars.keys()].filter((k) => /^--act-/.test(k)).concat(["--pos", "--neg", "--sd", "--amber", "--range-wide", "--range-tight"]);
const HEIGHTS = [0, 0.25, 0.5, 0.75, 1];

/** What can lie under the label at the top of a cell filled `h` from the bottom. Small cells
 * put the label low enough that any partial fill may reach it, so both count. */
const under = (fill, h) => (h === 0 ? [cell] : h === 1 ? [fill] : [cell, fill]);

const rows = [];
let worst = Infinity;
for (const token of FILL_TOKENS) {
  const fill = over(rgba(`var(${token})`), cell);
  const faded = over(withAlpha(rgba(`var(${token})`), dim), cell);
  const row = { token, values: [] };
  for (const h of HEIGHTS) {
    const backs = [...under(fill, h), ...under(faded, h)];
    const r = Math.min(...backs.map((b) => ratio(over(label, over(plate, b)), over(plate, b))));
    row.values.push(r);
    worst = Math.min(worst, r);
  }
  rows.push(row);
}
const outside = ratio(over(labelOut, cellOut), cellOut);
worst = Math.min(worst, outside);

const failing = rows.flatMap((r) => r.values.map((v, k) => ({ token: r.token, h: HEIGHTS[k], v }))).filter((x) => x.v < MIN);

if (args.includes("--table") || failing.length || outside < MIN) {
  console.log(`Füllung        ${HEIGHTS.map((h) => `${String(h * 100).padStart(3)} %`).join("   ")}`);
  for (const r of rows) console.log(`${r.token.padEnd(14)} ${r.values.map((v) => v.toFixed(2).padStart(5)).join("  ")}`);
  console.log(`nicht in Range ${outside.toFixed(2).padStart(5)}`);
}

if (failing.length || outside < MIN) {
  for (const f of failing) console.error(`✗ Raster-Beschriftung auf ${f.token} bei ${f.h * 100} % Füllung: ${f.v.toFixed(2)} : 1 (mindestens ${MIN} : 1)`);
  if (outside < MIN) console.error(`✗ Beschriftung außerhalb der Range: ${outside.toFixed(2)} : 1`);
  process.exit(1);
}
console.log(`check-contrast: ok (Raster-Beschriftung mindestens ${worst.toFixed(2)} : 1 über ${FILL_TOKENS.length} Farben × ${HEIGHTS.length} Füllhöhen)`);
