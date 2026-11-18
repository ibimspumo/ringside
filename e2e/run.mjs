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
  await s.click(el);
}

const mainText = () => s.exec(`return document.querySelector("main")?.innerText ?? ""`);

function waitText(pattern, timeout) {
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
  return errors.length ? errors : hooked;
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
step("Berichte", () => nav("Berichte"));
step("Turniere", async () => {
  await nav("Turniere");
  // The id sits in the name's tooltip; narrow windows hide the id column.
  if (TAURI) await find("main .gname[title*='#3500000001']");
});

step("Hände: Replayer öffnet eine Hand", async () => {
  await nav("Hände");
  await click("main [role='option']");
  await find(".replayer");
  await click(".replayer button[aria-label='Weiter']");
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
    await click("main .steps button", "Turn · Check");
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
step("Live-Tische", () => nav("Live-Tische"));
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
     const st = el.dataset.stacks.split(",").map(Number);
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

step("Solver: kleiner River-Spot", async () => {
  await nav("Solver");
  await click("main [role='tab']", "Aufbau");
  // Five board cards make it a river spot, which solves in a moment.
  await s.exec(`[...document.querySelectorAll("main button")].find((b) => b.textContent.trim() === "Board leeren")?.click()`);
  const slots = ["Flop", "Flop", "Flop", "Turn", "River"];
  for (const [i, card] of ["Ks", "7d", "2c", "3c", "4d"].entries()) {
    await click(`main .board button.slot[aria-label='${slots[i]}-Karte wählen']`);
    await click(`.deck-pop button.pick[aria-label='${card}']:not(:disabled)`);
  }
  await click("main button", "Ranges anpassen");
  await fill("main textarea[aria-label='Range von OOP als Text']", "AA,KK");
  await fill("main textarea[aria-label='Range von IP als Text']", "QQ,JJ");
  await click("main button.btn.primary:not(:disabled)", "Lösen");
  await find("main [role='group'][aria-label='Aktionen']", null, { timeout: 180_000 });
});

step("Solver: die Lösung steht in „Meine Lösungen“ und öffnet ohne Rechnen", async () => {
  await click("main [role='tab']", "Meine Lösungen");
  await click("main .saved li.card button.btn.primary");
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

step("Hände spielen: eine Hand startet", async () => {
  await click("nav button", "Hände spielen");
  await find("main h2", "Hände spielen");
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
    const now = await s.exec(
      `if (document.querySelector("main section.end")) return "end";
       if (document.querySelector("main [data-part='solving']")) return "solving";
       return document.querySelector("main .actions button.act") ? "decide" : "wait";`,
    );
    if (now === "end") break;
    if (now === "solving") {
      solving ??= Date.now();
      // A slow machine may take long for a live spot: then the hand goes on without it.
      if (TAURI && Date.now() - solving > 60_000) await click("main [data-part='solving'] button", "Abbrechen", { exact: false });
    } else if (now === "decide") {
      if (solving !== null) solved = true;
      const move = await s.exec(
        `const b = [...document.querySelectorAll("main .actions button.act")];
         return b.find((x) => /Call|Check/.test(x.textContent)) ?? b[0];`,
      );
      await s.click(move);
    }
    await sleep(250);
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

step("Coach: ohne Key erscheint der Hinweis", async () => {
  await nav("Coach");
  if (TAURI) await waitText("Für den Coach brauchst du einen API-Key");
  if (hits.OpenRouter) throw new Error(`${hits.OpenRouter} connection(s) to OpenRouter`);
});

step("Einstellungen: Cloud-Schalter aus", async () => {
  await nav("Einstellungen");
  const boxes = await waitFor(
    "the cloud switches",
    `const panel = [...document.querySelectorAll("main section.panel")]
       .find((p) => p.querySelector(".panel-title")?.textContent.trim() === "Ringside Cloud");
     const boxes = panel ? [...panel.querySelectorAll("input[type=checkbox]")].map((b) => b.checked) : [];
     return boxes.length ? boxes : null;`,
  );
  if (boxes.length !== 2) throw new Error(`expected 2 cloud switches, found ${boxes.length}`);
  if (TAURI && boxes.some(Boolean)) throw new Error("cloud switches are on");
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
