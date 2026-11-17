// Date and number helpers for the app views. Hand history timestamps look like "2026/11/16 21:04:10".

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

/** "3:25 h" between two timestamps. */
export function duration(start: string, end: string): string {
  const mins = Math.max(0, Math.round((parse(end).getTime() - parse(start).getTime()) / 60000));
  return `${Math.floor(mins / 60)}:${String(mins % 60).padStart(2, "0")} h`;
}

/** German integer with thousands dots. */
export function int(n: number): string {
  return Math.round(n).toLocaleString("de-DE");
}

/** Signed decimal: "+12,4", "−3,0", "0,0". */
export function signed(n: number, digits = 1): string {
  const abs = Math.abs(n).toLocaleString("de-DE", { minimumFractionDigits: digits, maximumFractionDigits: digits });
  if (Math.abs(n) < 0.5 / 10 ** digits) return abs;
  return `${n > 0 ? "+" : "−"}${abs}`;
}

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
