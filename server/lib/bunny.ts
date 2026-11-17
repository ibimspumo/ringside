/**
 * bunny.net account API. The key belongs to an account that also hosts unrelated projects,
 * so every write goes through `owned()`: only resources named `ringside-*` may be created,
 * changed or deleted.
 */

export const PREFIX = "ringside-";
const API = "https://api.bunny.net";

export function assertOurs(name: unknown): string {
  if (typeof name !== "string" || !/^ringside-[a-z0-9-]+$/.test(name)) {
    throw new Error(`refusing to touch "${String(name)}": only ringside-* resources are ours`);
  }
  return name;
}

export interface Named {
  Id: number;
  Name: string;
  [k: string]: unknown;
}

export class Bunny {
  constructor(private readonly key: string) {}

  async request(method: string, path: string, body?: unknown): Promise<any> {
    const res = await fetch(API + path, {
      method,
      headers: {
        AccessKey: this.key,
        Accept: "application/json",
        ...(body === undefined ? {} : { "Content-Type": "application/json" }),
      },
      body: body === undefined ? undefined : JSON.stringify(body),
      signal: AbortSignal.timeout(30_000),
    });
    const text = await res.text();
    if (!res.ok) throw new Error(`${method} ${path}: HTTP ${res.status} ${text.slice(0, 300)}`);
    return text ? JSON.parse(text) : null;
  }

  /** A write against an existing resource, after checking its name. */
  owned(resource: Named, method: string, path: string, body?: unknown): Promise<any> {
    assertOurs(resource.Name);
    return this.request(method, path, body);
  }

  /** A create call whose body names the new resource. */
  create(name: string, path: string, body: Record<string, unknown>): Promise<any> {
    assertOurs(name);
    return this.request("POST", path, body);
  }

  async scripts(): Promise<Named[]> {
    const out: Named[] = [];
    for (let page = 1; ; page++) {
      const r = await this.request("GET", `/compute/script?page=${page}&perPage=1000`);
      out.push(...r.Items);
      if (!r.HasMoreItems) return out;
    }
  }

  async pullZones(): Promise<Named[]> {
    return await this.request("GET", "/pullzone?perPage=1000");
  }

  async storageZones(): Promise<Named[]> {
    return await this.request("GET", "/storagezone?perPage=1000");
  }

  static find(list: Named[], name: string): Named | undefined {
    return list.find((r) => r.Name === assertOurs(name) && !r.Deleted);
  }
}
