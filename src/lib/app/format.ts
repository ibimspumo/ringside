// Date, number and hover-card helpers for the app views. Hand history timestamps look like "2026/11/16 21:04:10".

const WEEKDAY = ["So", "Mo", "Di", "Mi", "Do", "Fr", "Sa"];

function parse(ts: string): Date {
  const [d, t = "00:00:00"] = ts.split(" ");
  const [y, m, day] = d.split("/").map(Number);
  const [h, min, s] = t.split(":").map(Number);
  return new Date(y, m - 1, day, h, min, s || 0);
}

/** "16.11.2026" */
export function date(ts: string): string {
  const [y, m, d] = ts.slice(0, 10).split("/");
  return `${d}.${m}.${y}`;
}

/** "Mo, 16.11." */
export function dayShort(ts: string): string {
  const [, m, d] = ts.slice(0, 10).split("/");
  return `${WEEKDAY[parse(ts).getDay()]}, ${d}.${m}.`;
}

/** "21:04" */
export function time(ts: string): string {
  return ts.slice(11, 16);
}

/** "16.11. 21:04" */
export function dateTime(ts: string): string {
  const [, m, d] = ts.slice(0, 10).split("/");
  return `${d}.${m}. ${time(ts)}`;
}

/** Seconds between two timestamps. */
export function secondsBetween(start: string, end: string): number {
  return Math.max(0, (parse(end).getTime() - parse(start).getTime()) / 1000);
}

export { int, signed } from "../ui/format";

/** CSS class for a result: "pos", "neg" or "". */
export function sign(n: number | null | undefined): string {
  if (!n) return "";
  return n > 0 ? "pos" : "neg";
}

export type Period = "today" | "7d" | "30d" | "year" | "all";

export const PERIODS: [Period, string][] = [
  ["today", "Heute"],
  ["7d", "7 Tage"],
  ["30d", "30 Tage"],
  ["year", "Dieses Jahr"],
  ["all", "Alles"],
];

/** First day included by a period, as "YYYY/MM/DD", or null for everything. */
export function since(period: Period, now = new Date()): string | null {
  const d = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  if (period === "all") return null;
  if (period === "7d") d.setDate(d.getDate() - 6);
  if (period === "30d") d.setDate(d.getDate() - 29);
  if (period === "year") d.setMonth(0, 1);
  return `${d.getFullYear()}/${String(d.getMonth() + 1).padStart(2, "0")}/${String(d.getDate()).padStart(2, "0")}`;
}

export { floating, type Anchor, type FloatOptions } from "../ui/floating";

/** GGPoker ids live from 2⁵² up, so they never meet PokerStars ids (`Site::of_id` in ringside-core). */
const GG_ID_BASE = 2 ** 52;
/** Room per two-letter GGPoker hand prefix ("TM", "HD", "RC", …). */
const GG_PREFIX_SPAN = 6e12;

export function siteOfId(id: number): "pokerstars" | "ggpoker" {
  return id >= GG_ID_BASE ? "ggpoker" : "pokerstars";
}

/** A hand number as the client writes it: "260012345678" or "TM5216773942". */
export function handLabel(id: number): string {
  if (id < GG_ID_BASE) return String(id);
  const rest = id - GG_ID_BASE;
  const code = Math.floor(rest / GG_PREFIX_SPAN);
  return String.fromCharCode(65 + Math.floor(code / 26), 65 + (code % 26)) + String(rest % GG_PREFIX_SPAN);
}

/** A tournament number as the client writes it. */
export function tournamentLabel(id: number): string {
  return String(id >= GG_ID_BASE ? id - GG_ID_BASE : id);
}
