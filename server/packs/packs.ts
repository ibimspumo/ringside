/**
 * Signed download packs on Bunny Storage, delivered through the pull zone ringside-packs.
 *
 * Layout in the storage zone:
 *   packs/<name>/<version>/<files…>   immutable once uploaded
 *   index.json                        every pack version with each file's size and SHA-256
 *   index.json.sig                    base64 Ed25519 signature over the exact bytes of index.json
 *
 * A client fetches index.json and its signature, verifies them with the public key built
 * into the app, rejects an index whose serial is lower than one it has seen, then checks
 * each downloaded file against its SHA-256 from the index.
 */
import { assertOurs, Bunny, type Named } from "../lib/bunny.ts";
import { ENV_PATH, loadEnv, need } from "../lib/env.ts";
import { fromBase64, toBase64 } from "../lib/sql.ts";

const HELP = `Usage: deno task packs <command>

  setup                        create storage zone and pull zone ringside-packs (DE) if missing
  keygen                       create the Ed25519 signing key in .env (RINGSIDE_PACKS_SIGNING_KEY)
                               unless it exists; prints the public key
  pubkey                       print the public key for the app
  upload <name> <version> <dir>  upload all files below <dir> as packs/<name>/<version>/,
                               add them to index.json and sign it (versions are immutable)
  remove <name> <version>      delete a pack version and re-sign the index
  verify                       download index.json via the CDN, check the signature and every
                               file's size and SHA-256

Reads BUNNY_API_KEY and RINGSIDE_PACKS_SIGNING_KEY from .env. Only one upload at a time.`;

const ZONE = "ringside-packs";
const INDEX = "index.json";
const FORMAT = "ringside-packs-index/1";
const NAME_RE = /^[a-z0-9][a-z0-9-]{0,63}$/;
const VERSION_RE = /^\d{1,6}\.\d{1,6}\.\d{1,6}$/;
const PATH_RE = /^[A-Za-z0-9_][A-Za-z0-9._-]*(\/[A-Za-z0-9_][A-Za-z0-9._-]*)*$/;

export interface PackFile {
  path: string;
  size: number;
  sha256: string;
}

export interface Index {
  format: string;
  serial: number;
  updated_at: string;
  packs: Record<string, Record<string, { uploaded_at: string; files: PackFile[] }>>;
}

const emptyIndex = (): Index => ({ format: FORMAT, serial: 0, updated_at: "", packs: {} });

async function sha256Hex(bytes: Uint8Array<ArrayBuffer>): Promise<string> {
  const d = await crypto.subtle.digest("SHA-256", bytes);
  return [...new Uint8Array(d)].map((b) => b.toString(16).padStart(2, "0")).join("");
}

// --- signing ---------------------------------------------------------------------------

export async function generateKeyPair(): Promise<{ privateKey: string; publicKey: string }> {
  const pair = await crypto.subtle.generateKey({ name: "Ed25519" }, true, ["sign", "verify"]) as CryptoKeyPair;
  return {
    privateKey: toBase64(new Uint8Array(await crypto.subtle.exportKey("pkcs8", pair.privateKey))),
    publicKey: toBase64(new Uint8Array(await crypto.subtle.exportKey("raw", pair.publicKey))),
  };
}

async function importPrivate(pkcs8: string): Promise<CryptoKey> {
  return await crypto.subtle.importKey("pkcs8", fromBase64(pkcs8) as Uint8Array<ArrayBuffer>, { name: "Ed25519" }, true, [
    "sign",
  ]);
}

export async function publicKeyOf(pkcs8: string): Promise<string> {
  const jwk = await crypto.subtle.exportKey("jwk", await importPrivate(pkcs8));
  const raw = atob(jwk.x!.replace(/-/g, "+").replace(/_/g, "/") + "=");
  return btoa(raw);
}

export async function sign(pkcs8: string, data: Uint8Array<ArrayBuffer>): Promise<string> {
  return toBase64(new Uint8Array(await crypto.subtle.sign("Ed25519", await importPrivate(pkcs8), data)));
}

export async function verifySignature(publicKey: string, data: Uint8Array<ArrayBuffer>, sig: string): Promise<boolean> {
  const key = await crypto.subtle.importKey(
    "raw",
    fromBase64(publicKey) as Uint8Array<ArrayBuffer>,
    { name: "Ed25519" },
    false,
    ["verify"],
  );
  return await crypto.subtle.verify("Ed25519", key, fromBase64(sig.trim()) as Uint8Array<ArrayBuffer>, data);
}

// --- index -----------------------------------------------------------------------------

export function addVersion(index: Index, name: string, version: string, files: PackFile[], now: Date): Index {
  if (!NAME_RE.test(name)) throw new Error(`pack name must match ${NAME_RE}`);
  if (!VERSION_RE.test(version)) throw new Error("version must look like 1.2.3");
  if (index.packs[name]?.[version]) throw new Error(`${name} ${version} exists; pack versions are immutable`);
  const at = now.toISOString();
  return {
    format: FORMAT,
    serial: index.serial + 1,
    updated_at: at,
    packs: { ...index.packs, [name]: { ...index.packs[name], [version]: { uploaded_at: at, files } } },
  };
}

export function removeVersion(index: Index, name: string, version: string, now: Date): Index {
  if (!index.packs[name]?.[version]) throw new Error(`${name} ${version} is not in the index`);
  const versions = { ...index.packs[name] };
  delete versions[version];
  const packs = { ...index.packs, [name]: versions };
  if (Object.keys(versions).length === 0) delete packs[name];
  return { format: FORMAT, serial: index.serial + 1, updated_at: now.toISOString(), packs };
}

// --- bunny -----------------------------------------------------------------------------

class Storage {
  private constructor(
    private readonly zone: Named,
    private readonly password: string,
    private readonly host: string,
  ) {}

  static async open(bunny: Bunny): Promise<Storage> {
    const found = Bunny.find(await bunny.storageZones(), ZONE);
    if (!found) throw new Error(`storage zone ${ZONE} missing; run "deno task packs setup"`);
    const zone = await bunny.request("GET", `/storagezone/${found.Id}`) as Named;
    assertOurs(zone.Name);
    return new Storage(zone, String(zone.Password), String(zone.StorageHostname || "storage.bunnycdn.com"));
  }

  private url(path: string): string {
    return `https://${this.host}/${assertOurs(this.zone.Name)}/${path}`;
  }

  async get(path: string): Promise<Uint8Array<ArrayBuffer> | null> {
    const res = await fetch(this.url(path), { headers: { AccessKey: this.password } });
    if (res.status === 404) {
      await res.body?.cancel();
      return null;
    }
    if (!res.ok) throw new Error(`GET ${path}: HTTP ${res.status}`);
    return new Uint8Array(await res.arrayBuffer());
  }

  async put(path: string, data: Uint8Array<ArrayBuffer>, contentType = "application/octet-stream"): Promise<void> {
    const res = await fetch(this.url(path), {
      method: "PUT",
      headers: {
        AccessKey: this.password,
        "Content-Type": contentType,
        Checksum: (await sha256Hex(data)).toUpperCase(),
      },
      body: data,
    });
    if (!res.ok) throw new Error(`PUT ${path}: HTTP ${res.status} ${(await res.text()).slice(0, 200)}`);
    await res.body?.cancel();
  }

  async deleteDir(path: string): Promise<void> {
    const res = await fetch(this.url(path.endsWith("/") ? path : path + "/"), {
      method: "DELETE",
      headers: { AccessKey: this.password },
    });
    if (!res.ok && res.status !== 404) throw new Error(`DELETE ${path}: HTTP ${res.status}`);
    await res.body?.cancel();
  }
}

async function pullZone(bunny: Bunny): Promise<Named> {
  const zone = Bunny.find(await bunny.pullZones(), ZONE);
  if (!zone) throw new Error(`pull zone ${ZONE} missing; run "deno task packs setup"`);
  return zone;
}

function cdnBase(zone: Named): string {
  const hosts = (zone.Hostnames as { Value: string; IsSystemHostname: boolean }[]) ?? [];
  const host = hosts.find((h) => h.IsSystemHostname)?.Value ?? `${ZONE}.b-cdn.net`;
  return `https://${host}`;
}

async function readIndex(storage: Storage): Promise<Index> {
  const bytes = await storage.get(INDEX);
  if (!bytes) return emptyIndex();
  const index = JSON.parse(new TextDecoder().decode(bytes)) as Index;
  if (index.format !== FORMAT) throw new Error(`unknown index format ${index.format}`);
  return index;
}

async function writeIndex(bunny: Bunny, storage: Storage, index: Index, key: string): Promise<void> {
  const bytes = new TextEncoder().encode(JSON.stringify(index, null, 2) + "\n");
  // Signature first: a client that sees the new index next to the old signature fails
  // closed and retries, never trusts an unsigned index.
  await storage.put(`${INDEX}.sig`, new TextEncoder().encode(await sign(key, bytes) + "\n"), "text/plain");
  await storage.put(INDEX, bytes, "application/json");
  const zone = await pullZone(bunny);
  await bunny.owned(zone, "POST", `/pullzone/${zone.Id}/purgeCache`, {});
}

async function listFiles(dir: string, prefix = ""): Promise<string[]> {
  const out: string[] = [];
  for await (const e of Deno.readDir(dir)) {
    if (e.name.startsWith(".")) continue;
    const rel = prefix ? `${prefix}/${e.name}` : e.name;
    if (e.isDirectory) out.push(...await listFiles(`${dir}/${e.name}`, rel));
    else if (e.isFile) out.push(rel);
  }
  return out.sort();
}

// --- commands --------------------------------------------------------------------------

async function setup(bunny: Bunny) {
  let storage = Bunny.find(await bunny.storageZones(), ZONE);
  if (!storage) {
    storage = await bunny.create(ZONE, "/storagezone", { Name: ZONE, Region: "DE", ZoneTier: 0 }) as Named;
    console.log(`created storage zone ${ZONE} (id ${storage.Id}, region DE, standard tier)`);
  } else console.log(`storage zone ${ZONE} exists (id ${storage.Id})`);

  let zone = Bunny.find(await bunny.pullZones(), ZONE);
  if (!zone) {
    zone = await bunny.create(ZONE, "/pullzone", {
      Name: ZONE,
      OriginType: 2,
      StorageZoneId: storage.Id,
      Type: 0,
    }) as Named;
    console.log(`created pull zone ${ZONE} (id ${zone.Id})`);
  } else console.log(`pull zone ${ZONE} exists (id ${zone.Id})`);
  // index.json is JSON, which Smart Cache never caches; every index update purges the zone.
  zone = await bunny.request("GET", `/pullzone/${zone.Id}`) as Named;
  if (zone.EnableSmartCache !== false) {
    await bunny.owned(zone, "POST", `/pullzone/${zone.Id}`, { EnableSmartCache: false });
  }
  console.log(`CDN: ${cdnBase(zone)}`);
}

async function keygen() {
  const env = loadEnv();
  if (env.RINGSIDE_PACKS_SIGNING_KEY) {
    console.log("RINGSIDE_PACKS_SIGNING_KEY exists, keeping it");
    return console.log(`public key: ${await publicKeyOf(env.RINGSIDE_PACKS_SIGNING_KEY)}`);
  }
  const { privateKey, publicKey } = await generateKeyPair();
  const text = await Deno.readTextFile(ENV_PATH);
  const sep = text.endsWith("\n") || text === "" ? "" : "\n";
  await Deno.writeTextFile(ENV_PATH, `${text}${sep}RINGSIDE_PACKS_SIGNING_KEY=${privateKey}\n`, { mode: 0o600 });
  await Deno.chmod(ENV_PATH, 0o600);
  console.log("wrote RINGSIDE_PACKS_SIGNING_KEY to .env");
  console.log(`public key: ${publicKey}`);
}

async function upload(bunny: Bunny, name: string, version: string, dir: string) {
  const key = need(loadEnv(), "RINGSIDE_PACKS_SIGNING_KEY");
  const storage = await Storage.open(bunny);
  const index = await readIndex(storage);
  const paths = await listFiles(dir);
  if (paths.length === 0) throw new Error(`${dir} has no files`);
  const files: PackFile[] = [];
  for (const path of paths) {
    if (!PATH_RE.test(path)) throw new Error(`file name not allowed: ${path}`);
    const data = await Deno.readFile(`${dir}/${path}`) as Uint8Array<ArrayBuffer>;
    files.push({ path, size: data.length, sha256: await sha256Hex(data) });
  }
  const next = addVersion(index, name, version, files, new Date());
  for (const f of files) {
    await storage.put(`packs/${name}/${version}/${f.path}`, await Deno.readFile(`${dir}/${f.path}`) as Uint8Array<ArrayBuffer>);
    console.log(`uploaded packs/${name}/${version}/${f.path} (${f.size} bytes)`);
  }
  await writeIndex(bunny, storage, next, key);
  console.log(`index serial ${next.serial} signed and uploaded`);
}

async function remove(bunny: Bunny, name: string, version: string) {
  const key = need(loadEnv(), "RINGSIDE_PACKS_SIGNING_KEY");
  const storage = await Storage.open(bunny);
  const next = removeVersion(await readIndex(storage), name, version, new Date());
  await writeIndex(bunny, storage, next, key);
  await storage.deleteDir(`packs/${name}/${version}`);
  console.log(`removed ${name} ${version}; index serial ${next.serial}`);
}

async function verify(bunny: Bunny) {
  const publicKey = await publicKeyOf(need(loadEnv(), "RINGSIDE_PACKS_SIGNING_KEY"));
  const base = cdnBase(await pullZone(bunny));
  const fetchOk = async (path: string) => {
    const res = await fetch(`${base}/${path}`);
    if (!res.ok) throw new Error(`${path}: HTTP ${res.status}`);
    return new Uint8Array(await res.arrayBuffer());
  };
  const bytes = await fetchOk(INDEX);
  const sig = new TextDecoder().decode(await fetchOk(`${INDEX}.sig`));
  if (!await verifySignature(publicKey, bytes, sig)) throw new Error("index signature INVALID");
  const index = JSON.parse(new TextDecoder().decode(bytes)) as Index;
  console.log(`index serial ${index.serial}: signature valid`);
  for (const [name, versions] of Object.entries(index.packs)) {
    for (const [version, { files }] of Object.entries(versions)) {
      for (const f of files) {
        const data = await fetchOk(`packs/${name}/${version}/${f.path}`);
        if (data.length !== f.size || await sha256Hex(data) !== f.sha256) {
          throw new Error(`${name} ${version} ${f.path}: size or SHA-256 differs`);
        }
      }
      console.log(`${name} ${version}: ${files.length} file(s) match`);
    }
  }
}

async function main() {
  const [cmd, ...rest] = Deno.args;
  if (!cmd || cmd === "--help" || cmd === "-h") return console.log(HELP);
  if (cmd === "keygen") return await keygen();
  if (cmd === "pubkey") return console.log(await publicKeyOf(need(loadEnv(), "RINGSIDE_PACKS_SIGNING_KEY")));
  const bunny = new Bunny(need(loadEnv(), "BUNNY_API_KEY"));
  if (cmd === "setup") return await setup(bunny);
  if (cmd === "upload" && rest.length === 3) return await upload(bunny, rest[0], rest[1], rest[2]);
  if (cmd === "remove" && rest.length === 2) return await remove(bunny, rest[0], rest[1]);
  if (cmd === "verify") return await verify(bunny);
  throw new Error(`unknown command or arguments; see --help`);
}

if (import.meta.main) await main();
