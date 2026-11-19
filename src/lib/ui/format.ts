// Number formats for every view: German digits, a real minus sign, a thin gap before units.
// Amounts of money are stored in hundredths.

const MINUS = "−";

/** Decimal with fixed digits: "1,5". */
export function num(n: number, digits = 1): string {
  return n.toLocaleString("de-DE", { minimumFractionDigits: digits, maximumFractionDigits: digits });
}

/** German integer with thousands dots: "12.480". */
export function int(n: number): string {
  return Math.round(n).toLocaleString("de-DE");
}

/** Signed decimal: "+12,4", "−3,0"; values that round to zero have no sign. */
export function signed(n: number, digits = 1): string {
  const abs = num(Math.abs(n), digits);
  if (Math.abs(n) < 0.5 / 10 ** digits) return abs;
  return `${n > 0 ? "+" : MINUS}${abs}`;
}

/** Big blinds: "1,5 BB"; with `sign`, "+1,5 BB" / "−1,5 BB". */
export function bb(n: number, { digits = 1, sign = false }: { digits?: number; sign?: boolean } = {}): string {
  return `${sign ? signed(n, digits) : num(n, digits).replace("-", MINUS)} BB`;
}

/** Share of a whole given as 0–1: "12 %". */
export function pct(fraction: number, digits = 0): string {
  return `${num(fraction * 100, digits)} %`;
}

/** "1,2k", "12k" and "1,2M" for tight spaces. */
export function compact(n: number): string {
  if (n >= 9_999_500) return `${Math.round(n / 1_000_000)}M`;
  if (n >= 999_500) return `${num(n / 1_000_000, 1)}M`;
  if (n >= 10000) return `${Math.round(n / 1000)}k`;
  if (n >= 1000) return `${num(n / 1000, 1)}k`;
  return Math.round(n).toString();
}

/**
 * A span of time given in seconds: "unter 1 Min.", "12 Min.", "2 Std. 49 Min.", "3 Std.". With
 * `about`, an estimate: "etwa 12 Min." (the minute floor needs no "etwa").
 */
export function duration(seconds: number, { about = false }: { about?: boolean } = {}): string {
  const mins = Math.round(Math.max(0, seconds) / 60);
  if (mins < 1) return "unter 1 Min.";
  const h = Math.floor(mins / 60);
  const m = mins % 60;
  const text = h === 0 ? `${m} Min.` : m ? `${h} Std. ${m} Min.` : `${h} Std.`;
  return about ? `etwa ${text}` : text;
}

/** How long ago, from seconds since: "gerade eben", "vor 12 Min.", "vor 2 Std. 5 Min.", "gestern", "vor 3 Tagen". */
export function ago(seconds: number): string {
  if (seconds < 60) return "gerade eben";
  if (seconds < 24 * 3600) return `vor ${duration(seconds)}`;
  const days = Math.round(seconds / 86400);
  return days === 1 ? "gestern" : `vor ${days} Tagen`;
}

/** Time left of a running task: "noch etwa 12 Min."; null without an estimate. */
export function remaining(seconds: number | null): string | null {
  if (seconds === null || !Number.isFinite(seconds)) return null;
  return seconds < 60 ? "gleich fertig" : `noch ${duration(seconds, { about: true })}`;
}

const SYMBOL: Record<string, string> = { USD: "$", EUR: "€", GBP: "£" };

/** Money as the client writes it, symbol first: "$11,40". Tournament chips have no decimals. */
export function money(amount: number, currency: string | null, game: "cash" | "tournament" = "cash"): string {
  if (game === "tournament" || !currency) return Math.round(amount / 100).toLocaleString("de-DE");
  const v = num(amount / 100, 2);
  return SYMBOL[currency] ? `${amount < 0 ? MINUS : ""}${SYMBOL[currency]}${v.replace("-", "")}` : `${v} ${currency}`;
}

/** Short money for tight spaces: "$1,25", "€12". */
export function moneyShort(amount: number, currency: string | null): string {
  const v = amount / 100;
  const text = v.toLocaleString("de-DE", { minimumFractionDigits: v % 1 ? 2 : 0, maximumFractionDigits: 2 });
  return `${SYMBOL[currency ?? ""] ?? ""}${text}`;
}

/** Tournament money; play chips and points have no decimals. */
export function cash(amount: number, currency: string | null): string {
  const whole = Math.round(amount / 100).toLocaleString("de-DE");
  if (!currency || currency === "PLAY") return whole;
  if (currency === "FPP" || currency === "SC") return `${whole} ${currency}`;
  return money(amount, currency);
}

/** Long names shortened in the middle, so two that share a start stay apart: "Ein_sehr_l…ername_0". */
export function middle(text: string, max = 20): string {
  if (text.length <= max) return text;
  const tail = Math.ceil((max - 1) * 0.4);
  return `${text.slice(0, max - 1 - tail)}…${text.slice(-tail)}`;
}
