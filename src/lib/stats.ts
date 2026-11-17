import type { ClassicPreset, GameKind, HudConfig, Palette, StatId, StatValue } from "./types";

export const STAT_META: Record<StatId, { short: string; name: string; plain: string; help: string; ratio?: boolean }> = {
  hands: { short: "H", name: "Hände", plain: "Hände", help: "Gemeinsame Hände – je mehr, desto verlässlicher die Werte." },
  vpip: { short: "VPIP", name: "VPIP", plain: "spielt", help: "Wie oft freiwillig Geld in den Pot gelegt wird. Hoch = spielt viele Hände (loose)." },
  pfr: { short: "PFR", name: "Preflop-Raise", plain: "erhöht", help: "Wie oft vor dem Flop erhöht wird. Nah an VPIP = aggressiv, weit darunter = passiv." },
  three_bet: { short: "3B", name: "3-Bet", plain: "3-bettet", help: "Re-Raise gegen einen Open-Raise." },
  fold_to_three_bet: { short: "F3B", name: "Fold auf 3-Bet", plain: "gibt auf Re-Raise auf", help: "Wie oft nach eigenem Raise auf ein Re-Raise gefoldet wird." },
  steal: { short: "ST", name: "Steal", plain: "klaut Blinds", help: "Open-Raise aus Cutoff, Button oder Small Blind, wenn alle davor gefoldet haben." },
  fold_to_steal: { short: "FST", name: "Fold auf Steal", plain: "gibt Blinds her", help: "Wie oft die Blinds gegen einen Steal folden." },
  cbet: { short: "CB", name: "C-Bet", plain: "setzt am Flop nach", help: "Continuation-Bet am Flop nach eigenem Preflop-Raise." },
  fold_to_cbet: { short: "FCB", name: "Fold auf C-Bet", plain: "gibt am Flop auf", help: "Wie oft auf eine C-Bet am Flop gefoldet wird." },
  wtsd: { short: "WTSD", name: "Showdown", plain: "geht zum Showdown", help: "Wie oft es nach gesehenem Flop zum Showdown geht." },
  wsd: { short: "W$SD", name: "Gewonnen am Showdown", plain: "gewinnt Showdowns", help: "Anteil gewonnener Showdowns." },
  af: { short: "AF", name: "Aggression", plain: "aggressiv", help: "(Bets + Raises) / Calls nach dem Flop. Über 3 = sehr aggressiv.", ratio: true },
  afq: { short: "AFq", name: "Aggressions-Frequenz", plain: "aggressiv nach dem Flop", help: "Anteil aggressiver Aktionen nach dem Flop." },
};

export const ALL_STATS = Object.keys(STAT_META) as StatId[];
export const RATED_STATS = ALL_STATS.filter((id) => id !== "hands");

export type Tone = "low" | "mid" | "high" | "none";

export const PALETTES: Record<Palette, { name: string; low: string; mid: string; high: string }> = {
  traffic: { name: "Ampel", low: "#e5483b", mid: "#f2b12e", high: "#78c142" },
  icefire: { name: "Eis & Feuer", low: "#4cc6ff", mid: "#e9edf4", high: "#ff5b3a" },
};

export function stat(stats: StatValue[], id: StatId): StatValue | undefined {
  return stats.find((s) => s.id === id);
}

export function tone(hud: HudConfig, s: StatValue | undefined): Tone {
  if (!s || s.value === null || s.id === "hands") return "none";
  const t = hud.thresholds[s.id];
  if (!t) return "mid";
  if (s.value < t.low) return "low";
  if (s.value > t.high) return "high";
  return "mid";
}

/** CSS colour for a stat, or `fallback` when there is no sample yet. */
export function toneColor(hud: HudConfig, s: StatValue | undefined, fallback = "var(--hud-empty)"): string {
  const t = tone(hud, s);
  return t === "none" ? fallback : PALETTES[hud.palette]?.[t] ?? PALETTES.traffic[t];
}

const de1 = (n: number) => n.toLocaleString("de-DE", { minimumFractionDigits: 1, maximumFractionDigits: 1 });

export function fmt(s: StatValue | undefined): string {
  if (!s || s.value === null) return "–";
  if (s.id === "hands") return compact(s.value);
  if (STAT_META[s.id].ratio) return s.value >= 10 ? Math.round(s.value).toString() : de1(s.value);
  return Math.round(s.value).toString();
}

/** Unit written after a value in prose: "%" for frequencies, "×" for AF. */
export function unit(s: StatValue | undefined): string {
  if (!s || s.value === null || s.id === "hands") return "";
  return STAT_META[s.id].ratio ? "×" : "%";
}

/** Value with its unit, for prose: "24 %", "2,6×". */
export function fmtUnit(s: StatValue | undefined): string {
  const u = unit(s);
  return u === "%" ? `${fmt(s)} %` : `${fmt(s)}${u}`;
}

export function compact(n: number): string {
  if (n >= 10000) return `${Math.round(n / 1000)}k`;
  if (n >= 1000) return `${de1(n / 1000)}k`;
  return Math.round(n).toString();
}

const SYMBOL: Record<string, string> = { USD: "$", EUR: "€", GBP: "£" };

/** Amounts are stored in hundredths. */
export function money(amount: number, currency: string | null, game: GameKind = "cash"): string {
  if (game === "tournament" || !currency) return Math.round(amount / 100).toLocaleString("de-DE");
  const v = (amount / 100).toLocaleString("de-DE", { minimumFractionDigits: 2, maximumFractionDigits: 2 });
  // Symbol first, as the client writes it: "$11,40".
  return SYMBOL[currency] ? `${amount < 0 ? "−" : ""}${SYMBOL[currency]}${v.replace("-", "")}` : `${v} ${currency}`;
}

/** Short money for tight spaces: "$1,25", "€12". */
export function moneyShort(amount: number, currency: string | null): string {
  const v = amount / 100;
  const text = v.toLocaleString("de-DE", { minimumFractionDigits: v % 1 ? 2 : 0, maximumFractionDigits: 2 });
  return `${SYMBOL[currency ?? ""] ?? ""}${text}`;
}

/** Bounty totals per currency as one string: "$37,45 · €5". Empty when nothing was won. */
export function bountyText(bounties: Record<string, number>): string {
  return Object.entries(bounties)
    .filter(([, a]) => a !== 0)
    .map(([c, a]) => moneyShort(a, c))
    .join(" · ");
}

export function bb(n: number, digits = 1): string {
  return `${n > 0 ? "+" : ""}${n.toLocaleString("de-DE", { minimumFractionDigits: digits, maximumFractionDigits: digits })}`;
}

/** Classic text-box layouts. Each line is a list of stats; `words` writes them out in prose. */
export const CLASSIC_PRESETS: Record<
  Exclude<ClassicPreset, "custom">,
  { name: string; description: string; words: boolean; lines: StatId[][] }
> = {
  beginner: {
    name: "Klartext",
    description: "Ausgeschriebene Sätze, ideal zum Lernen",
    words: true,
    lines: [["vpip", "pfr"], ["three_bet", "af"]],
  },
  standard: {
    name: "Standard",
    description: "VPIP / PFR / 3-Bet / AF und Hände in einer Zeile",
    words: false,
    lines: [["vpip", "pfr", "three_bet", "af", "hands"]],
  },
  tournament: {
    name: "Turnier",
    description: "Steals und Blind-Verteidigung für MTTs",
    words: false,
    lines: [["vpip", "pfr", "three_bet", "hands"], ["steal", "fold_to_steal", "fold_to_three_bet"]],
  },
  detailed: {
    name: "Ausführlich",
    description: "Zwei Zeilen: Preflop, danach Flop und Showdown",
    words: false,
    lines: [["vpip", "pfr", "three_bet", "fold_to_three_bet", "steal", "fold_to_steal"], ["cbet", "fold_to_cbet", "af", "wtsd", "wsd", "hands"]],
  },
};

export function classicLines(hud: HudConfig): { words: boolean; lines: StatId[][] } {
  const preset = hud.classic.preset;
  if (preset === "custom") {
    const ids = hud.classic.custom;
    const lines: StatId[][] = [];
    for (let i = 0; i < ids.length; i += 4) lines.push(ids.slice(i, i + 4));
    return { words: false, lines };
  }
  return CLASSIC_PRESETS[preset] ?? CLASSIC_PRESETS.beginner;
}
