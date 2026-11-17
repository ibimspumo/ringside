/** Configuration from the repo's git-ignored `.env`; real environment variables win. */
export type Env = Record<string, string>;

export const ENV_PATH = new URL("../../.env", import.meta.url);

export function loadEnv(path: URL = ENV_PATH): Env {
  const env: Env = {};
  let text = "";
  try {
    text = Deno.readTextFileSync(path);
  } catch (e) {
    if (!(e instanceof Deno.errors.NotFound)) throw e;
  }
  for (const line of text.split("\n")) {
    const m = line.match(/^\s*([A-Z0-9_]+)\s*=\s*(.*?)\s*$/);
    if (!m) continue;
    env[m[1]] = m[2].replace(/^(['"])(.*)\1$/, "$2");
  }
  for (const key of Object.keys(env)) {
    const own = Deno.env.get(key);
    if (own) env[key] = own;
  }
  return env;
}

/** The value, or an error that names the variable but never shows a value. */
export function need(env: Env, name: string): string {
  const v = env[name] ?? Deno.env.get(name);
  if (!v) throw new Error(`${name} is not set (expected in .env or the environment)`);
  return v;
}
