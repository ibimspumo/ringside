import type { GameKind, HudConfig, StatId, StatValue } from "./types";

export const STAT_META: Record<StatId, { short: string; name: string; help: string; ratio?: boolean }> = {
  hands: { short: "H", name: "Hände", help: "Anzahl gemeinsamer Hände – je mehr, desto verlässlicher die Werte." },
  vpip: { short: "VPIP", name: "VPIP", help: "Wie oft freiwillig Geld in den Pot gelegt wird. Hoch = spielt viele Hände (loose)." },
  pfr: { short: "PFR", name: "Preflop-Raise", help: "Wie oft vor dem Flop erhöht wird. Nah an VPIP = aggressiv, weit darunter = passiv." },
  three_bet: { short: "3B", name: "3-Bet", help: "Re-Raise gegen einen Open-Raise." },
  fold_to_three_bet: { short: "F3B", name: "Fold auf 3-Bet", help: "Wie oft nach eigenem Raise auf ein Re-Raise gefoldet wird." },
  steal: { short: "ST", name: "Steal", help: "Open-Raise aus Cutoff, Button oder Small Blind, wenn alle davor gefoldet haben." },
  fold_to_steal: { short: "FST", name: "Fold auf Steal", help: "Wie oft die Blinds gegen einen Steal folden." },
  cbet: { short: "CB", name: "C-Bet", help: "Continuation-Bet am Flop nach eigenem Preflop-Raise." },
  fold_to_cbet: { short: "FCB", name: "Fold auf C-Bet", help: "Wie oft auf eine C-Bet am Flop gefoldet wird." },
  wtsd: { short: "WTSD", name: "Showdown", help: "Wie oft es nach gesehenem Flop zum Showdown geht." },
  wsd: { short: "W$SD", name: "Gewonnen am Showdown", help: "Anteil gewonnener Showdowns." },
  af: { short: "AF", name: "Aggression", help: "(Bets + Raises) / Calls nach dem Flop. Über 3 = sehr aggressiv.", ratio: true },
  afq: { short: "AFq", name: "Aggressions-Frequenz", help: "Anteil aggressiver Aktionen nach dem Flop." },
};

export const ALL_STATS = Object.keys(STAT_META) as StatId[];

export type Tone = "low" | "mid" | "high" | "none";

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

export function fmt(s: StatValue | undefined): string {
  if (!s || s.value === null) return "–";
  if (s.id === "hands") return compact(s.value);
  if (STAT_META[s.id].ratio) return s.value >= 10 ? s.value.toFixed(0) : s.value.toFixed(1);
  return Math.round(s.value).toString();
}

export function compact(n: number): string {
  if (n >= 10000) return `${Math.round(n / 1000)}k`;
  if (n >= 1000) return `${(n / 1000).toFixed(1)}k`;
  return Math.round(n).toString();
}

/** Arc fill 0..1 for the ring; AF is mapped onto 0..5. */
export function fill(s: StatValue | undefined): number {
  if (!s || s.value === null) return 0;
  if (STAT_META[s.id].ratio) return Math.min(s.value / 5, 1);
  return Math.min(Math.max(s.value / 100, 0), 1);
}

const SYMBOL: Record<string, string> = { USD: "$", EUR: "€", GBP: "£" };

/** Amounts are stored in hundredths. */
export function money(amount: number, currency: string | null, game: GameKind = "cash"): string {
  if (game === "tournament" || !currency) return Math.round(amount / 100).toLocaleString("de-DE");
  const v = (amount / 100).toLocaleString("de-DE", { minimumFractionDigits: 2, maximumFractionDigits: 2 });
  return `${v} ${SYMBOL[currency] ?? currency}`;
}

export function signed(text: string, n: number): string {
  return n > 0 ? `+${text}` : text;
}
