import type { ClassicPreset, HudConfig, Palette, StatId, StatValue, TournamentRow } from "./types";
import { cash, compact, moneyShort, num } from "./ui/format";
import { tournamentLabel } from "./app/format";
import { WORDS, explain, type WordId } from "./ui/words";

export { cash, compact, money, moneyShort } from "./ui/format";

/** Name and explanation of a stat from the glossary. */
const said = (id: StatId & WordId) => ({ name: WORDS[id].word, help: explain(id) });

/** Each value's name in words a casual player reads at once; the abbreviation stays beside it. */
export const PLAIN_NAME: Record<StatId, string> = {
  hands: "Hände",
  vpip: "Spielt mit",
  pfr: "Erhöht",
  three_bet: "Erhöht nochmal",
  fold_to_three_bet: "Gibt nach Re-Raise auf",
  steal: "Greift Blinds an",
  fold_to_steal: "Gibt Blinds her",
  cbet: "Setzt am Flop nach",
  fold_to_cbet: "Gibt am Flop auf",
  wtsd: "Geht bis zum Showdown",
  wsd: "Gewinnt am Showdown",
  af: "Aggression",
  afq: "Aggressiv nach dem Flop",
};

/** `plain` + value + `after` reads as a sentence in the "Klartext" preset. */
export const STAT_META: Record<StatId, { short: string; name: string; plain: string; after?: string; help: string; ratio?: boolean }> = {
  hands: { short: "H", ...said("hands"), plain: "Hände" },
  vpip: { short: "VPIP", ...said("vpip"), plain: "spielt" },
  pfr: { short: "PFR", ...said("pfr"), plain: "erhöht" },
  three_bet: { short: "3B", ...said("three_bet"), plain: "erhöht erneut" },
  fold_to_three_bet: { short: "F3B", ...said("fold_to_three_bet"), plain: "gibt auf Re-Raise auf" },
  steal: { short: "ST", ...said("steal"), plain: "greift Blinds an" },
  fold_to_steal: { short: "FST", ...said("fold_to_steal"), plain: "gibt Blinds her" },
  cbet: { short: "CB", ...said("cbet"), plain: "setzt am Flop nach" },
  fold_to_cbet: { short: "FCB", ...said("fold_to_cbet"), plain: "gibt am Flop auf" },
  wtsd: { short: "WTSD", ...said("wtsd"), plain: "geht zum Showdown" },
  wsd: { short: "W$SD", ...said("wsd"), plain: "gewinnt Showdowns" },
  af: { short: "AF", ...said("af"), plain: "setzt/erhöht", after: "so oft wie er mitgeht", ratio: true },
  afq: { short: "AFq", ...said("afq"), plain: "aggressiv nach dem Flop" },
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

/** The usual range of a value in prose: "20–35 %", "1,5–3,5×". */
export function usual(id: StatId, range: { low: number; high: number }): string {
  const d = STAT_META[id].ratio ? 1 : 0;
  return `${num(range.low, d)}–${num(range.high, d)}${STAT_META[id].ratio ? "×" : " %"}`;
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

/** How full a ring segment is: percent of 100, AF on a 0–5 scale. 0 without a sample. */
export function fill(s: StatValue | undefined): number {
  if (!s || s.value === null) return 0;
  if (STAT_META[s.id].ratio) return Math.min(s.value / 5, 1);
  return Math.min(Math.max(s.value / 100, 0), 1);
}

/** The name from the table window, else the game: "NL Hold'em #4046789300". */
export function tournamentName(t: Pick<TournamentRow, "id" | "game" | "name">): string {
  return t.name || `${t.game || "Turnier"} #${tournamentLabel(t.id)}`;
}

/** Buy-in as paid, "Freeroll", or "–" while unknown. */
export function buyInText(t: Pick<TournamentRow, "buy_in" | "cost" | "currency">): string {
  if (!t.buy_in) return "–";
  return t.buy_in.currency ? cash(t.cost ?? 0, t.currency) : "Freeroll";
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
    lines: [["vpip", "pfr"], ["three_bet", "steal"], ["af"]],
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
