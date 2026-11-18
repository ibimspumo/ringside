#!/usr/bin/env node
// UI ratchet for src/lib: counts design-system violations per rule and compares the totals with
// scripts/ui-baseline.json. More violations than the baseline fail; fewer only print a hint,
// because several helpers share one checkout and an automatic rewrite would let one of them
// commit another's unfinished improvement.
//
//   node scripts/check-ui.mjs            check against the baseline
//   node scripts/check-ui.mjs --list     every finding with file and line
//   node scripts/check-ui.mjs --update   lower the baseline to today's counts (never raises it)
//   node scripts/check-ui.mjs --root <dir> --baseline <file>   for the script's own test

import { readFileSync, writeFileSync, readdirSync, statSync, existsSync } from "node:fs";
import { join, relative, sep } from "node:path";
import { fileURLToPath } from "node:url";

const args = process.argv.slice(2);
const flag = (name) => args.includes(name);
const option = (name, fallback) => {
  const i = args.indexOf(name);
  return i >= 0 ? args[i + 1] : fallback;
};

const repo = join(fileURLToPath(import.meta.url), "..", "..");
const root = option("--root", join(repo, "src", "lib"));
const baselinePath = option("--baseline", join(repo, "scripts", "ui-baseline.json"));

/** Font sizes of the type scale in theme.css (--fs-*). Literals outside it count as free sizes. */
const SCALE = [11, 12, 13, 14, 16, 18, 22, 26, 34, 44];

const RULES = {
  "color-literal": "Feste Farbe (Hex/rgb/hsl) außerhalb von theme.css und src/lib/ui/ – nimm ein Token aus theme.css.",
  "font-size": `Schriftgröße außerhalb der Skala (${SCALE.join(", ")} px) – nimm var(--fs-…).`,
  "button-style": "Eigener Knopf-Stil außerhalb von src/lib/ui/ – nimm Button, IconButton oder SegmentedControl.",
  "global-class-override": "Lokale Regel auf einer globalen Klasse aus theme.css – eigenen Namen wählen oder Baustein nehmen.",
  "kbd-style": "Eigener Stil für <kbd> außerhalb von src/lib/ui/ – nimm Kbd oder Button mit kbd; Lage (margin, grid, position) ist erlaubt.",
  motion: "transition/animation mit linear, ease oder freier Dauer – nimm var(--dur…) und var(--ease…).",
  "page-heading": "Eigene Seitenüberschrift (<h1> oder Intro-Block .page-head) – Titel, Zeile und Bereiche gehören in den PageHeader (usePageHead).",
};

const posix = (p) => p.split(sep).join("/");

function walk(dir, out = []) {
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) walk(path, out);
    else if (/\.(svelte|ts|css)$/.test(name)) out.push(path);
  }
  return out;
}

const lineOf = (text, index) => text.slice(0, index).split("\n").length;

/** Replaces comments with spaces so indices (and line numbers) stay put. */
function blankComments(text, html) {
  const blank = (m) => m.replace(/[^\n]/g, " ");
  let t = text.replace(/\/\*[\s\S]*?\*\//g, blank);
  if (html) t = t.replace(/<!--[\s\S]*?-->/g, blank);
  return t;
}

/** Style sources of a file with their offset: whole .css files, <style> blocks of components. */
function styleBlocks(file, text) {
  if (file.endsWith(".css")) return [{ css: text, offset: 0 }];
  if (!file.endsWith(".svelte")) return [];
  const blocks = [];
  for (const m of text.matchAll(/<style[^>]*>([\s\S]*?)<\/style>/g)) {
    blocks.push({ css: m[1], offset: m.index + m[0].indexOf(">") + 1 });
  }
  return blocks;
}

/** Innermost CSS rules ({ selector, body, index }); at-rule wrappers such as @media are descended into. */
function cssRules(css) {
  const rules = [];
  const stack = [];
  let start = 0;
  for (let i = 0; i < css.length; i++) {
    const c = css[i];
    if (c === "{") {
      stack.push({ prelude: css.slice(start, i).trim(), open: i, preludeAt: start, nested: false });
      if (stack.length > 1) stack[stack.length - 2].nested = true;
      start = i + 1;
    } else if (c === "}") {
      const block = stack.pop();
      if (block && !block.nested && !block.prelude.startsWith("@")) {
        const lead = css.slice(block.preludeAt).search(/\S/);
        rules.push({ selector: block.prelude, body: css.slice(block.open + 1, i), index: block.preludeAt + Math.max(lead, 0) });
      }
      start = i + 1;
    } else if (c === ";" && stack.length === 0) {
      start = i + 1;
    }
  }
  return rules;
}

/**
 * Class names theme.css styles on their own (`.btn`, `.chip`, `.panel-head`). Modifiers that only
 * work next to such a class (`.btn.primary`, `.seg button.on`, `.tip .row`) are not counted.
 */
function globalClasses(themeText) {
  const names = new Set();
  for (const rule of cssRules(blankComments(themeText, false))) {
    for (const selector of rule.selector.split(",")) {
      const first = selector.trim().split(/[\s>+~]+/)[0];
      const classes = [...first.matchAll(/\.(-?[_a-zA-Z][\w-]*)/g)].map((m) => m[1]);
      if (classes.length === 1) names.add(classes[0]);
    }
  }
  return names;
}

const COLOR = /(?<![&\w$])#(?:[0-9a-fA-F]{8}|[0-9a-fA-F]{6}|[0-9a-fA-F]{3,4})\b|\b(?:rgba?|hsla?)\(/g;

function freeFontSize(value) {
  const v = value.trim();
  if (/^(var\(--fs-|inherit|initial|unset|[\d.]+(em|%)$)/.test(v)) return false;
  const px = [...v.matchAll(/(\d+(?:\.\d+)?)px/g)].map((m) => Number(m[1]));
  const bare = /^\d+(?:\.\d+)?$/.test(v) ? [Number(v)] : [];
  const sizes = [...px, ...bare];
  return sizes.some((s) => !SCALE.includes(s));
}

const VISUAL = /(^|;|\s)(background|border|border-radius|padding|height|box-shadow|font-weight)\s*:/;

/** Layout may be set where a keycap sits; anything else belongs to the global kbd rule. */
const LAYOUT = /^(margin(-[\w-]+)?|position|top|right|bottom|left|inset|transform|grid-[\w-]+|align-self|justify-self|order|z-index|flex(-[\w-]+)?|vertical-align)$/;

function declarations(body) {
  return body
    .split(";")
    .map((d) => d.trim())
    .filter(Boolean)
    .map((d) => {
      const i = d.indexOf(":");
      return i < 0 ? null : { prop: d.slice(0, i).trim().toLowerCase(), value: d.slice(i + 1).trim() };
    })
    .filter((d) => d !== null);
}

function stylesKbd(selector) {
  return selector.split(",").some((s) => {
    const parts = s.trim().split(/[\s>+~]+/);
    return /^kbd\b/.test(parts[parts.length - 1]);
  });
}

/** A transition or animation off the motion tokens: linear, a keyword ease, a literal time, or no curve. */
function freeMotion(prop, value) {
  if (!/^(transition|animation)(-duration|-timing-function)?$/.test(prop)) return false;
  if (/^(none|inherit|initial|unset)$/.test(value)) return false;
  // A keyword ease stands alone; inside a token name ("--ease-standard") it is the token.
  if (/\blinear\b|(?<![-\w])ease(-in|-out|-in-out)?\b/.test(value)) return true;
  if (/(^|[\s,(])[\d.]+m?s\b/.test(value)) return true;
  return (prop === "transition" || prop === "animation") && !/var\(--ease/.test(value);
}

/** Opening tags of an element, read up to the `>` outside quotes and `{…}` (handlers contain `=>`). */
function openTags(markup, tag) {
  const tags = [];
  const re = new RegExp(`<${tag}\\b`, "g");
  for (const m of markup.matchAll(re)) {
    let depth = 0;
    let quote = "";
    let i = m.index + m[0].length;
    for (; i < markup.length; i++) {
      const c = markup[i];
      if (quote) {
        if (c === quote) quote = "";
      } else if (c === '"' || c === "'") quote = c;
      else if (c === "{") depth++;
      else if (c === "}") depth--;
      else if (c === ">" && depth === 0) break;
    }
    tags.push(markup.slice(m.index, i));
  }
  return tags;
}

/** Class names that sit on <button> elements of a component. */
function buttonClasses(markup) {
  const names = new Set();
  for (const tag of openTags(markup, "button")) {
    for (const m of tag.matchAll(/\bclass="([^"]*)"/g)) {
      for (const n of m[1].replace(/\{[^}]*\}/g, " ").split(/\s+/)) if (n) names.add(n);
    }
    for (const m of tag.matchAll(/\bclass:([\w-]+)/g)) names.add(m[1]);
  }
  return names;
}

/** Whether a selector styles a button: its last compound is `button`, `.btn…`, or a class used on <button>. */
function stylesButton(selector, ownButtonClasses) {
  return selector.split(",").some((s) => {
    const parts = s.trim().split(/[\s>+~]+/);
    const last = parts[parts.length - 1].replace(/::?[\w-]+(\([^)]*\))?/g, "");
    if (/^button\b/.test(last)) return true;
    const classes = [...last.matchAll(/\.(-?[_a-zA-Z][\w-]*)/g)].map((m) => m[1]);
    return classes.some((c) => /(^|-)btn(-|$)/.test(c) || ownButtonClasses.has(c));
  });
}

function scan(file, text, globals) {
  const rel = posix(relative(root, file));
  const inKit = rel.startsWith("ui/");
  const isTheme = rel === "theme.css";
  const findings = [];
  const add = (rule, index, detail) => findings.push({ rule, file: rel, line: lineOf(text, index), detail });
  const clean = blankComments(text, file.endsWith(".svelte"));

  if (!inKit && !isTheme) {
    for (const m of clean.matchAll(COLOR)) add("color-literal", m.index, m[0]);
  }

  if (!isTheme) {
    for (const m of clean.matchAll(/font-size\s*[:=]\s*["']?([^;}"'\n]+)/g)) {
      if (/^\{/.test(m[1].trim())) continue;
      if (freeFontSize(m[1])) add("font-size", m.index, m[1].trim());
    }
  }

  if (file.endsWith(".svelte") && rel !== "ui/PageHeader.svelte") {
    const markup = clean.replace(/<style[^>]*>[\s\S]*?<\/style>/g, (m) => m.replace(/[^\n]/g, " "));
    for (const m of markup.matchAll(/<h1\b/g)) add("page-heading", m.index, "<h1>");
    for (const m of markup.matchAll(/class="[^"]*\bpage-head\b[^"]*"/g)) add("page-heading", m.index, m[0]);
  }

  const ownButtons = file.endsWith(".svelte") ? buttonClasses(clean.replace(/<style[^>]*>[\s\S]*?<\/style>/g, "")) : new Set();
  for (const { css, offset } of styleBlocks(file, clean)) {
    if (isTheme) continue;
    for (const rule of cssRules(css)) {
      const at = offset + rule.index;
      if (!inKit && stylesButton(rule.selector, ownButtons) && VISUAL.test(rule.body)) {
        add("button-style", at, rule.selector.replace(/\s+/g, " "));
      }
      if (!inKit && stylesKbd(rule.selector) && declarations(rule.body).some((d) => !LAYOUT.test(d.prop))) {
        add("kbd-style", at, rule.selector.replace(/\s+/g, " "));
      }
      if (!inKit) {
        for (const d of declarations(rule.body)) if (freeMotion(d.prop, d.value)) add("motion", at, `${d.prop}: ${d.value.replace(/\s+/g, " ")}`);
      }
      const hits = [...rule.selector.matchAll(/\.(-?[_a-zA-Z][\w-]*)/g)].map((m) => m[1]).filter((n) => globals.has(n));
      for (const name of new Set(hits)) add("global-class-override", at, `.${name} in „${rule.selector.replace(/\s+/g, " ")}“`);
    }
  }
  return findings;
}

const themePath = join(root, "theme.css");
const globals = existsSync(themePath) ? globalClasses(readFileSync(themePath, "utf8")) : new Set();
const findings = walk(root).flatMap((f) => scan(f, readFileSync(f, "utf8"), globals));

const counts = Object.fromEntries(Object.keys(RULES).map((r) => [r, 0]));
const perFile = Object.fromEntries(Object.keys(RULES).map((r) => [r, {}]));
for (const f of findings) {
  counts[f.rule]++;
  perFile[f.rule][f.file] = (perFile[f.rule][f.file] ?? 0) + 1;
}

const baseline = existsSync(baselinePath) ? JSON.parse(readFileSync(baselinePath, "utf8")) : null;

function writeBaseline(rules) {
  const sorted = (o) => Object.fromEntries(Object.entries(o).sort(([a], [b]) => a.localeCompare(b)));
  const data = { rules: {} };
  for (const r of Object.keys(RULES)) data.rules[r] = { total: rules[r].total, files: sorted(rules[r].files) };
  writeFileSync(baselinePath, JSON.stringify(data, null, 2) + "\n");
}

if (flag("--list")) {
  for (const f of findings) console.log(`${f.file}:${f.line}  [${f.rule}]  ${f.detail}`);
}

if (!baseline || flag("--init")) {
  if (!flag("--init")) {
    console.error(`check-ui: keine Baseline unter ${baselinePath}. Mit --init anlegen.`);
    process.exit(1);
  }
  writeBaseline(Object.fromEntries(Object.keys(RULES).map((r) => [r, { total: counts[r], files: perFile[r] }])));
  console.log(`check-ui: Baseline angelegt: ${JSON.stringify(counts)}`);
  process.exit(0);
}

let failed = false;
const lowered = [];
for (const rule of Object.keys(RULES)) {
  const base = baseline.rules[rule] ?? { total: 0, files: {} };
  const now = counts[rule];
  if (now > base.total) {
    failed = true;
    console.error(`\n✗ ${rule}: ${now} statt höchstens ${base.total}. ${RULES[rule]}`);
    for (const [file, n] of Object.entries(perFile[rule])) {
      const before = base.files[file] ?? 0;
      if (n <= before) continue;
      console.error(`  ${file}: ${before} → ${n}`);
      for (const f of findings.filter((x) => x.rule === rule && x.file === file)) {
        console.error(`    ${file}:${f.line}  ${f.detail}`);
      }
    }
  } else if (now < base.total) {
    lowered.push(`${rule} ${base.total} → ${now}`);
  }
}

if (flag("--update")) {
  const next = {};
  for (const rule of Object.keys(RULES)) {
    const base = baseline.rules[rule] ?? { total: 0, files: {} };
    next[rule] = now(rule, base);
  }
  writeBaseline(next);
  console.log(lowered.length ? `check-ui: Baseline gesenkt: ${lowered.join(", ")}` : "check-ui: Baseline unverändert.");
  process.exit(failed ? 1 : 0);
}

function now(rule, base) {
  if (counts[rule] > base.total) return base;
  return { total: counts[rule], files: perFile[rule] };
}

if (failed) {
  console.error("\ncheck-ui: neue Verstöße gegen das Designsystem (Liste mit --list). Bausteine liegen in src/lib/ui/, Katalog unter ?view=kit.");
  process.exit(1);
}
const summary = Object.keys(RULES).map((r) => `${r} ${counts[r]}/${baseline.rules[r]?.total ?? 0}`).join(", ");
console.log(`check-ui: ok (${summary})`);
if (lowered.length) console.log(`check-ui: weniger Verstöße (${lowered.join(", ")}) – Baseline senken mit \`pnpm ui:ratchet\`.`);
