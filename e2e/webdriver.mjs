// A minimal W3C WebDriver client: the handful of calls the end-to-end run needs, without
// pulling a test framework into the app's dependencies.

const ELEMENT = "element-6066-11e4-a52e-4f735466cecf";

export class Session {
  /** @param {string} server @param {string} id */
  constructor(server, id) {
    this.base = `${server}/session/${id}`;
  }

  /** @param {string} server @param {Record<string, unknown>} capabilities */
  static async start(server, capabilities) {
    const value = await request("POST", `${server}/session`, { capabilities: { alwaysMatch: capabilities } });
    return new Session(server, value.sessionId);
  }

  /** @param {string} method @param {string} path @param {unknown} [body] */
  cmd(method, path, body) {
    return request(method, `${this.base}${path}`, body);
  }

  /** Runs `script` in the page; `args` arrive as `arguments`. Returned elements become references. */
  exec(script, ...args) {
    return this.cmd("POST", "/execute/sync", { script, args });
  }

  /** Like `exec`, but `script` gets a callback as its last argument and is awaited. */
  execAsync(script, ...args) {
    return this.cmd("POST", "/execute/async", { script, args });
  }

  /** @param {{ [ELEMENT]: string }} element */
  click(element) {
    return this.cmd("POST", `/element/${element[ELEMENT]}/click`, {});
  }

  /** @param {{ [ELEMENT]: string }} element @param {string} text */
  async type(element, text) {
    await this.cmd("POST", `/element/${element[ELEMENT]}/clear`, {});
    return this.cmd("POST", `/element/${element[ELEMENT]}/value`, { text });
  }

  navigate(url) {
    return this.cmd("POST", "/url", { url });
  }

  /** @returns {Promise<string>} PNG as base64 */
  screenshot() {
    return this.cmd("GET", "/screenshot");
  }

  /** Browser console entries since the last call, where the driver supports the log endpoint. */
  async consoleLog() {
    try {
      return await this.cmd("POST", "/se/log", { type: "browser" });
    } catch {
      return null;
    }
  }

  quit() {
    return request("DELETE", this.base);
  }
}

/** @param {string} method @param {string} url @param {unknown} [body] */
async function request(method, url, body) {
  const res = await fetch(url, {
    method,
    headers: { "content-type": "application/json; charset=utf-8" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const json = await res.json().catch(() => ({ value: null }));
  if (!res.ok) {
    const v = json.value ?? {};
    throw new Error(`${method} ${url}: ${v.error ?? res.status} ${v.message ?? ""}`.trim());
  }
  return json.value;
}
