import type { Depth, Selection } from "./select";
import { BANDS, STACK_CAP } from "./select";
import type { Comparison, History, Move } from "./types";

export const HISTORY: Record<History, { label: string; hint: string }> = {
  unopened: { label: "Alle gefoldet", hint: "Deine erste Aktion, alle vor dir haben gefoldet: Open, Limp oder Fold." },
  limped: { label: "Nach Limp", hint: "Vor dir hat jemand nur den Big Blind mitbezahlt (Limp), niemand hat erhöht." },
  vs_open: { label: "Erhöhung", hint: "Vor dir hat einer erhöht (Open), noch niemand gecallt. Du foldest, callst oder 3-bettest." },
  squeeze: {
    label: "Erhöht + Call",
    hint: "Vor dir ein Open und mindestens ein Call. Ein Raise hier heißt Squeeze – er setzt beide unter Druck.",
  },
  vs_three_bet: {
    label: "Gegen 3-Bet",
    hint: "Es wurde schon re-raist (3-Bet oder mehr), oft nach deinem eigenen Open.",
  },
  vs_shove: { label: "Gegen All-in", hint: "Der letzte Raise vor dir war ein All-in. Du callst oder foldest." },
};

export const MOVE: Record<Move, { label: string; color: string }> = {
  fold: { label: "Fold", color: "var(--surface-2)" },
  call: { label: "Call / Limp", color: "color-mix(in srgb, var(--pos) 70%, var(--surface-2))" },
  raise: { label: "Raise", color: "color-mix(in srgb, var(--amber) 80%, var(--surface-2))" },
  all_in: { label: "All-in", color: "color-mix(in srgb, var(--neg) 82%, var(--surface-2))" },
};

/** Too loose (you play what the model folds) and too tight. */
export const WIDE = "var(--range-wide)";
export const TIGHT = "var(--range-tight)";

const POSITION_FROM: Record<string, string> = {
  BTN: "Vom Button",
  SB: "Aus dem Small Blind",
  BB: "Aus dem Big Blind",
  CO: "Vom Cutoff",
  HJ: "Vom Hijack",
  LJ: "Vom Lojack",
};

export const POSITION_NAME: Record<string, string> = {
  UTG: "Under the Gun – die erste Position nach den Blinds",
  "UTG+1": "die Position nach UTG",
  "UTG+2": "zwei Plätze nach UTG",
  "UTG+3": "drei Plätze nach UTG",
  "UTG+4": "vier Plätze nach UTG",
  LJ: "Lojack – vier Plätze vor dem Button",
  HJ: "Hijack – zwei Plätze vor dem Button",
  CO: "Cutoff – direkt vor dem Button",
  BTN: "Button – du handelst nach dem Flop zuletzt",
  SB: "Small Blind",
  BB: "Big Blind",
};

function from(position: string): string {
  return POSITION_FROM[position] ?? `Von ${position}`;
}

const AGAINST: Record<string, string> = {
  BTN: "vom Button",
  SB: "aus dem Small Blind",
  BB: "aus dem Big Blind",
  CO: "vom Cutoff",
  HJ: "vom Hijack",
  LJ: "vom Lojack",
};
export const against = (p: string) => AGAINST[p] ?? `von ${p}`;

export function depthLabel(d: Depth): string {
  if (d.kind === "all") return "jeder Tiefe";
  const [lo, hi] = d.kind === "band" ? BANDS[d.band] : [d.min, d.max >= STACK_CAP ? null : d.max];
  if (!lo && hi !== null) return `unter ${hi} BB`;
  if (hi === null) return `ab ${lo} BB`;
  return `${lo}–${hi} BB`;
}

/** What "playing" means in the situation, as a verb phrase with the percentage. */
function verb(h: History, vs: string | null, pct: string): string {
  const who = vs ? ` ${against(vs)}` : "";
  switch (h) {
    case "unopened":
      return `öffnest du ${pct}`;
    case "limped":
      return `spielst du nach einem Limp${who} ${pct} weiter`;
    case "vs_open":
      return `spielst du gegen ein Open${who} ${pct} weiter`;
    case "squeeze":
      return `spielst du nach Open${who} und Call ${pct} weiter`;
    case "vs_three_bet":
      return `spielst du gegen eine 3-Bet${who} ${pct} weiter`;
    case "vs_shove":
      return `callst du ein All-in${who} mit ${pct}`;
  }
}

export const pct = (x: number) => `${Math.round(x).toLocaleString("de-DE")} %`;

/** The answer at the top: how much you play here, and against the model. */
export function headline(s: Selection, youPct: number, cmp: Comparison | null): string {
  const where = s.position ? from(s.position) : "Über alle Positionen";
  const sentence = `${where} bei ${depthLabel(s.depth)} ${verb(s.history, s.vs, pct(youPct))}`;
  return cmp ? `${sentence}, das Modell ${pct(cmp.model_pct)}.` : `${sentence}.`;
}

/** A stored line ("CO:r BTN:c") in words. */
export function lineText(line: string): string {
  if (!line) return "alle vor dir gefoldet";
  const what: Record<string, string> = { l: "Limp", c: "Call", r: "Raise", a: "All-in" };
  return line
    .split(" ")
    .map((s) => {
      const [p, k] = s.split(":");
      return `${p} ${what[k] ?? k}`;
    })
    .join(", ");
}

const RANKS = "AKQJT98765432";
/** Class name for row * 13 + col with the ace first, suited above the diagonal. */
export function className(i: number): string {
  const r = Math.floor(i / 13);
  const c = i % 13;
  const hi = RANKS[Math.min(r, c)];
  const lo = RANKS[Math.max(r, c)];
  return r === c ? hi + lo : r < c ? `${hi}${lo}s` : `${hi}${lo}o`;
}

export const combos = (i: number) => {
  const r = Math.floor(i / 13);
  const c = i % 13;
  return r === c ? 6 : r < c ? 4 : 12;
};

/** Below this many hands a cell is too thin to judge. */
export const FEW = 5;
