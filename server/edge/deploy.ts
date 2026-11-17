/**
 * Bundles edge/main.ts and deploys it as the standalone Edge Script `ringside-api`:
 * creates the script and its pull zone on first run, uploads code, sets secrets and the
 * pull zone's per-IP limits, then publishes. Safe to run again; touches only ringside-*.
 */
import { assertOurs, Bunny, type Named } from "../lib/bunny.ts";
import { loadEnv, need } from "../lib/env.ts";
import { API_VERSION } from "./api.ts";

const HELP = `Usage: deno task deploy [--entry FILE]

Bundles server/edge/main.ts and deploys it as the Edge Script "ringside-api" with its
pull zone "ringside-api". Reads BUNNY_API_KEY, BUNNY_DATABASE_URL and
BUNNY_DATABASE_AUTH_TOKEN from .env. Run "deno task migrate" first after schema changes.

  --entry FILE  deploy another entry file instead (for diagnostics)
  --help        this text`;

const NAME = "ringside-api";
const STANDALONE = 1;
/**
 * Per-IP limits the CDN enforces before the script runs (requests per second, burst).
 * Smart Cache would never cache JSON; the script sets Cache-Control on every response
 * (no-store except verified solutions), so the CDN can follow it.
 */
const PULL_ZONE_SETTINGS = { RequestLimit: 10, BurstSize: 20, CacheErrorResponses: false, EnableSmartCache: false };

async function bundle(entry: string): Promise<string> {
  const out = new URL("../../target/ringside-edge/main.js", import.meta.url).pathname;
  await Deno.mkdir(new URL("../../target/ringside-edge/", import.meta.url), { recursive: true });
  const { code, stderr } = await new Deno.Command("deno", {
    args: ["bundle", "--quiet", "--platform", "deno", entry, "-o", out],
    stdout: "null",
    stderr: "piped",
  }).output();
  if (code !== 0) throw new Error(`deno bundle failed:\n${new TextDecoder().decode(stderr)}`);
  return await Deno.readTextFile(out);
}

function randomHex(bytes: number): string {
  return [...crypto.getRandomValues(new Uint8Array(bytes))].map((b) => b.toString(16).padStart(2, "0")).join("");
}

async function main() {
  const args = [...Deno.args];
  if (args.includes("--help") || args.includes("-h")) return console.log(HELP);
  const entryAt = args.indexOf("--entry");
  const entry = entryAt >= 0 ? args[entryAt + 1] : new URL("./main.ts", import.meta.url).pathname;

  const env = loadEnv();
  const bunny = new Bunny(need(env, "BUNNY_API_KEY"));
  const secrets = {
    BUNNY_DATABASE_URL: need(env, "BUNNY_DATABASE_URL"),
    BUNNY_DATABASE_AUTH_TOKEN: need(env, "BUNNY_DATABASE_AUTH_TOKEN"),
  };

  const code = await bundle(entry);
  console.log(`bundled ${entry} (${code.length} bytes)`);

  let script: Named | undefined = Bunny.find(await bunny.scripts(), NAME);
  if (!script) {
    script = await bunny.create(NAME, "/compute/script", {
      Name: NAME,
      Code: code,
      ScriptType: STANDALONE,
      CreateLinkedPullZone: true,
      LinkedPullZoneName: NAME,
    }) as Named;
    console.log(`created script ${NAME} (id ${script.Id})`);
  } else {
    await bunny.owned(script, "POST", `/compute/script/${script.Id}/code`, { Code: code });
    console.log(`uploaded code to ${NAME} (id ${script.Id})`);
  }
  script = await bunny.request("GET", `/compute/script/${script.Id}`) as Named;
  assertOurs(script.Name);

  const existing = (await bunny.owned(script, "GET", `/compute/script/${script.Id}/secrets`)).Secrets as Named[];
  const has = (n: string) => existing.some((s) => s.Name === n);
  for (const [name, value] of Object.entries(secrets)) {
    await bunny.owned(script, "PUT", `/compute/script/${script.Id}/secrets`, { Name: name, Secret: value });
  }
  if (!has("RINGSIDE_IP_SALT")) {
    // Created once; replacing it later only resets rate-limit counters and submitter ids.
    await bunny.owned(script, "PUT", `/compute/script/${script.Id}/secrets`, {
      Name: "RINGSIDE_IP_SALT",
      Secret: randomHex(32),
    });
  }
  console.log(`secrets set: ${[...Object.keys(secrets), "RINGSIDE_IP_SALT"].join(", ")}`);

  const linked = (script.LinkedPullZones as { Id: number; PullZoneName: string; DefaultHostname: string }[]) ?? [];
  const link = linked.find((z) => z.PullZoneName === NAME);
  if (!link) throw new Error(`pull zone ${NAME} is not linked to the script`);
  const zone = await bunny.request("GET", `/pullzone/${link.Id}`) as Named;
  const stale = Object.entries(PULL_ZONE_SETTINGS).filter(([k, v]) => zone[k] !== v);
  if (stale.length > 0) {
    await bunny.owned(zone, "POST", `/pullzone/${zone.Id}`, PULL_ZONE_SETTINGS);
    console.log(`pull zone ${zone.Name}: set ${stale.map(([k]) => k).join(", ")}`);
  }

  await bunny.owned(script, "POST", `/compute/script/${script.Id}/publish`, { Note: "deploy.ts" });
  const url = `https://${link.DefaultHostname.replace(/^https?:\/\//, "")}`;
  console.log(`published ${NAME}: ${url}`);

  // A new release takes a few seconds to reach every edge; until then the old one answers.
  for (let i = 0; i < 40; i++) {
    const res = await fetch(`${url}/v1/health`).catch(() => null);
    const body = res?.ok ? await res.json().catch(() => null) : null;
    if (body?.status === "ok" && body.api === API_VERSION) return console.log(`health: ${JSON.stringify(body)}`);
    await new Promise((r) => setTimeout(r, 3000));
  }
  throw new Error("the new release did not answer the health check within two minutes");
}

if (import.meta.main) await main();
