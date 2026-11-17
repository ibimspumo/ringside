/**
 * Canonical JSON for solution inputs, version 1. The Rust client implements the same rules
 * (crates/ringside-cloud/src/canonical.rs); crates/ringside-cloud/testdata/canonical.json
 * holds vectors both test suites check.
 *
 * Only values with exactly one spelling are allowed, so no float formatting differences
 * between JavaScript and Rust can change a key:
 * - integers with |n| ≤ 2^53 − 1 (inputs use integer units such as milli-BB)
 * - strings of printable ASCII, escaping only `"` and `\`
 * - booleans, null, arrays
 * - objects with keys matching [a-z0-9_]{1,64}, written in byte order
 * No whitespace, nesting depth at most 8.
 */

export class CanonicalError extends Error {}

const KEY = /^[a-z0-9_]{1,64}$/;
const PRINTABLE = /^[\x20-\x7e]*$/;
const MAX_DEPTH = 8;

export function canonicalize(value: unknown, depth = 0): string {
  if (depth > MAX_DEPTH) throw new CanonicalError("nested too deeply");
  if (value === null) return "null";
  switch (typeof value) {
    case "boolean":
      return value ? "true" : "false";
    case "number":
      if (!Number.isSafeInteger(value)) throw new CanonicalError("numbers must be integers within ±(2^53−1)");
      return String(value === 0 ? 0 : value);
    case "string":
      if (!PRINTABLE.test(value)) throw new CanonicalError("strings must be printable ASCII");
      return `"${value.replace(/[\\"]/g, (c) => "\\" + c)}"`;
    case "object": {
      if (Array.isArray(value)) return `[${value.map((v) => canonicalize(v, depth + 1)).join(",")}]`;
      const keys = Object.keys(value as object).sort();
      for (const k of keys) if (!KEY.test(k)) throw new CanonicalError(`invalid key "${k.slice(0, 70)}"`);
      const rec = value as Record<string, unknown>;
      return `{${keys.map((k) => `"${k}":${canonicalize(rec[k], depth + 1)}`).join(",")}}`;
    }
    default:
      throw new CanonicalError(`unsupported type ${typeof value}`);
  }
}

export async function solutionKey(kind: string, engineVersion: number, canonicalInput: string): Promise<string> {
  const msg = `ringside-solution/1\n${kind}\n${engineVersion}\n${canonicalInput}`;
  const d = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(msg));
  return [...new Uint8Array(d)].map((b) => b.toString(16).padStart(2, "0")).join("");
}
