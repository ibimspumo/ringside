// Layout check of the app frame against the browser demo: every page at every window width the
// app supports, plus the open menus, a window full of notes at once and a deliberately broken
// case that must be caught. Checked, per view:
//   - nothing scrolls sideways (document and content area)
//   - the page frame is no wider than --page-max
//   - top bar, page header and page content start at the same left edge
//   - nothing in the top bar or the row below overlaps or leaves the bar
//   - no tab, mark or note in them wraps or is cut off with "…"
//   - the page header keeps title and filters on one row
//
// Runs as a step of e2e/run.mjs (demo only) or alone:
//   E2E_URL=http://localhost:5173/ E2E_WEBDRIVER=http://localhost:9515 node e2e/layout.mjs
// Needs chromedriver: the window is sized over the DevTools protocol, exactly and without chrome.

import { Session } from "./webdriver.mjs";

export const WIDTHS = [980, 1280, 1440, 1920, 2560, 3840];
export const PAGES = [
  "dashboard",
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
const AREAS = ["spiel", "ueben", "gegner", "werkzeuge"];

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

  const row = document.querySelector("[role=banner] .bar .row");
  if (!row) return ["obere Leiste fehlt"];
  const rs = getComputedStyle(row);
  const barLeft = rect(row).left + px(rs.paddingLeft);
  const barRight = rect(row).right - px(rs.paddingRight);
  const h1 = document.querySelector("main h1");
  if (h1 && Math.abs(rect(h1).left - barLeft) > 1) out.push("Seitenkopf beginnt bei " + Math.round(rect(h1).left) + ", Leiste bei " + Math.round(barLeft));
  const page = document.querySelector("main .content .page");
  if (page) {
    const left = rect(page).left + px(getComputedStyle(page).paddingLeft);
    if (Math.abs(left - barLeft) > 1) out.push("Seite beginnt bei " + Math.round(left) + ", Leiste bei " + Math.round(barLeft));
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
  const inside = (name, items, left, right) => {
    for (const el of items) {
      const r = rect(el);
      if (r.left < left - 12.5 || r.right > right + 12.5) out.push(name + ": „" + el.textContent.trim().slice(0, 30) + "“ ragt aus der Leiste (" + Math.round(r.left) + "–" + Math.round(r.right) + ")");
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
    for (const el of scope.querySelectorAll("*")) {
      if (!visible(el) || getComputedStyle(el).textOverflow !== "ellipsis") continue;
      if (el.scrollWidth > el.clientWidth + 1) out.push(name + ": „" + el.textContent.trim().slice(0, 30) + "“ ist abgeschnitten");
    }
  };

  const barItems = [row.querySelector(".brand"), ...row.querySelectorAll("nav > button"), ...row.querySelectorAll(".right > .item")].filter((el) => el && visible(el));
  overlaps("Leiste", barItems);
  inside("Leiste", barItems, barLeft, barRight);
  unbroken("Leiste", row);

  const sub = document.querySelector("[role=banner] .sub .row");
  if (sub) {
    const items = [...sub.querySelectorAll(":scope > button")].filter(visible);
    overlaps("Unterzeile", items);
    inside("Unterzeile", items, barLeft, barRight);
    unbroken("Unterzeile", sub);
  }
  for (const note of document.querySelectorAll("main .notice .line, [role=banner] ~ .strip")) unbroken("Hinweis", note);

  const menu = document.querySelector("#nav-menu");
  if (menu) {
    const m = rect(menu);
    if (m.right > W + 0.5 || m.left < -0.5) out.push("Menü ragt aus dem Fenster");
    const tiles = [...menu.querySelectorAll(".tiles > button")].filter(visible);
    overlaps("Menü", tiles);
    for (const t of tiles) if (t.scrollWidth > t.clientWidth + 1) out.push("Menü: „" + t.textContent.trim().slice(0, 30) + "“ läuft über");
  }

  const head = document.querySelector("main .ui-page-header .bar");
  if (head) {
    const title = head.querySelector(".title"), actions = head.querySelector(".actions");
    if (title && actions && visible(actions) && rect(actions).top >= rect(title).bottom - 2) out.push("Seitenkopf bricht um: Filter unter dem Titel");
  }
  return out;`;

/** Waits for the page to settle: the title is there, fonts are loaded, the bar has folded. */
const SETTLED = `
  const done = arguments[arguments.length - 1];
  const until = Date.now() + 15000;
  const poll = () => {
    if (document.querySelector("main h1") || Date.now() > until)
      document.fonts.ready.then(() => setTimeout(() => requestAnimationFrame(() => requestAnimationFrame(() => done(true))), 250));
    else setTimeout(poll, 100);
  };
  poll();`;

async function size(s, width) {
  const height = width === 980 ? 700 : Math.round(width * 0.5625);
  await s.cmd("POST", "/goog/cdp/execute", {
    cmd: "Emulation.setDeviceMetricsOverride",
    params: { width, height, deviceScaleFactor: 1, mobile: false },
  });
}

/** One view: its problems, prefixed with where they happened. */
async function view(s, base, query, width) {
  await s.navigate(new URL(query, base).href);
  await s.execAsync(SETTLED);
  const problems = await s.exec(CHECK);
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
    for (const area of AREAS) problems.push(...(await view(s, base, `?tab=leaks&navopen=${area}`, width)));
    // Many notes at once: PokerStars runs, no hand history yet, a very long mark.
    const long = encodeURIComponent("4 Schwachstellen und noch eine sehr lange Marke");
    problems.push(...(await view(s, base, `?tab=leaks&locked=1&nohistory=1&navmark=${long}`, width)));
    problems.push(...(await view(s, base, `?tab=training&locked=1&nohistory=1&navopen=werkzeuge`, width)));
    log(`${width} px geprüft`);
  }
  // Counter-check: folding off and an absurdly long mark at the narrowest width must be caught.
  await size(s, WIDTHS[0]);
  const broken = await view(s, base, `?tab=leaks&navfit=0&locked=1&navmark=${encodeURIComponent("4 Schwachstellen ".repeat(6).trim())}`, WIDTHS[0]);
  if (!broken.length) throw new Error("Gegenprobe: eine zu lange Marke ohne Einklappen wurde nicht erkannt");
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
