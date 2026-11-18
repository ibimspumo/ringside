/**
 * Fixed-window rate limits. Writes count in the database, so the limit holds across all
 * edge locations; reads only count per isolate in memory, because a database write per
 * cache miss would cost more than the read it protects (the pull zone's per-IP request
 * limit backs them up).
 */
import type { Stmt } from "../lib/sql.ts";

export interface Rule {
  name: string;
  scope: "client" | "global";
  windowS: number;
  max: number;
}

export const RULES = {
  submit: [
    { name: "submit-min", scope: "client", windowS: 60, max: 30 },
    { name: "submit-hour", scope: "client", windowS: 3600, max: 600 },
    { name: "submit-global", scope: "global", windowS: 3600, max: 3000 },
  ],
  report: [
    { name: "report-hour", scope: "client", windowS: 3600, max: 10 },
    { name: "report-global", scope: "global", windowS: 3600, max: 300 },
  ],
  read: [{ name: "read-min", scope: "client", windowS: 60, max: 300 }],
  status: [{ name: "status-min", scope: "client", windowS: 60, max: 20 }],
  flops: [{ name: "flops-min", scope: "client", windowS: 60, max: 20 }],
  work: [
    { name: "work-min", scope: "client", windowS: 60, max: 30 },
    { name: "work-hour", scope: "client", windowS: 3600, max: 600 },
    { name: "work-global", scope: "global", windowS: 3600, max: 3000 },
  ],
  release: [{ name: "release-hour", scope: "client", windowS: 3600, max: 600 }],
  request: [
    { name: "request-hour", scope: "client", windowS: 3600, max: 30 },
    { name: "request-global", scope: "global", windowS: 3600, max: 1500 },
  ],
} as const satisfies Record<string, readonly Rule[]>;

/** Counters older than this are deleted; longer than the longest window. */
export const KEEP_S = 2 * 86_400;

export function windowStart(now: number, windowS: number): number {
  return Math.floor(now / windowS) * windowS;
}

export function bucket(rule: Rule, client: string): string {
  return `${rule.name}:${rule.scope === "client" ? client : "global"}`;
}

export type Decision = { ok: true } | { ok: false; retryAfter: number; rule: string };

/** Over the limit if any count exceeds its rule; retry once the latest such window ends. */
export function decide(hits: { rule: Rule; count: number }[], now: number): Decision {
  let worst: Decision = { ok: true };
  for (const { rule, count } of hits) {
    if (count <= rule.max) continue;
    const retryAfter = Math.max(1, Math.ceil(windowStart(now, rule.windowS) + rule.windowS - now));
    if (worst.ok || retryAfter > worst.retryAfter) worst = { ok: false, retryAfter, rule: rule.name };
  }
  return worst;
}

/** Statements that count one hit per rule and return the new count each. */
export function hitStatements(rules: readonly Rule[], client: string, now: number): Stmt[] {
  return rules.map((rule) => ({
    sql:
      "INSERT INTO rate_limits (bucket, window_start, count) VALUES (?, ?, 1) " +
      "ON CONFLICT (bucket, window_start) DO UPDATE SET count = count + 1 RETURNING count",
    args: [bucket(rule, client), windowStart(now, rule.windowS)],
  }));
}

export function pruneStatement(now: number): Stmt {
  return { sql: "DELETE FROM rate_limits WHERE window_start < ?", args: [now - KEEP_S] };
}

/** Per-isolate counters for rules that are not worth a database write. */
export class MemoryCounter {
  private counts = new Map<string, number>();

  constructor(private readonly maxEntries = 50_000) {}

  hit(rule: Rule, client: string, now: number): number {
    const key = `${bucket(rule, client)}@${windowStart(now, rule.windowS)}`;
    if (this.counts.size >= this.maxEntries && !this.counts.has(key)) this.counts.clear();
    const n = (this.counts.get(key) ?? 0) + 1;
    this.counts.set(key, n);
    return n;
  }
}
