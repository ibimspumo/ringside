// End-to-end run of the real app: starts the debug build on its own data folder and the
// anonymized fixture hand histories, then clicks through every area and checks what the
// page says. Needs Windows: msedgedriver attaches to the app's WebView2.
//
// msedgedriver attaches through a fixed DevTools port instead of launching the app itself
// (tauri-driver's way): from WebView2 runtime 133 on, the WEBVIEW2_* variables it launches
// with are ignored when the app sets its own browser arguments, as Tauri always does
// (MicrosoftEdge/WebView2Feedback#5415). So the test build opens the port by config:
//
//   node e2e/run.mjs --tauri-config > e2e.conf.json
//   pnpm tauri build --debug --no-bundle --config e2e.conf.json
//
//   E2E_APP          the built app (default: target/debug/ringside.exe)
//   MSEDGEDRIVER     msedgedriver.exe matching the installed WebView2 (default: from PATH)
//   E2E_NET_TRAP=1   count connections to the hosts printed by `node e2e/run.mjs --hosts`,
//                    which CI points at local addresses in the hosts file
//   E2E_OUT          screenshots, page texts and logs (default: target/e2e)
//
// For writing steps, E2E_URL runs the same steps against the browser demo through any
// WebDriver server at E2E_WEBDRIVER (e.g. chromedriver); checks that need the backend
// are skipped there.

import { spawn } from "node:child_process";
import { createWriteStream, cpSync, mkdirSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import net from "node:net";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { Session } from "./webdriver.mjs";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const OUT = path.resolve(process.env.E2E_OUT ?? path.join(ROOT, "target", "e2e"));
const DEMO_URL = process.env.E2E_URL;
const TAURI = !DEMO_URL;
const PORT = 4444;
const DEVTOOLS = "127.0.0.1:9222";

/** Hosts the app may talk to. In the test nothing may reach them. */
const TRAPS = [
  { name: "Ringside Cloud", host: "ringside-api.bunny.run", ip: "127.0.0.2" },
  { name: "OpenRouter", host: "openrouter.ai", ip: "127.0.0.3" },
];

if (process.argv.includes("--hosts")) {
  for (const t of TRAPS) console.log(`${t.ip} ${t.host}`);
  process.exit(0);
}

if (process.argv.includes("--tauri-config")) {
  const { readFileSync } = await import("node:fs");
  const conf = JSON.parse(readFileSync(path.join(ROOT, "src-tauri", "tauri.conf.json"), "utf8"));
  // Giving arguments replaces wry's defaults, so they are repeated here.
  const windows = conf.app.windows.map((w) => ({
    ...w,
    additionalBrowserArgs: `--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --remote-debugging-port=${DEVTOOLS.split(":")[1]}`,
  }));
  console.log(JSON.stringify({ app: { windows } }));
  process.exit(0);
}

/** Error texts of failed commands as the pages show them (Tauri's and serde's wording). */
const ERROR_TEXT =
  /invalid args|missing (required )?(key|field)|unknown (variant|field)|invalid type|Command \S+ not found|not allowed|is not a function|Cannot read properties|undefined is not/i;

rmSync(OUT, { recursive: true, force: true });
mkdirSync(path.join(OUT, "pages"), { recursive: true });

// ---------------------------------------------------------------------------------
// Test data: an empty data folder and the fixtures laid out like the client's folders.

const fixtures = path.join(ROOT, "crates", "ringside-core", "tests", "fixtures");
const stars = path.join(OUT, "PokerStars");
const historyDir = path.join(stars, "HandHistory");
const copyTxt = (from, to) => {
  mkdirSync(to, { recursive: true });
  for (const f of readdirSync(from).filter((f) => f.endsWith(".txt"))) cpSync(path.join(from, f), path.join(to, f));
};
copyTxt(fixtures, path.join(historyDir, "Hero"));
copyTxt(path.join(fixtures, "summaries"), path.join(stars, "TournSummary", "Hero"));
const dataDir = path.join(OUT, "data");
mkdirSync(dataDir, { recursive: true });

// ---------------------------------------------------------------------------------
// Network trap: every connection to a trapped host lands here and is counted.

const hits = Object.fromEntries(TRAPS.map((t) => [t.name, 0]));
const servers = [];
if (process.env.E2E_NET_TRAP) {
  // Both services are HTTPS only.
  for (const t of TRAPS) {
    const server = net.createServer((socket) => {
      hits[t.name]++;
      log(`network: connection to ${t.host}`);
      socket.destroy();
    });
    await new Promise((resolve, reject) => server.once("error", reject).listen(443, t.ip, resolve));
    servers.push(server);
  }
}

// ---------------------------------------------------------------------------------
// Driver and session.

const consoleFile = createWriteStream(path.join(OUT, "console.log"));
function log(line) {
  consoleFile.write(line + "\n");
}

/** Runs `exe` with its output in OUT/`name`.log; a failed start ends the run. */
function launch(name, exe, args, env = process.env) {
  const out = createWriteStream(path.join(OUT, `${name}.log`));
  const child = spawn(exe, args, { env, stdio: ["ignore", "pipe", "pipe"] });
  child.stdout.pipe(out);
  child.stderr.pipe(out);
  child.on("error", (e) => {
    console.error(`${name}: ${e.message}`);
    process.exit(1);
  });
  return child;
}

const children = [];
const server = process.env.E2E_WEBDRIVER ?? `http://127.0.0.1:${PORT}`;
let capabilities;
if (TAURI) {
  const app = path.resolve(process.env.E2E_APP ?? path.join(ROOT, "target", "debug", "ringside.exe"));
  children.push(launch("app", app, [], { ...process.env, RINGSIDE_DATA_DIR: dataDir }));
  children.push(launch("msedgedriver", process.env.MSEDGEDRIVER ?? "msedgedriver", [`--port=${PORT}`]));
  capabilities = {
    browserName: "webview2",
    "ms:edgeOptions": { debuggerAddress: DEVTOOLS },
    "ms:loggingPrefs": { browser: "ALL" },
  };
} else {
  capabilities = {
    browserName: "chrome",
    "goog:chromeOptions": { args: ["--headless=new", "--window-size=1280,820"] },
    "goog:loggingPrefs": { browser: "ALL" },
  };
}

const s = await startSession();
await s.cmd("POST", "/timeouts", { script: 180_000 });
if (!TAURI) await s.navigate(DEMO_URL);

async function startSession() {
  const until = Date.now() + 60_000;
  for (;;) {
    try {
      return await Session.start(server, capabilities);
    } catch (e) {
      if (Date.now() > until) throw e;
      await sleep(500);
    }
  }
}

// ---------------------------------------------------------------------------------
// Helpers. Elements are found in the page by visible text, like a user would.

function sleep(ms) {
  return new Promise((r) => setTimeout(r, ms));
}

const FIND = `
  const [sel, text, exact] = arguments;
  const norm = (t) => t.replace(/\\s+/g, " ").trim();
  return [...document.querySelectorAll(sel)].find((e) => {
    if (!e.getClientRects().length) return false;
    if (text === null) return true;
    const t = norm(e.textContent ?? "");
    return exact ? t === text : t.includes(text);
  }) ?? null;`;

/** Polls `script` until it returns something truthy. */
async function waitFor(what, script, args = [], timeout = 20_000) {
  const until = Date.now() + timeout;
  let last;
  for (;;) {
    last = await s.exec(script, ...args);
    if (last) return last;
    if (Date.now() > until) throw new Error(`timed out waiting for ${what}`);
    await sleep(250);
  }
}

const find = (sel, text = null, { exact = true, timeout } = {}) =>
  waitFor(`${sel}${text === null ? "" : ` „${text}“`}`, FIND, [sel, text, exact], timeout);

async function click(sel, text = null, opts) {
  const el = await find(sel, text, opts);
  // Like a user scrolling to it: sticky bars may cover elements at the window edge.
  await s.exec(`arguments[0].scrollIntoView({ block: "center" })`, el);
  // The scroll event comes with the next frame, before its animation callbacks. Arriving after
  // the click, it would hide the popover the click opened (ui/floating.ts).
  await s.execAsync(`const done = arguments[0]; requestAnimationFrame(() => done());`);
  await s.click(el);
}

/** Antes sum up in one line of the replayer's log, never one row per player. Returns that line. */
async function anteLine() {
  const r = await s.exec(`
    const log = document.querySelector(".replayer [aria-label='Zugverlauf']");
    const rows = log ? [...log.querySelectorAll(".what")].filter((e) => /\\bAnte\\b/.test(e.textContent)).length : 0;
    return { rows, line: log?.querySelector(".antes")?.textContent.trim() ?? null };`);
  if (r.rows) throw new Error(`${r.rows} ante row(s) in the log instead of one line`);
  log(`ante line: ${r.line ?? "none"}`);
  return r.line;
}

const mainText = () => s.exec(`return document.querySelector("main")?.innerText ?? ""`);

function waitText(pattern, timeout) {
  if (typeof pattern === "string" && pattern.includes("|")) throw new Error(`waitText("${pattern}") is literal text: pass a RegExp for alternatives`);
  const source = pattern instanceof RegExp ? pattern.source : pattern.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return waitFor(`text /${source}/`, `return new RegExp(arguments[0]).test(document.querySelector("main")?.innerText ?? "")`, [source], timeout);
}

async function nav(label) {
  await click("nav button", label);
  await find("main h1", label);
}

/** A command called the way the frontend calls it. */
async function invoke(cmd, args = {}) {
  const r = await s.execAsync(
    `const [cmd, args, done] = arguments;
     window.__TAURI_INTERNALS__.invoke(cmd, args).then((ok) => done({ ok }), (e) => done({ err: String(e) }));`,
    cmd,
    args,
  );
  if ("err" in r) throw new Error(`${cmd}: ${r.err}`);
  return r.ok;
}

const HOOK = `
  window.__e2eErrors ??= [];
  if (!window.__e2eHooked) {
    window.__e2eHooked = true;
    const push = (m) => window.__e2eErrors.push(m);
    const error = console.error;
    console.error = (...a) => { push(a.map(String).join(" ")); error(...a); };
    addEventListener("error", (e) => push("error: " + e.message));
    addEventListener("unhandledrejection", (e) => push("unhandled rejection: " + String(e.reason)));
  }`;

/** Console errors since the last call: from the driver's log and from the page's own hook. */
async function consoleErrors() {
  const errors = [];
  for (const e of (await s.consoleLog()) ?? []) {
    log(`[${e.level}] ${e.message}`);
    if (e.level === "SEVERE") errors.push(e.message);
  }
  const hooked = await s.exec(`${HOOK}; return window.__e2eErrors.splice(0);`);
  for (const m of hooked) log(`[hook] ${m}`);
  // The driver's log already has what the hook saw, unless the driver has no log.
  // A table sized from its own width settles one frame later after a window resize; the
  // browser reports that as this "error", which the ResizeObserver spec defines as benign.
  const benign = (m) => m.includes("ResizeObserver loop completed with undelivered notifications");
  return (errors.length ? errors : hooked).filter((m) => !benign(m));
}

// ---------------------------------------------------------------------------------
// Steps.

const steps = [];
const step = (name, fn) => steps.push({ name, fn });
const tauriOnly = (name, fn) => TAURI && step(name, fn);

step("App startet", async () => {
  await find("nav button", "Übersicht", { timeout: 60_000 });
  await s.exec(HOOK);
  if (TAURI && !(await s.exec(`return "__TAURI_INTERNALS__" in window`))) throw new Error("not running in Tauri");
  // Proves the console check below every step can see errors at all.
  const PROBE = "e2e console probe";
  await s.exec(`console.error(arguments[0])`, PROBE);
  const errors = await consoleErrors();
  if (!errors.some((m) => m.includes(PROBE))) throw new Error("console errors are not captured");
  const real = errors.filter((m) => !m.includes(PROBE));
  if (real.length) throw new Error(`console errors:\n  ${real.join("\n  ")}`);
});

tauriOnly("PokerStars-Sperre ist aus", async () => {
  if ((await invoke("poker_client_running")) !== false) throw new Error("the app thinks PokerStars runs");
});

step("Cloud-Hinweis erscheint, Cloud aus", async () => {
  await waitText("Ringside Cloud ist an");
  if (TAURI) {
    // Before anything could solve push/fold, so no request can reach the cloud.
    await invoke("cloud_save_settings", { cloud: { fetch: false, share: false, notice_seen: true } });
    // A runner without input counts as idle; a background solve would only slow the run.
    await invoke("library_set_mode", { mode: "off" });
  } else {
    await click("main button", "Verstanden");
  }
  await waitFor("the notice to go", `return !document.querySelector("main .notice")`);
});

tauriOnly("Hand-Histories einlesen", async () => {
  const settings = await invoke("get_settings");
  settings.hand_history_dirs = [historyDir];
  await invoke("save_settings", { settings });
  const n = await waitFor(
    "stored hands > 0",
    `const n = Number(document.querySelector(".stored .num")?.textContent.replace(/\\D/g, "") || 0); return n > 0 && n;`,
    [],
    60_000,
  );
  log(`stored hands: ${n}`);
});

step("Übersicht", () => nav("Übersicht"));
step("Statistik", () => nav("Statistik"));
step("Turniere", async () => {
  await nav("Turniere");
  // The id sits in the name's tooltip; narrow windows hide the id column.
  if (TAURI) await find("main .gname[title*='#3500000001']");
});

step("Turnier-Bericht: Klick öffnet ihn, ein Fehler spielt die Hand ab", async () => {
  await click(TAURI ? "main tbody tr:has(.gname[title*='#3500000001'])" : "main tbody tr:has(.gname[title*='Sunday Million'])");
  await waitText("Glück gegen All-in-EV");
  await waitText("Deine Entscheidungen");
  await click("main button", "Frag den Coach");
  await waitText(/eigenen Key von OpenRouter|Kosten prüfen/);
  if (!TAURI) {
    // The demo has a key: the cost comes before anything is sent, the answer after.
    await click("main section[aria-label='Frag den Coach'] button", "Was sagt mein Score?");
    await waitText("Kostet etwa");
    await click("main section[aria-label='Frag den Coach'] button", "Senden");
    await waitText("Antwort von");
  }
  if (TAURI && !(await s.exec(`return !!document.querySelector("main .moment button")`))) {
    // The fixture tournament may have no rated mistake: back to the list is enough here.
    await click("main button", "Alle Turniere");
    await find("main .gname[title*='#3500000001']");
    return;
  }
  await find("main .moment.bad .ui-reason", "Warum?", { exact: false });
  await click("main .moment.bad button", "Abspielen");
  await find("main h1", "Hände");
  await find(".replayer");
  await anteLine();
  await click("main button[aria-label='Turnierfilter entfernen']");
});

step("Hände: Replayer öffnet eine Hand", async () => {
  await nav("Hände");
  await click("main [role='option']");
  await find(".replayer");
  await click(".replayer button[aria-label^='Weiter']");
});

step("Hände: die Analyse zeigt eine Entscheidung erst, wenn das Abspielen dort steht", async () => {
  const TABS = "main [role='tablist'][aria-label^='Deine Entscheidungen'] [role='tab']";
  const verdict = () => s.exec(`return !!document.querySelector("main .decision")`);
  if (!(await s.exec(`return !!document.querySelector(arguments[0])`, TABS))) {
    if (!TAURI) throw new Error("the demo hand has rated decisions");
    log("hand without decisions: replay order not checked");
    return;
  }
  await click(TABS);
  await find("main .decision");
  // One step back the replay stands before the action ahead of your first decision.
  await click(".replayer button[aria-label^='Zurück']");
  await waitText("Gleich bist du dran");
  if (await verdict()) throw new Error("the first decision's verdict shows before the replay reaches it");
  await click(".replayer button[aria-label^='Weiter']");
  await find("main .decision");
  // Every rated decision says why the best move is best.
  await find("main .decision .ui-reason", "Warum?", { exact: false });
});

step("Hände: auch die beste Aktion zeigt, was sie bringt", async () => {
  const value = await s.exec(`
    const row = [...document.querySelectorAll("main .decision .options li")].find((li) => li.querySelector(".ev.best"));
    return row ? row.querySelector(".gain")?.textContent.trim() ?? "" : null;`);
  if (value === null) {
    if (!TAURI) throw new Error("the demo decision lists its actions");
    log("decision without a list of actions: „bringt“ not checked");
    return;
  }
  if (!/^[+−]?\d+,\d\d BB$/.test(value)) throw new Error(`the best action brings „${value}“`);
});

/** Scroll boxes in "Hände" that are wider than their room, and how far the analysis sits from the right edge. */
const OVERFLOW = `
  const boxes = [document.querySelector("main .content"), ...document.querySelectorAll("main .hands *")].filter((e) => {
    if (!e || !e.getClientRects().length) return false;
    const o = getComputedStyle(e);
    return /auto|scroll/.test(o.overflowX + o.overflowY) && e.scrollWidth > e.clientWidth;
  });
  const content = document.querySelector("main .content").getBoundingClientRect();
  const aside = document.querySelector("main .replayer .aside");
  return {
    width: innerWidth,
    wide: boxes.map((e) => e.className + " +" + (e.scrollWidth - e.clientWidth) + "px"),
    gap: aside ? Math.round(content.right - aside.getBoundingClientRect().right) : null,
  };`;

step("Hände: nichts scrollt seitwärts, die Analyse hält Abstand zum Rand", async () => {
  const check = async () => {
    // The replayer lays out over a few frames after a resize.
    await sleep(600);
    const r = await s.exec(OVERFLOW);
    log(`hands overflow at ${r.width}px: ${JSON.stringify(r)}`);
    if (r.wide.length) throw new Error(`scrolls sideways at ${r.width}px: ${r.wide.join(", ")}`);
    if (r.gap !== null && r.gap < 16) throw new Error(`the analysis is ${r.gap}px from the right edge at ${r.width}px`);
    return r;
  };
  await check();
  // The analysis only stands beside the table in wide windows: the bug lived there.
  try {
    await s.cmd("POST", "/window/rect", { width: 1920, height: 1080 });
  } catch (e) {
    log(`window could not be resized, only ${await s.exec("return innerWidth")}px checked: ${e.message}`);
    return;
  }
  try {
    const r = await check();
    if (r.width >= 1700 && r.gap === null) throw new Error(`no analysis beside the table at ${r.width}px`);
  } finally {
    await s.cmd("POST", "/window/rect", { width: 1280, height: 820 });
  }
});

// The demo's decisions link the solver with the line to them; the app's need a solved spot.
// The demo hand with A♥Q♦ is heads-up after the flop, so the solver rates it.
!TAURI &&
  step("Analyse: „Im Solver öffnen“ spielt die Hand bis zur Entscheidung", async () => {
    const hand = await waitFor(
      "the heads-up demo hand",
      `return [...document.querySelectorAll("main [role='option']")].find((o) => o.querySelector("[aria-label='A Herz']") && o.querySelector("[aria-label='Q Karo']")) ?? null`,
    );
    await s.exec(`arguments[0].scrollIntoView({ block: "center" })`, hand);
    await s.click(hand);
    await click("main .steps button[title^='Turn: Check']");
    await find("main .decision", "Solver", { exact: false });
    await click("main article.ui-verdict button", "Im Solver öffnen");
    // The line stands at the hero's turn decision: the turn card is dealt, the last step is on.
    await waitFor(
      "the line up to the turn decision",
      `const line = document.querySelector("main nav.line"); const steps = line ? [...line.querySelectorAll("button")] : [];
       return steps.some((b) => b.classList.contains("card") && b.textContent.includes("Turn")) && steps.at(-1)?.classList.contains("on") ? line : null`,
      [],
      120_000,
    );
    await find("main [role='group'][aria-label='Aktionen']");
    await find("main .cell.hero");
  });

step("Leaks", () => nav("Leaks"));
step("Deine Ranges: Chart mit Modell, Klick auf eine Hand zeigt ihre Hände", async () => {
  await nav("Deine Ranges");
  await waitText(/öffnest du|spielst du|callst du/);
  await find("main figure figcaption b", "Du");
  // A changed filter can be reset to the opening selection.
  const reset = await find("main .situation button", "Filter zurücksetzen");
  if (!(await s.exec(`return arguments[0].disabled`, reset))) throw new Error("reset is enabled before any filter changed");
  await click("main .situation button.tile:not(.on):not([disabled])");
  await waitFor("reset enabled", `return arguments[0].disabled ? null : true`, [reset]);
  await s.click(reset);
  await waitFor("reset disabled again", `return arguments[0].disabled ? true : null`, [reset]);
  const cell = await waitFor(
    "a visible cell with hands",
    `const you = [...document.querySelectorAll("main figure")].find((f) => f.querySelector("figcaption b")?.textContent.trim() === "Du");
    return [...(you?.querySelectorAll("button.cell.in") ?? [])].find((c) => c.offsetParent) ?? null`,
  );
  const name = (await s.exec(`return arguments[0].querySelector(".k").textContent`, cell)).trim();
  await s.exec(`arguments[0].scrollIntoView({ block: "center" })`, cell);
  await s.click(cell);
  await find("main h1", "Hände");
  await find("main .chip-name", name, { exact: false });
});
step("Spieler", () => nav("Spieler"));
// The demo has known opponents; PokerStars is closed there, so type and counter may show.
!TAURI &&
  step("Spieler: Suche filtert, das Detail nennt Typ, Gegenzug und Stichprobe", async () => {
    await find("main tbody td.name button", "Rüdiger", { exact: false });
    await s.exec(`const i = document.querySelector("main input[type=search]"); i.value = "Rüd"; i.dispatchEvent(new Event("input", { bubbles: true }));`);
    await waitFor("nur Rüdiger in der Liste", `return document.querySelectorAll("main tbody tr").length === 1`);
    await click("main tbody td.name button", "Rüdiger", { exact: false });
    const text = await waitFor("das Detail von Rüdiger", `return document.querySelector("aside[aria-label='Spieler Rüdiger'] .read")?.closest("aside").innerText ?? null`);
    for (const want of ["Calling Station", "Gegenzug", "Mit guten Händen oft und groß setzen, kaum bluffen.", "nach 96 Händen", "aus PokerStars"])
      // Kickers are upper case on screen, so compare without case.
      if (!text.toLowerCase().includes(want.toLowerCase())) throw new Error(`detail lacks „${want}“`);
    await s.exec(`const i = document.querySelector("main input[type=search]"); i.value = ""; i.dispatchEvent(new Event("input", { bubbles: true }));`);
    await waitFor("die ganze Liste", `return document.querySelectorAll("main tbody tr").length > 1`);
    await s.exec(`document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }))`);
  });
step("Live-Tische", () => nav("Live-Tische"));
!TAURI &&
  step("Live-Tische: ein Gegner zeigt seine Werte mit Stichprobe", async () => {
    await click("main tbody td.name button", "Kestrel", { exact: false });
    const text = await waitFor("das Detail von Kestrel", `return document.querySelector("aside[aria-label='Spieler Kestrel']")?.innerText ?? null`);
    if (!/aus\s27\sHänden/.test(text)) throw new Error("detail lacks the sample size");
    await s.exec(`window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }))`);
    await waitFor("Detail geschlossen", `return !document.querySelector("aside[aria-label='Spieler Kestrel']")`);
  });
step("HUD", () => nav("HUD"));

step("Training: Pot Odds, Stufe 1", async () => {
  await nav("Lernen");
  // PokerStars is closed, so the header must not claim the page is locked.
  if (await s.exec(`return !!document.querySelector("main .ui-source.locked")`)) throw new Error("lock note shown while PokerStars is closed");
  await click("main button", "Trainer");
  // Skill and level buttons also show the user's level and the level's name.
  await click("main button.skill", "Pot Odds", { exact: false });
  await click("main button.level", "Stufe 1", { exact: false });
  const answer = await find("main input[aria-label='Antwort in %']");
  await waitText("Equity");
  await s.type(answer, "30");
  await click("main button", "Prüfen", { exact: false });
  await waitText("Lösung");
  await find("main button", "Nächste Aufgabe", { exact: false });
});

step("Training: All-in nach Folds", async () => {
  await click("main button.skill", "All-in nach Folds", { exact: false });
  await waitText("gefoldet");
  await click("main button.choice.fold");
  await find("main button", "Nächste Aufgabe", { exact: false });
});

/** Opens a skill at level 1 in the trainer and returns the question once it is that skill's. */
async function drill(button, short) {
  await click("main button.skill", button, { exact: false });
  await click("main button.level", "Stufe 1", { exact: false });
  return waitFor(
    `a question of ${short}`,
    `const t = document.querySelector("main .task");
     return t?.querySelector(".kind")?.textContent.trim() === arguments[0] && !t.querySelector(".fb") ? t.querySelector("h2.q").textContent : null;`,
    [short],
  );
}

/** Numbers of a German sentence: "7.550" → 7550, "2,6" → 2.6. */
const numbers = (text) => [...text.matchAll(/\d[\d.]*(?:,\d+)?/g)].map((m) => Number(m[0].replace(/\.(?=\d{3})/g, "").replace(",", ".")));

async function answerNumber(unit, value) {
  const input = await find(`main input[aria-label='Antwort in ${unit}']`);
  await s.type(input, String(Math.round(value * 10) / 10));
  await click("main button", "Prüfen", { exact: false });
}

/** The verdict and the solution's text once the answer is graded. */
async function graded() {
  await find("main .task .fb");
  await find("main button", "Nächste Aufgabe", { exact: false });
  return s.exec(`const f = document.querySelector("main .task .fb");
    return { right: f.classList.contains("ok"), truth: f.querySelector(".what b").textContent.trim() };`);
}

async function solved(what) {
  const g = await graded();
  if (!g.right) throw new Error(`${what}: graded wrong, solution ${g.truth}`);
}

/** A decision whose answer only the engine knows: the verdict has to agree with the solution. */
async function decided(id, label) {
  await click(`main button.choice.${id}`);
  const g = await graded();
  if (g.right !== (g.truth === label)) throw new Error(`answered ${label}, solution ${g.truth}, graded ${g.right ? "right" : "wrong"}`);
}

/** Malmuth–Harville: places go out top-down in proportion to the chips. */
function icmValue(stacks, payouts, seat) {
  const go = (left, place) => {
    if (place >= payouts.length) return 0;
    const total = left.reduce((a, i) => a + stacks[i], 0);
    return left.reduce((sum, i) => {
      const p = stacks[i] / total;
      const rest = left.filter((j) => j !== i);
      return sum + p * ((i === seat ? payouts[place] : 0) + (i === seat ? 0 : go(rest, place + 1)));
    }, 0);
  };
  return go(stacks.map((_, i) => i), 0);
}

step("Training: Stacktiefe, Stufe 1 gelöst", async () => {
  const q = await drill("Stacktiefe", "Stacktiefe");
  const [stack, , bigBlind] = numbers(q);
  await answerNumber("bb", stack / bigBlind);
  await solved(q);
});

step("Training: Pot Odds mit Kopfgeld, Stufe 1 gelöst", async () => {
  const q = await drill("Pot Odds mit Kopfgeld", "Pot Odds mit Kopfgeld");
  const [shove, dead, stack, bounty] = numbers(q);
  const call = Math.min(shove, stack);
  await answerNumber("%", (100 * call) / (dead + call + call + bounty));
  await solved(q);
});

step("Training: Gegner lesen, Stufe 1 gelöst", async () => {
  const q = await drill("Gegner lesen", "Gegner lesen");
  const [vpip, pfr] = numbers(q);
  // The rules of thumb of crates/ringside-learn/src/hud.rs.
  const type = vpip < 15 ? "nit" : vpip < 27 ? "tag" : pfr / vpip >= 0.6 ? "lag" : "station";
  await click(`main button.choice.${type}`);
  await solved(q);
});

step("Training: ICM-Wert, Stufe 1 gelöst", async () => {
  const q = await drill("ICM und Bubble", "ICM und Bubble");
  const prizes = [...q.matchAll(/(\d+) \$/g)].map((m) => Number(m[1]));
  const rows = await s.exec(`return [...document.querySelectorAll("main ol.stacks li")].map((li) => ({
      bb: Number(li.querySelector(".num").textContent.replace(/[^0-9,]/g, "").replace(",", ".")),
      asked: li.classList.contains("asked") }))`);
  const seat = rows.findIndex((r) => r.asked);
  await answerNumber("$", icmValue(rows.map((r) => r.bb), prizes, seat));
  await solved(q);
});

step("Training: Öffnen oder folden, Stufe 1", async () => {
  await drill("Öffnen oder folden", "Öffnen oder folden");
  await decided("fold", "Fold");
  await find("main .task .fb [aria-label^='Open-Range']");
});

step("Training: Bet oder Check am River, Stufe 1", async () => {
  await drill("Bet oder Check am River", "Bet oder Check am River");
  await decided("check", "Check");
  await waitText("Gleichgewicht des River-Spiels");
});

// The app's fixture hands are too few to rank a kind of spot; there the page has to say why it is empty.
step("Heute: aus deinem Spiel üben, Hände der Schwachstelle ansehen", async () => {
  await click("main [role='tab']", "Heute");
  await waitText("Runde starten");
  if (TAURI && !/Zuerst: aus deinem Spiel/.test(await mainText())) {
    await waitText(/Schwachstellen erscheinen nach der Analyse|Deine Schwachstellen/);
    return;
  }
  await waitText("Zuerst: aus deinem Spiel");
  const title = await s.exec(`return document.querySelector("main .rows .row .title")?.textContent.trim() ?? null`);
  if (!title) throw new Error("no kind of spot among the weaknesses");
  await click("main .rows .row button", "Üben", { exact: false });
  await find("main .drill .origin", "Echte Hand", { exact: false });
  await click("main .drill .choices button");
  await find("main .drill .verdict");
  await find("main .drill .verdict .ui-reason", "Warum?", { exact: false });
  // An estimate's table counts from folding: Fold brings 0,00 BB, its cost is the gap to the best move.
  const fold = await s.exec(`
    const card = document.querySelector("main .drill .verdict");
    if (!card || !card.textContent.includes("geschätzt")) return null;
    const row = [...card.querySelectorAll(".options li")].find((li) => li.querySelector(".label")?.textContent.trim().startsWith("Fold"));
    return row ? { gain: row.querySelector(".gain").textContent.trim(), cost: row.querySelector(".ev").textContent.trim() } : null;`);
  if (fold && fold.gain !== "0,00 BB") throw new Error(`an estimated fold brings „${fold.gain}“`);
  if (!TAURI && fold?.cost !== "−3,62 BB") throw new Error(`the demo fold costs „${fold?.cost}“, not −3,62 BB as before`);
  await waitText("Damals hast du");
  await click("main .drill button", "Weiter", { exact: false });
  // The round goes on with the next spot of the same kind: real or dealt anew, it says which.
  await find("main .drill .origin");
  await click("main button", "Runde beenden");
  await click("main button", "Noch eine Runde");
  await click("main .rows .row button", "Hände ansehen", { exact: false });
  await find("main h1", "Hände");
  await waitFor("the weakness as filter chip", `return document.querySelector("main")?.innerText.includes(arguments[0])`, [title]);
});

step("Deine Hände: die Antes stehen in einer Zeile, nicht je Spieler", async () => {
  await nav("Lernen");
  await click("main [role='tab']", "Deine Hände");
  const found = await waitFor(
    "a hand or the empty list",
    `return document.querySelector("main .replayer [aria-label='Zugverlauf']") ? "hand" : /Noch keine passenden Spots/.test(document.querySelector("main").innerText) && "none"`,
  );
  if (found === "none") {
    if (!TAURI) throw new Error("the demo has own spots");
    return;
  }
  const line = await anteLine();
  // The demo's spots are tournament hands with eight antes.
  if (!TAURI && !/^Antes 8 × .+ = /.test(line ?? "")) throw new Error(`ante line: ${line}`);
});

/** Sets the table size with the − / + buttons in the middle of the table. */
async function tableSize(n) {
  await s.exec(
    `const want = arguments[0];
     const cur = parseInt(document.querySelector(".stepper span").textContent);
     const b = document.querySelector(cur < want ? '.stepper button[aria-label="Ein Spieler mehr"]' : '.stepper button[aria-label="Ein Spieler weniger"]');
     for (let i = 0; i < Math.abs(want - cur); i++) b.click();`,
    n,
  );
}

/** Moves the stack slider to `bb` the way dragging would: the slider runs on a log scale. */
async function stackTo(bb) {
  await s.exec(
    `const el = document.querySelector("#stack-axis");
     const st = el.dataset.values.split(",").map(Number);
     const lo = Math.log(st[0]), hi = Math.log(st[st.length - 1]);
     el.value = String(Math.round(((Math.log(arguments[0]) - lo) / (hi - lo)) * 1000));
     el.dispatchEvent(new Event("input", { bubbles: true }));`,
    bb,
  );
}

step("Ranges nach Stack: Push/Fold, 6 Spieler, 10 BB aus der Tabelle", async () => {
  await nav("Ranges nach Stack");
  await find(".stepper span");
  await tableSize(6);
  await find("#stack-axis");
  await stackTo(10);
  await waitText("6 Spieler, alle mit 10 BB");
  await waitText(TAURI ? "Vorgerechnet" : "Quelle");
  await waitText("Gleichgewicht");
});

step("Ranges nach Stack: Modell mit 50 BB", async () => {
  await stackTo(50);
  await waitText(/erhöhst du mit etwa \d+ %/);
  await waitText("Modell");
});

step("Ranges nach Stack: Satz oben und Range nennen denselben Platz, auch nach dem Wechsel der Tischgröße", async () => {
  // 25 BB is the model, 10 BB push/fold: both name the seat from the same list.
  for (const [n, bb] of [[9, 25], [9, 10], [6, 25]]) {
    await tableSize(n);
    await stackTo(bb);
    await find(".stepper span", `${n} Spieler`);
    const names = await waitFor(
      `beide Sätze bei ${n} Spielern und ${bb} BB`,
      `const seat = (el) => el?.textContent.replace(/\\s+/g, " ").match(new RegExp("^(?:Von|Vom|Aus dem) (\\\\S+) mit " + arguments[0] + " BB"))?.[1];
       const top = seat(document.querySelector(".pfr b"));
       const range = seat(document.querySelector("section.chart-panel:not([aria-busy='true']) header.answer h2"));
       return top && range ? { top, range } : null;`,
      [bb],
    );
    if (names.top !== names.range) throw new Error(`${n} Spieler, ${bb} BB: oben ${names.top}, Range ${names.range}`);
  }
});

step("Ranges nach Stack: Tischgrößen-Vergleich, Klick auf heads-up wechselt den Tisch", async () => {
  const panel = "main section[aria-label='Andere Tischgröße']";
  await find(panel);
  await waitText(/Als Erster am Zug spielst du an 9-max \d+\s%, heads-up \d+\s%/);
  const first = `${panel} [role='group'][aria-label^='Als Erster am Zug'] button`;
  const values = await waitFor(
    "the bars of the first seat",
    `const b = [...document.querySelectorAll(arguments[0])];
     return b.length >= 7 ? b.map((x) => ({ n: x.querySelector(".lbl").textContent, v: Number(x.querySelector(".val").textContent) })) : null;`,
    [first],
  );
  if (values.map((x) => x.n).join() !== "9,8,6,5,4,3,HU") throw new Error(`bars ${values.map((x) => x.n).join(", ")}`);
  for (let i = 1; i < values.length; i++)
    if (values[i].v <= values[i - 1].v) throw new Error(`first in does not widen with fewer players: ${values.map((x) => `${x.n} ${x.v}`).join(" · ")}`);
  await click(first, "HU", { exact: false });
  await find(".stepper span", "2 Spieler");
  await waitText(/Heads-up bist du als Button selbst Small Blind|Als Erster am Zug spielst du/);
  await tableSize(6);
  await find(".stepper span", "6 Spieler");
});

step("Ranges nach Stack: Schnellwahl „10 BB“ ändert Wert und Satz", async () => {
  await click("main [role='group'][aria-label='Schnellwahl Dein Stack'] button", "10");
  await waitFor("10 BB oben rechts", `return document.querySelector("main output.val")?.textContent.replace(/\\s+/g, " ").trim() === "10 BB"`);
  await waitText(/Vo[nm] [^\n]*mit 10 BB (erhöhst|foldest) du/);
});

/** Presses one key the way a keyboard would, on whatever has the focus. */
async function press(key) {
  await s.cmd("POST", "/actions", { actions: [{ type: "key", id: "k", actions: [{ type: "keyDown", value: key }, { type: "keyUp", value: key }] }] });
}

step("Begriffe per Tastatur: Tab erreicht die Erklärung, Esc schließt sie", async () => {
  // Tab from the stack slider to the next explained word on the page.
  await s.exec(`document.querySelector("#stack-axis").focus()`);
  for (let i = 0; i < 40; i++) {
    await press("\uE004");
    if (await s.exec(`return !!document.activeElement?.matches(".ui-tooltip-anchor")`)) break;
  }
  await waitFor("Erklärung beim Fokus", `return !!document.querySelector(".ui-tooltip")`);
  await press("\uE00C");
  await waitFor("Erklärung zu nach Esc", `return !document.querySelector(".ui-tooltip")`);
});

/** Sets a field's text the way typing would, replacing what is there. */
async function fill(sel, text) {
  const el = await find(sel);
  await s.exec(
    `const [el, v] = arguments; el.value = v; el.dispatchEvent(new Event("input", { bubbles: true })); el.dispatchEvent(new Event("change", { bubbles: true }));`,
    el,
    text,
  );
}

step("Solver: Durchspielen erklärt vor dem Lösen, was dort kommt", async () => {
  await nav("Solver");
  await click("main [role='tab']", "Durchspielen", { exact: false });
  // A spot opened from the hand analysis is solved already; without it the tab explains itself.
  await s.exec(`[...document.querySelectorAll("main button")].find((b) => b.textContent.trim() === "Verwerfen")?.click()`);
  await click("main [role='tab']", "Durchspielen", { exact: false });
  await waitText("Durchspielen startet nach dem Lösen");
  await find("main button", "Spot lösen", { exact: false });
});

step("Solver: kleiner River-Spot mit deiner Hand", async () => {
  await click("main [role='tab']", "Aufbau");
  // A new table drops a hand handed over from the analysis, so you pick your own.
  await click("main [aria-label='Spieler am Tisch'] button", "6-max");
  // Five board cards make it a river spot, which solves in a moment.
  await s.exec(`[...document.querySelectorAll("main button")].find((b) => b.textContent.trim() === "Board leeren")?.click()`);
  const slots = ["Flop", "Flop", "Flop", "Turn", "River"];
  for (const [i, card] of ["Ks", "7d", "2c", "3c", "4d"].entries()) {
    await click(`main .board button.ui-slot[aria-label='${slots[i]}-Karte wählen']`);
    await click(`.deck-pop button.pick[aria-label='${card}']:not(:disabled)`);
  }
  // At once, not after the estimate: the hint follows the board, not the debounced estimate.
  if ((await mainText()).includes("Wähle mindestens drei Boardkarten")) throw new Error("five board cards, yet the page asks for three");
  await click("main button", "Ranges anpassen");
  await fill("main textarea[aria-label='Range von OOP als Text']", "AA,KK");
  await fill("main textarea[aria-label='Range von IP als Text']", "QQ,JJ");
  await click("main button", "zuerst dran", { exact: false });
  await click("main button.ui-slot[aria-label='Deine Hand wählen (optional)']");
  await click(".deck-pop button.pick[aria-label='As']:not(:disabled)");
  await click(".deck-pop button.pick[aria-label='Ah']:not(:disabled)");
  await click("main button.ui-btn.primary:not(:disabled)", "Lösen");
  await find("main [role='group'][aria-label='Aktionen']", null, { timeout: 180_000 });
  await waitText("Mit deiner Hand hier");
});

step("Solver: die Lösung steht in „Meine Lösungen“ und öffnet ohne Rechnen", async () => {
  await click("main [role='tab']", "Meine Lösungen");
  await waitText("Selbst gerechnet");
  await click("main .saved li.card button.ui-btn.primary");
  await find("main [role='group'][aria-label='Aktionen']");
});

step("Aufgaben: der Solve steht erledigt im Panel", async () => {
  await click("aside button.dock");
  await find(TAURI ? "#jobs-panel .job.done" : "#jobs-panel");
  await click("#jobs-panel button.close");
});

step("Bibliothek: Mitrechnen startet von Hand, steht in den Aufgaben und hält an", async () => {
  await nav("Bibliothek");
  await click("main button.go", "Jetzt mitrechnen");
  await find("main button.go", "Anhalten");
  if (TAURI) {
    await click("aside button.dock");
    await find("#jobs-panel .job", "Bibliothek: Mitrechnen", { exact: false });
    await click("#jobs-panel button.close");
  }
  await click("main button.go", "Anhalten");
  // The headline depends on the automatic mode ("Angehalten", or "Aus" when it is off).
  await find("main button.go", "Jetzt mitrechnen", { timeout: 60_000 });
});

// Practice table. Every move is found and clicked inside the page in one call, then the test
// waits for its effect: a button read in one call may show the next decision by the next one
// (the buttons are kept per index), and a fixed pause misses an end that is still drawing.

/** "end", "solving", "decide" (your buttons are live) or null while the table plays on. */
const playNow = (timeout = 60_000) =>
  waitFor(
    "your decision, a live solve or the end",
    `if (document.querySelector("main section.end")) return "end";
     if (document.querySelector("main [data-part='solving']")) return "solving";
     return document.querySelector("main .actions button.act:not(:disabled)") ? "decide" : null;`,
    [],
    timeout,
  );

const heroMoves = () => s.exec(`return document.querySelectorAll("main nav.timeline button.move.hero").length`);

/** Plays the first live button matching `want` (else the first one, unless `strict`) and waits until the move is in the history. */
async function playMove(want, { strict = false } = {}) {
  const before = await heroMoves();
  const label = await s.exec(
    `const b = [...document.querySelectorAll("main .actions button.act:not(:disabled)")];
     const m = b.find((x) => new RegExp(arguments[0]).test(x.textContent)) ?? (arguments[1] ? null : b[0]);
     if (!m) return null;
     m.click();
     return m.textContent.replace(/\\s+/g, " ").trim();`,
    want.source,
    strict,
  );
  if (!label) throw new Error(`no button like /${want.source}/ to play`);
  await waitFor(`your move „${label}“ in the history`, `return document.querySelectorAll("main nav.timeline button.move.hero").length > arguments[0]`, [before]);
  return label;
}

/** Stops a live solve if it still runs; the hand plays on without a grade. */
const cancelSolve = () =>
  s.exec(`const b = document.querySelector("main [data-part='solving'] button:not(:disabled)"); b?.click(); return !!b;`);

/** Deals the next hand from the end of one and waits until it is on the table. */
async function nextHandFromEnd() {
  const clicked = await s.execAsync(
    `const done = arguments[arguments.length - 1];
     const b = [...document.querySelectorAll("main section.end button")].find((x) => /Nächste Hand/.test(x.textContent));
     if (!b || b.disabled) return done(false);
     b.click();
     setTimeout(() => done(true), 0);`,
  );
  if (!clicked) throw new Error("no „Nächste Hand“ to click");
  // While the hand is dealt the old end stays with its buttons off.
  await waitFor("the next hand", `return ![...document.querySelectorAll("main section.end button")].some((x) => x.disabled)`);
}

step("Hände spielen: eine Hand startet", async () => {
  await click("nav button", "Hände spielen");
  await find("main h1", "Hände spielen");
  await click("main .tiles button.tile");
  await waitText("Tipp anzeigen");
});

step("Hände spielen: bis zum nächsten Zug steht jede Aktion im Verlauf", async () => {
  await click("main [role='radiogroup'][aria-label='Tempo der Animation'] button", "Sofort");
  const positions = await waitFor(
    "the seats",
    `const p = [...document.querySelectorAll("main [data-part='seat'] .plate b")].map((b) => b.textContent.replace("Du · ", "").trim());
     return p.length ? p : null;`,
  );
  // Open or raise where you can, so the others have something to answer.
  const move = await waitFor(
    "your buttons",
    `const b = [...document.querySelectorAll("main .actions button.act")];
     return b.find((x) => /Raise/.test(x.textContent)) ?? b.find((x) => /Call/.test(x.textContent)) ?? b[0] ?? null;`,
  );
  await s.click(move);
  await waitFor("your move in the history", `return document.querySelector("main nav.timeline button.move.hero") ?? null`);
  // Your next decision, a live solve of the flop or the end: everyone has acted preflop by then.
  await waitFor(
    "the next decision, a live solve or the end",
    `return document.querySelector("main .actions button.act, main [data-part='solving'], main section.end") ?? null`,
    [],
    60_000,
  );
  const preflop = await s.exec(
    `const street = document.querySelector("main nav.timeline section");
     return [...street.querySelectorAll("button.move")].map((b) => b.dataset.pos);`,
  );
  const missing = positions.filter((p) => !preflop.includes(p));
  if (missing.length) throw new Error(`no action of ${missing.join(", ")} in the history: ${preflop.join(" · ")}`);
  if (!TAURI) {
    // The demo hand: LJ folds, you open, CO 3-bets, BTN, SB and BB fold.
    const want = ["LJ", "HJ", "CO", "BTN", "SB", "BB"];
    if (preflop.join() !== want.join()) throw new Error(`history ${preflop.join(" · ")}, expected ${want.join(" · ")}`);
    await waitText(/CO hat auf 6,1 BB erhöht\s\(3-Bet\), BTN, SB und BB haben gefoldet\./);
    await find("main section[aria-label='Deine Züge in dieser Hand'] button.move", null, { exact: false });
  }
});

step("Hände spielen: die Hand läuft bis zum Handende, auch über einen live gerechneten Spot", async () => {
  // Call or check wherever you can, so the hand goes as far as it can. In the demo the call
  // of the 3-bet leaves the prepared line and the flop is solved live.
  let solving = null;
  let solved = false;
  const until = Date.now() + 300_000;
  for (;;) {
    if (Date.now() > until) throw new Error("the hand did not reach its end");
    const now = await playNow(TAURI ? 120_000 : 60_000);
    if (now === "end") break;
    if (now === "solving") {
      solving ??= Date.now();
      // A slow machine may take long for a live spot: then the hand goes on without it.
      if (TAURI && Date.now() - solving > 60_000) await cancelSolve();
      await sleep(250);
    } else {
      if (solving !== null) solved = true;
      await playMove(/Call|Check/);
    }
  }
  const text = await mainText();
  if (/anders weitergegangen|endet diese Hand|nichts mehr hinterlegt/.test(text)) throw new Error("the hand stopped before its end");
  if (!/Du gewinnst|Du foldest|gewinnt|Geteilter Pot|Alle anderen folden/.test(text)) throw new Error("no result of the hand");
  if (!TAURI) {
    if (!solved) throw new Error("the demo hand was not solved live");
    await waitText("Du gewinnst beim Showdown");
    await click("main section[aria-label='Deine Züge in dieser Hand'] button.move", "Flop", { exact: false });
    await waitText(/Quelle: Solver, live gerechnet .* Genauigkeit 1,8 % des Pots/);
  }
});

step("Hände spielen: heads-up ist der Button der Small Blind, zuerst vor dem Flop, zuletzt danach", async () => {
  await click("main button.back");
  await click("main .tiles button.tile", "Heads-up", { exact: false });
  // Your seat is dealt at random in the app: play hands until one reaches a flop.
  for (let hand = 0; ; hand++) {
    if (hand >= 15) throw new Error("no heads-up flop in 15 hands");
    let now = await playNow();
    const table = await s.exec(
      `const seats = [...document.querySelectorAll("main [data-part='seat']")].map((el) => {
         const r = el.getBoundingClientRect();
         return { name: el.querySelector(".plate b").textContent.replace("Du · ", "").trim(), x: (r.left + r.right) / 2, y: (r.top + r.bottom) / 2 };
       });
       const d = document.querySelector("main [data-part='dealer']").getBoundingClientRect();
       const dx = (d.left + d.right) / 2, dy = (d.top + d.bottom) / 2;
       const near = seats.reduce((a, b) => (Math.hypot(a.x - dx, a.y - dy) < Math.hypot(b.x - dx, b.y - dy) ? a : b));
       return { names: seats.map((x) => x.name).sort(), button: near.name };`,
    );
    if (table.names.join() !== "BB,SB") throw new Error(`heads-up seats ${table.names.join(", ")}`);
    if (table.button !== "SB") throw new Error(`the dealer button sits at ${table.button}, not at the small blind`);
    let flop = null;
    while (now !== "end" && !flop) {
      if (now === "solving") {
        await cancelSolve();
        await sleep(250);
      } else {
        const streets = await s.exec(
          `return [...document.querySelectorAll("main nav.timeline section.street")].map((sec) => ({
             name: sec.querySelector(".name").textContent.trim(),
             first: sec.querySelector("button.move")?.dataset.pos ?? null,
           }));`,
        );
        if (streets[0]?.first && streets[0].first !== "SB") throw new Error(`preflop starts with ${streets[0].first}`);
        flop = streets.find((x) => x.name === "Flop" && x.first) ?? null;
        if (!flop) await playMove(/Raise|Call|Check/);
      }
      if (!flop) now = await playNow();
    }
    if (flop) {
      if (flop.first !== "BB") throw new Error(`the flop starts with ${flop.first}, not with the big blind`);
      break;
    }
    await nextHandFromEnd();
  }
  await waitText(/Heads-up: Du \((SB, mit|BB, ohne) Position\)/);
  if (!TAURI) {
    // The demo hand: you open on the button, the big blind calls and checks to you on every street.
    let checks = 0;
    while ((await playNow()) === "decide") {
      if (++checks > 3) throw new Error("more than three decisions after the flop");
      await playMove(/Check/, { strict: true });
    }
    if (checks !== 3) throw new Error(`${checks} checks to the showdown, expected 3`);
    await waitText("Du gewinnst beim Showdown");
  }
});

step("KI-Coach: ohne Key erscheint der Hinweis", async () => {
  await nav("KI-Coach");
  if (TAURI) await waitText("Für den Coach brauchst du einen API-Key");
  if (hits.OpenRouter) throw new Error(`${hits.OpenRouter} connection(s) to OpenRouter`);
});

step("Einstellungen: Cloud-Schalter aus", async () => {
  await nav("Einstellungen");
  const boxes = await waitFor(
    "the cloud switches",
    `const boxes = [...document.querySelectorAll("main #s-cloud [role=switch]")].map((b) => b.getAttribute("aria-checked") === "true");
     return boxes.length ? boxes : null;`,
  );
  if (boxes.length !== 3) throw new Error(`expected 3 cloud switches, found ${boxes.length}`);
  if (TAURI && (boxes[0] || boxes[1])) throw new Error("cloud switches are on");
  if (!boxes[2]) throw new Error("„Üben mit fertigen Spots anderer“ is off by default");
});

step("Einstellungen: „Neue Hände automatisch analysieren“ ist an und bleibt gespeichert", async () => {
  // Switching PokerStars on and off cannot be done here; the Rust tests of auto_analysis.rs cover the runs.
  const LABEL = "Neue Hände automatisch analysieren";
  const shows = (on) =>
    waitFor(
      `the auto-analysis switch ${on ? "on" : "off"}`,
      `const box = document.querySelector(\`main [role=switch][aria-label="\${arguments[0]}"]\`);
       return box?.getAttribute("aria-checked") === String(arguments[1]);`,
      [LABEL, on],
    );
  const saved = async (on) => {
    for (let tries = 0; TAURI; tries++) {
      if ((await invoke("get_settings")).analysis?.auto === on) return;
      if (tries === 20) throw new Error(`the setting is not stored as ${on}`);
      await sleep(250);
    }
  };
  const reopen = async () => {
    await nav("Übersicht");
    await nav("Einstellungen");
  };
  await waitText("Solange PokerStars läuft, rechnet Ringside nie");
  await shows(true);
  await saved(true);
  for (const on of [false, true]) {
    await click(`main [role=switch][aria-label="${LABEL}"]`);
    await shows(on);
    await saved(on);
    await reopen();
    await shows(on);
  }
});

// ---------------------------------------------------------------------------------
// Run.

let failed = null;
for (const [i, { name, fn }] of steps.entries()) {
  const started = Date.now();
  const file = `${String(i + 1).padStart(2, "0")}`;
  try {
    await fn();
    const text = await mainText();
    writeFileSync(path.join(OUT, "pages", `${file}.txt`), `${name}\n\n${text}`);
    const shown = text.match(ERROR_TEXT);
    if (shown) throw new Error(`the page shows an error: „${shown[0]}“`);
    const errors = await consoleErrors();
    if (errors.length) throw new Error(`console errors:\n  ${errors.join("\n  ")}`);
    console.log(`ok   ${name} (${((Date.now() - started) / 1000).toFixed(1)} s)`);
  } catch (e) {
    failed = { name, error: e };
    console.log(`FAIL ${name}: ${e.message}`);
    break;
  }
}

if (!failed && hits["Ringside Cloud"]) {
  failed = { name: "Netz", error: new Error(`${hits["Ringside Cloud"]} connection(s) to Ringside Cloud`) };
  console.log(`FAIL ${failed.error.message}`);
}
if (process.env.E2E_NET_TRAP) console.log(`network: ${JSON.stringify(hits)}`);

if (failed) {
  try {
    writeFileSync(path.join(OUT, "failure.png"), Buffer.from(await s.screenshot(), "base64"));
    writeFileSync(path.join(OUT, "failure.txt"), `${failed.name}\n${failed.error.stack}\n\n${await mainText()}`);
    await consoleErrors();
  } catch (e) {
    console.log(`could not save the failure: ${e.message}`);
  }
}

await s.quit().catch(() => {});
for (const child of children) child.kill();
for (const srv of servers) srv.close();
consoleFile.end();
process.exit(failed ? 1 : 0);
