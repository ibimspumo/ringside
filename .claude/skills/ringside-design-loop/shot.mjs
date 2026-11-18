// One screenshot over the Chrome DevTools protocol: waits for fonts and two painted frames,
// then captures the full viewport. `chrome --screenshot` raced the compositor and left parts
// of some pictures unpainted.
//
//   node shot.mjs <chrome> <url> <width> <height> <out.png> [full]
// With `full` the window grows by what the scrolling content area hides, so the whole page is in
// one picture.
import { spawn } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const [chrome, url, width, height, out, full] = process.argv.slice(2);
const profile = mkdtempSync(join(tmpdir(), "ringside-shot-"));
const proc = spawn(
  chrome,
  [
    "--headless=new",
    "--disable-gpu",
    "--hide-scrollbars",
    "--force-prefers-reduced-motion",
    "--remote-debugging-port=0",
    `--user-data-dir=${profile}`,
    `--window-size=${width},${height}`,
    "about:blank",
  ],
  { stdio: ["ignore", "ignore", "pipe"] },
);

const wsUrl = await new Promise((resolve, reject) => {
  let buf = "";
  proc.stderr.on("data", (d) => {
    buf += d;
    const m = buf.match(/DevTools listening on (ws:\/\/\S+)/);
    if (m) resolve(m[1]);
  });
  proc.on("exit", () => reject(new Error("Chrome ended before DevTools was ready")));
});

const browser = new WebSocket(wsUrl);
await new Promise((r) => browser.addEventListener("open", r, { once: true }));
let id = 0;
const pending = new Map();
browser.addEventListener("message", (e) => {
  const msg = JSON.parse(e.data);
  if (msg.id && pending.has(msg.id)) {
    const { resolve, reject } = pending.get(msg.id);
    pending.delete(msg.id);
    msg.error ? reject(new Error(msg.error.message)) : resolve(msg.result);
  }
});
const send = (method, params = {}, sessionId) =>
  new Promise((resolve, reject) => {
    pending.set(++id, { resolve, reject });
    browser.send(JSON.stringify({ id, method, params, sessionId }));
  });

try {
  const { targetId } = await send("Target.createTarget", { url: "about:blank" });
  const { sessionId } = await send("Target.attachToTarget", { targetId, flatten: true });
  const page = (method, params) => send(method, params, sessionId);
  await page("Emulation.setDeviceMetricsOverride", { width: +width, height: +height, deviceScaleFactor: Number(process.env.SHOT_SCALE ?? 2), mobile: false });
  await page("Page.enable");
  await page("Runtime.enable");
  // The first visit compiles the app in Vite, so "load" comes long before anything is drawn:
  // wait until the page has content that stops changing, then for fonts and two frames.
  const ready = `new Promise((done) => {
    let last = -1, same = 0;
    const until = Date.now() + 20000;
    const poll = () => {
      const size = document.body ? document.body.innerHTML.length : 0;
      same = size > 0 && size === last && document.body.innerText.trim() ? same + 1 : 0;
      last = size;
      if (same >= 3 || Date.now() > until)
        document.fonts.ready.then(() => requestAnimationFrame(() => requestAnimationFrame(() => done(true))));
      else setTimeout(poll, 250);
    };
    poll();
  })`;
  // Vite may reload the page once while it optimizes dependencies on the first visits; the wait
  // then ends with "Inspected target navigated", and the page is loaded again.
  for (let attempt = 1; ; attempt++) {
    await page("Page.navigate", { url });
    try {
      // A reload can also swallow the answer entirely; give up on this attempt after 30 s.
      await Promise.race([
        page("Runtime.evaluate", { expression: ready, awaitPromise: true, timeout: 20000 }),
        new Promise((_, reject) => setTimeout(() => reject(new Error("no answer: navigated or closed")), 30000)),
      ]);
      break;
    } catch (e) {
      if (attempt >= 3 || !/navigated or closed|context was destroyed/i.test(e.message)) throw e;
    }
  }
  if (full === "full") {
    const { result } = await page("Runtime.evaluate", {
      expression: `(() => { const el = document.querySelector("main .content") ?? document.scrollingElement; return el.scrollHeight - el.clientHeight; })()`,
      returnByValue: true,
    });
    if (result.value > 0) {
      await page("Emulation.setDeviceMetricsOverride", { width: +width, height: +height + result.value, deviceScaleFactor: Number(process.env.SHOT_SCALE ?? 2), mobile: false });
      await page("Runtime.evaluate", {
        expression: "new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(() => setTimeout(done, 150))))",
        awaitPromise: true,
      });
    }
  }
  const { data } = await page("Page.captureScreenshot", { format: "png", captureBeyondViewport: false });
  writeFileSync(out, Buffer.from(data, "base64"));
} finally {
  browser.close();
  proc.kill();
  await new Promise((r) => proc.once("exit", r));
  rmSync(profile, { recursive: true, force: true });
}
