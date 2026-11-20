// Layout check of the app frame against the browser demo: every page at every window width the
// app supports, the narrow rail, a window full of notes at once and a deliberately broken case
// that must be caught. Checked, per view:
//   - nothing scrolls sideways (document and content area)
//   - the page frame is no wider than --page-max and stands centred beside the side bar
//   - page header and page content start at the same left edge
//   - no entry of the side bar overlaps another or leaves the bar
//   - no name, mark or note in it wraps or is cut off with "…"
//   - the page header keeps title and filters on one row
//
// Runs as a step of e2e/run.mjs (demo only) or alone:
//   E2E_URL=http://localhost:5173/ E2E_WEBDRIVER=http://localhost:9515 node e2e/layout.mjs
// Needs chromedriver: the window is sized over the DevTools protocol, exactly and without chrome.

import { Session } from "./webdriver.mjs";

export const WIDTHS = [980, 1280, 1440, 1920, 2560, 3840];
export const PAGES = [
  "dashboard",
  "skill",
  "overview",
  "tournaments",
  "hands",
  "leaks",
  "reports",
  "ranges",
  "training",
  "play",
  "coach",
  "players",
  "live",
  "hud",
  "charts",
  "solver",
  "library",
  "settings",
];

/** In the page: what breaks the frame rules, as readable lines (empty when all is well). */
const CHECK = `
  const out = [];
  const px = (v) => parseFloat(v) || 0;
  const rect = (el) => el.getBoundingClientRect();
  const W = window.innerWidth;
  const root = getComputedStyle(document.documentElement);
  const max = px(root.getPropertyValue("--page-max"));
  if (!max) out.push("Token --page-max fehlt");

  const doc = document.documentElement;
  if (doc.scrollWidth > W + 1) out.push("Dokument scrollt waagrecht: " + doc.scrollWidth + " > " + W);
  const content = document.querySelector("main .content");
  if (content && content.scrollWidth > content.clientWidth + 1) out.push("Inhalt scrollt waagrecht: " + content.scrollWidth + " > " + content.clientWidth);
  for (const el of document.querySelectorAll("main .content > .frame, main .content .page")) {
    const w = rect(el).width;
    if (w > max + 0.5) out.push("breiter als --page-max: " + (el.className || el.tagName) + " " + Math.round(w) + " px");
  }

  const side = document.querySelector("aside.side");
  if (!side) return ["Seitenleiste fehlt"];
  const h1 = document.querySelector("main h1");
  const page = document.querySelector("main .content .page");
  if (page) {
    const left = rect(page).left + px(getComputedStyle(page).paddingLeft);
    if (h1 && Math.abs(rect(h1).left - left) > 1) out.push("Seitenkopf beginnt bei " + Math.round(rect(h1).left) + ", Seite bei " + Math.round(left));
    // The header follows the page; a page with its own margin moves the title when you switch pages.
    const gutter = px(root.getPropertyValue("--page-gutter"));
    const own = px(getComputedStyle(page).paddingLeft);
    if (Math.abs(own - gutter) > 0.5) out.push("Seite hat eigenen Rand " + own + " px statt --page-gutter " + gutter + " px");
  }
  const frame = document.querySelector("main .content > .frame");
  if (content && frame) {
    // Centred in the room beside the side bar; the scrollbar may take a few pixels on the right.
    const c = rect(content), f = rect(frame);
    const gap = f.left - c.left - (c.left + content.clientWidth - f.right);
    if (Math.abs(gap) > 1) out.push("Seite steht nicht mittig: links " + Math.round(f.left - c.left) + " px, rechts " + Math.round(c.left + content.clientWidth - f.right) + " px");
  }

  const visible = (el) => { const r = rect(el); return r.width > 0 && r.height > 0 && getComputedStyle(el).visibility !== "hidden"; };
  const overlaps = (name, items) => {
    for (let i = 0; i < items.length; i++)
      for (let j = i + 1; j < items.length; j++) {
        const a = rect(items[i]), b = rect(items[j]);
        if (a.right > b.left + 0.5 && b.right > a.left + 0.5 && a.bottom > b.top + 0.5 && b.bottom > a.top + 0.5)
          out.push(name + ": „" + items[i].textContent.trim().slice(0, 30) + "“ überlappt „" + items[j].textContent.trim().slice(0, 30) + "“");
      }
  };
  // Text that wraps has more than one line box; text cut with "…" is wider than its box.
  const unbroken = (name, scope) => {
    const walk = document.createTreeWalker(scope, NodeFilter.SHOW_TEXT);
    for (let n = walk.nextNode(); n; n = walk.nextNode()) {
      if (!n.textContent.trim() || !n.parentElement || !visible(n.parentElement)) continue;
      const range = document.createRange();
      range.selectNodeContents(n);
      const tops = new Set([...range.getClientRects()].map((r) => Math.round(r.top)));
      if (tops.size > 1) out.push(name + ": „" + n.textContent.trim().slice(0, 30) + "“ bricht um");
    }
    // Cut off: "…", or one line in a box that hides what does not fit.
    for (const el of scope.querySelectorAll("*")) {
      if (!visible(el)) continue;
      const cs = getComputedStyle(el);
      const clips = cs.textOverflow === "ellipsis" || (cs.overflowX === "hidden" && cs.whiteSpace === "nowrap");
      if (clips && el.scrollWidth > el.clientWidth + 1) out.push(name + ": „" + el.textContent.trim().slice(0, 30) + "“ ist abgeschnitten");
    }
  };

  const s = rect(side);
  // Entries scrolled out of the list are reached by scrolling; the ones in view count.
  const list = rect(side.querySelector("nav"));
  const inView = (el) => { const r = rect(el); return r.top >= list.top - 0.5 && r.bottom <= list.bottom + 0.5; };
  const items = [
    side.querySelector(".brand"),
    side.querySelector("[data-skill-chip]"),
    ...[...side.querySelectorAll("nav [data-nav]")].filter(inView),
    ...side.querySelectorAll(".foot > :not(.tools), .foot .tools > *"),
  ].filter((el) => el && visible(el));
  overlaps("Seitenleiste", items);
  for (const el of items) {
    const r = rect(el);
    if (r.left < s.left - 0.5 || r.right > s.right + 0.5) out.push("Seitenleiste: „" + el.textContent.trim().slice(0, 30) + "“ ragt heraus");
    // A mark may not leave its entry, nor squeeze the name out of it.
    for (const part of el.querySelectorAll(":scope > *")) if (rect(part).right > r.right + 0.5) out.push("Seitenleiste: „" + part.textContent.trim().slice(0, 30) + "“ ragt aus „" + el.textContent.trim().slice(0, 20) + "“");
    const name = el.querySelector(".label");
    if (name && rect(name).width < Math.min(name.scrollWidth, 40)) out.push("Seitenleiste: Name „" + name.textContent.trim() + "“ ist verdrängt");
  }
  unbroken("Seitenleiste", side);
  for (const note of document.querySelectorAll("main .notice .line, main .strip")) unbroken("Hinweis", note);
  for (const note of document.querySelectorAll("main .notice, main .strip")) overlaps("Hinweis", [...note.children].filter(visible));

  const head = document.querySelector("main .ui-page-header .bar");
  if (head) {
    const title = head.querySelector(".title"), actions = head.querySelector(".actions");
    if (title && actions && visible(actions) && rect(actions).top >= rect(title).bottom - 2) out.push("Seitenkopf bricht um: Filter unter dem Titel");
  }
  return out;`;

/** Waits for the page to settle: the title and the side bar are there, fonts are loaded. */
const SETTLED = `
  const done = arguments[arguments.length - 1];
  const until = Date.now() + 15000;
  const poll = () => {
    if ((document.querySelector("main h1") && document.querySelector("aside.side")) || Date.now() > until)
      document.fonts.ready.then(() => setTimeout(() => requestAnimationFrame(() => requestAnimationFrame(() => done(true))), 250));
    else setTimeout(poll, 100);
  };
  poll();`;

export async function size(s, width) {
  const height = width === 980 ? 700 : Math.round(width * 0.5625);
  await s.cmd("POST", "/goog/cdp/execute", {
    cmd: "Emulation.setDeviceMetricsOverride",
    params: { width, height, deviceScaleFactor: 1, mobile: false },
  });
}

/** One view: its problems, prefixed with where they happened. */
export async function view(s, base, query, width) {
  let problems = [];
  // The dev server may reload the page once while it optimizes; a page that is not there yet is
  // loaded again before anything counts.
  // A reload in the middle ends the wait with a script timeout; that attempt counts as not there.
  for (let attempt = 0; attempt < 2; attempt++) {
    await s.navigate(new URL(query, base).href);
    try {
      await s.execAsync(SETTLED);
      problems = await s.exec(CHECK);
    } catch (e) {
      if (attempt === 1 || !/script timeout|navigat|context/i.test(e.message)) throw e;
      problems = ["Seitenleiste fehlt"];
    }
    if (!problems.includes("Seitenleiste fehlt")) break;
  }
  return problems.map((p) => `${width} px ${query}: ${p}`);
}

/**
 * Runs every view at every width. Returns the problems found; throws when the broken case below
 * goes unnoticed, since then the check itself is blind.
 */
export async function checkLayout(s, base, log = () => {}) {
  const problems = [];
  for (const width of WIDTHS) {
    await size(s, width);
    for (const page of PAGES) problems.push(...(await view(s, base, `?tab=${page}`, width)));
    // The rail at every width, and many notes at once: a poker client runs, no hand history yet,
    // a long mark.
    problems.push(...(await view(s, base, `?tab=leaks&rail=1`, width)));
    problems.push(...(await view(s, base, `?tab=leaks&locked=1&nohistory=1&navmark=${encodeURIComponent("Analyse läuft")}`, width)));
    log(`${width} px geprüft`);
  }
  // Counter-check: an absurdly long mark beside a name must be caught.
  await size(s, 1440);
  const broken = await view(s, base, `?tab=leaks&navmark=${encodeURIComponent("4 Schwachstellen ".repeat(4).trim())}`, 1440);
  if (!broken.length) throw new Error("Gegenprobe: eine zu lange Marke wurde nicht erkannt");
  log(`Gegenprobe erkannt: ${broken[0]}`);
  await s.cmd("POST", "/goog/cdp/execute", { cmd: "Emulation.clearDeviceMetricsOverride", params: {} });
  return problems;
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const base = process.env.E2E_URL;
  const server = process.env.E2E_WEBDRIVER;
  if (!base || !server) {
    console.error("E2E_URL und E2E_WEBDRIVER setzen (Browser-Demo und chromedriver).");
    process.exit(2);
  }
  const s = await Session.start(server, { browserName: "chrome", "goog:chromeOptions": { args: ["--headless=new"] } });
  try {
    const problems = await checkLayout(s, base, (l) => console.log(l));
    for (const p of problems) console.log(`FEHLER ${p}`);
    console.log(problems.length ? `${problems.length} Fehler` : "Layout: alles in Ordnung");
    process.exitCode = problems.length ? 1 : 0;
  } finally {
    await s.quit();
  }
}
