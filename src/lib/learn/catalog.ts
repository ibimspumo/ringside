import type { ConceptCount } from "../coach/types";
import type { Grading, LearnKind, SkillProgress } from "../types";
import type { UnitId } from "./wissen/articles";

/** Kinds the generator can make, in learning order. */
export const TRAINABLE: LearnKind[] = [
  "position",
  "showdown",
  "outs",
  "probability",
  "pot_odds",
  "combos",
  "equity_hand",
  "equity_range",
  "push_fold_sb",
  "push_fold_bb",
  "push_fold_open",
  "push_fold_call",
];

export type SkillMeta = {
  short: string;
  /** What each exercise level adds. */
  levels: [string, string, string];
  icon: string;
  articles: string[];
};

export const SKILLS: Record<LearnKind, SkillMeta> = {
  position: {
    short: "Positionen",
    levels: ["Volle Tische", "Kleinere Tische", "Toter Button, fehlender Small Blind"],
    icon: "M12 6.5c4.7 0 8.5 2.5 8.5 5.5s-3.8 5.5-8.5 5.5S3.5 15 3.5 12 7.3 6.5 12 6.5zM12 17.5v2.5",
    articles: ["position"],
  },
  showdown: {
    short: "Showdown",
    levels: ["Verschiedene Hände", "Kicker und Split", "Bis zu vier Spieler"],
    icon: "M5 6h6v9H5zM13 9h6v9h-6z",
    articles: [],
  },
  outs: {
    short: "Outs",
    levels: ["Saubere Outs am Flop", "Halbe Outs, Flop und Turn", "Schmutzige Outs"],
    icon: "M4 6h5v7H4zM10 6h5v7h-5zM16 6h4v7h-4zM6 17h12",
    articles: ["outs", "regel-2-4"],
  },
  probability: {
    short: "Trefferchance",
    levels: ["Regel 2/4", "Exakt, eine Karte", "Exakt, zwei Karten"],
    icon: "M4 19c5-1 9-6 16-14M4 19c6-3 10-7 16-11",
    articles: ["regel-2-4", "outs"],
  },
  pot_odds: {
    short: "Pot Odds",
    levels: ["Benötigte Equity", "Krumme Beträge", "Call oder Fold"],
    icon: "M4 16h16M7 16V9M12 16V5M17 16v-4",
    articles: ["pot-odds", "equity", "ev"],
  },
  combos: {
    short: "Kombos",
    levels: ["Eine Handklasse", "Mit toten Karten", "Ganze Ranges"],
    icon: "M4 4h16v16H4zM4 9.3h16M4 14.6h16M9.3 4v16M14.6 4v16",
    articles: ["kombinatorik", "blocker", "range"],
  },
  equity_hand: {
    short: "Hand gegen Hand",
    levels: ["Preflop", "Am Flop", "Knappe Spots"],
    icon: "M3 7h5v8H3zM16 7h5v8h-5zM10 11h4",
    articles: ["equity"],
  },
  equity_range: {
    short: "Hand gegen Range",
    levels: ["Enge Ranges", "Breitere Ranges", "Am Flop"],
    icon: "M4 7h12M4 12h16M4 17h8",
    articles: ["range", "equity"],
  },
  push_fold_sb: {
    short: "All-in aus dem Small Blind",
    levels: ["Ohne Ante", "Mit Ante", "Volle Tische, totes Geld"],
    icon: "M5 18h14M8 18V9l4-4 4 4v9",
    articles: ["push-fold", "effektiver-stack", "m-ratio"],
  },
  push_fold_bb: {
    short: "Call im Big Blind",
    levels: ["Ohne Ante", "Mit Ante", "Volle Tische, totes Geld"],
    icon: "M12 4v12M7 11l5 5 5-5M5 20h14",
    articles: ["push-fold", "pot-odds", "effektiver-stack"],
  },
  push_fold_open: {
    short: "All-in nach Folds",
    levels: ["2–6 Spieler, ohne Ante", "6–9 Spieler mit Ante", "Ungleiche Stacks, knappe Spots"],
    icon: "M4 18h16M6 18v-5M10 18V9M14 18V6l4 4v8",
    articles: ["push-fold", "fold-equity", "effektiver-stack"],
  },
  push_fold_call: {
    short: "Call gegen All-in",
    levels: ["2–6 Spieler, ohne Ante", "6–9 Spieler mit Ante", "Overcalls, ungleiche Stacks"],
    icon: "M5 5l7 7M12 12l7-7M12 12v8M7 20h10",
    articles: ["push-fold", "pot-odds", "effektiver-stack"],
  },
  all_in: {
    short: "Deine All-ins",
    levels: ["Equity schätzen", "Equity schätzen", "Equity schätzen"],
    icon: "M12 4l2.5 5.2 5.5.8-4 3.9 1 5.6-5-2.7-5 2.7 1-5.6-4-3.9 5.5-.8z",
    articles: ["entscheidung-ergebnis", "ev"],
  },
};

export const GRADING: Record<Grading, { label: string; text: string }> = {
  exakt: { label: "Exakt", text: "Reine Mathematik mit eindeutiger Lösung. Nahe Schätzungen bekommen Teilpunkte." },
  formel: { label: "Formel", text: "Faustformel. Bewertet wird, ob du die Formel richtig anwendest." },
  modell: { label: "Modell", text: "Exakt innerhalb eines Modells. Die Annahmen stehen dabei." },
};

export const LEVEL_TEXT = ["Neu", "Kennengelernt", "Mit Hilfe", "Selbstständig", "Bestätigt"] as const;
export const LEVEL_HINT = [
  "Noch nicht geübt.",
  "Schon versucht.",
  "Mindestens 3 Antworten mit 60 % im Schnitt.",
  "8 Antworten mit 80 % im Schnitt, verteilt auf 2 Tage.",
  "Nach 3 Tagen erneut sicher gelöst.",
] as const;

export type Unit = { id: UnitId; n: number; title: string; goal: string; skills: LearnKind[]; articles: string[]; next: string[] };

/** The eleven units of the curriculum; skills are the exercises that train them today. */
export const UNITS: Unit[] = [
  {
    id: "grundlagen",
    n: 1,
    title: "Grundlagen",
    goal: "Handrangfolge, Positionen und in Big Blinds denken.",
    skills: ["showdown", "position"],
    articles: ["position", "big-blinds"],
    next: ["Beste 5 aus 7", "Ablauf einer Hand"],
  },
  {
    id: "mathe",
    n: 2,
    title: "Mathe-Kern",
    goal: "Outs, Trefferchance, Pot Odds und Equity sicher im Kopf.",
    skills: ["outs", "probability", "pot_odds", "equity_hand"],
    articles: ["outs", "regel-2-4", "pot-odds", "equity", "range", "implizite-odds", "ev"],
    next: ["Implizite Odds", "EV-Bäume"],
  },
  {
    id: "preflop",
    n: 3,
    title: "Preflop",
    goal: "Kombos zählen und Hände gegen Ranges einschätzen.",
    skills: ["combos", "equity_range"],
    articles: ["kombinatorik", "blocker"],
    next: ["Open-Ranges", "3-Bets"],
  },
  {
    id: "position",
    n: 4,
    title: "Position und Initiative",
    goal: "Warum das letzte Wort Geld wert ist.",
    skills: [],
    articles: ["equity-realisierung", "c-bet-warum"],
    next: ["Steal-Rechner", "Range-Vorteil"],
  },
  {
    id: "postflop",
    n: 5,
    title: "Postflop",
    goal: "Bets mit Zweck: Value, Bluff, Fold Equity, SPR.",
    skills: [],
    articles: ["fold-equity", "semi-bluff", "value-bluff", "mdf", "alpha", "spr"],
    next: ["MDF und Alpha", "SPR"],
  },
  {
    id: "turnier",
    n: 6,
    title: "Turnier und Stacktiefe",
    goal: "Effektiver Stack, M-Ratio, Antes.",
    skills: [],
    articles: ["effektiver-stack", "m-ratio", "big-blinds"],
    next: ["Effektiver Stack", "Antes"],
  },
  {
    id: "pushfold",
    n: 7,
    title: "Push/Fold",
    goal: "Shoven und Callen bei kurzen Stacks nach Nash, von heads-up bis zum vollen Tisch.",
    skills: ["push_fold_sb", "push_fold_bb", "push_fold_open", "push_fold_call"],
    articles: ["push-fold", "effektiver-stack"],
    next: ["Push/Fold mit ICM"],
  },
  {
    id: "icm",
    n: 8,
    title: "ICM und Bubble",
    goal: "Chips in Preisgeld umrechnen und unter Druck richtig callen.",
    skills: [],
    articles: ["icm", "bubble-factor"],
    next: ["ICM-Rechner", "Bubble-Calls"],
  },
  {
    id: "bounty",
    n: 9,
    title: "Bounty-Turniere",
    goal: "Kopfgeld in Pot Odds einrechnen.",
    skills: [],
    articles: ["bounty-pot-odds"],
    next: ["Bounty Power"],
  },
  {
    id: "hud",
    n: 10,
    title: "Gegner lesen mit HUD-Werten",
    goal: "Was die Zahlen im HUD bedeuten und wann du ihnen trauen kannst.",
    skills: [],
    articles: ["stichprobe", "vpip", "pfr", "three-bet", "fold-three-bet", "steal", "fold-steal", "cbet", "fold-cbet", "wtsd", "wsd", "af", "afq"],
    next: ["Stat-Quiz"],
  },
  {
    id: "mental",
    n: 11,
    title: "Mentales Spiel",
    goal: "Entscheidung vom Ergebnis trennen.",
    skills: ["all_in"],
    articles: ["entscheidung-ergebnis"],
    next: ["Varianz"],
  },
];

/** Coach concepts with at least one mistake or questionable spot, weakest first as the coach sorts them. */
export const weakConcepts = (concepts: ConceptCount[]) => concepts.filter((c) => c.mistakes + c.questionable > 0);

/** The generated exercise that trains a coach concept, if there is one. */
export const trainableKind = (c: ConceptCount): LearnKind | null => TRAINABLE.find((k) => k === c.exercise) ?? null;

/** 0–1 share of the way to "bestätigt". */
export const mastery = (s: SkillProgress | undefined) => (s ? s.level / 4 : 0);

/** Exercise level to practise next: the highest tried, one up once it is solid. */
export function nextLevel(s: SkillProgress | undefined): number {
  if (!s || s.max_level === 0) return 1;
  if (s.level >= 3 && (s.mean_score ?? 0) >= 0.8) return Math.min(3, s.max_level + 1);
  return s.max_level;
}

/** Days since the last attempt, or null. */
export function daysSince(s: SkillProgress | undefined): number | null {
  if (!s?.last_at) return null;
  return (Date.now() / 1000 - s.last_at) / 86_400;
}

/**
 * Due for review: practised before, and either shaky or not seen for a while
 * (sooner when the level is low). A simple stand-in until the backend schedules reviews.
 */
export function isDue(s: SkillProgress): boolean {
  if (s.attempts === 0 || s.id === "all_in") return false;
  const days = daysSince(s) ?? 0;
  const gap = [0, 0.5, 1, 3, 7][s.level];
  return days >= gap || (s.mean_score ?? 1) < 0.6;
}

export const fmtPct = (x: number, d = 0) => `${x.toLocaleString("de-DE", { minimumFractionDigits: d, maximumFractionDigits: d })} %`;
export const fmtBB = (x: number, signed = false) => {
  const v = Math.abs(x).toLocaleString("de-DE", { maximumFractionDigits: 2 });
  const sign = signed ? (x > 0.004 ? "+" : x < -0.004 ? "−" : "±") : x < 0 ? "−" : "";
  return `${sign}${v} bb`;
};

/** "TT+, AQs+, KQo" → the hand classes it names. */
export function parseRange(range: string): string[] {
  const R = "23456789TJQKA";
  return range.split(",").flatMap((part) => {
    const p = part.trim();
    if (!p) return [];
    const plus = p.endsWith("+");
    const c = plus ? p.slice(0, -1) : p;
    if (!plus) return [c];
    if (c[0] === c[1]) return [...R.slice(R.indexOf(c[0]))].map((x) => x + x);
    const out: string[] = [];
    for (let k = R.indexOf(c[1]); k < R.indexOf(c[0]); k++) out.push(c[0] + R[k] + (c[2] ?? ""));
    return out;
  });
}

/** German message for a rejected learn command. */
export function errorText(e: unknown): string {
  const msg = typeof e === "string" ? e : e instanceof Error ? e.message : "";
  if (msg === "gesperrt") return "Gesperrt, solange PokerStars läuft.";
  return msg ? `Das hat nicht geklappt: ${msg}` : "Das hat nicht geklappt. Versuch es noch einmal.";
}

/** The first unit with a skill below "mit Hilfe": where the path continues. */
export function currentUnit(byKind: Map<LearnKind, SkillProgress>): Unit | null {
  return UNITS.find((u) => u.skills.some((k) => (byKind.get(k)?.level ?? 0) < 2)) ?? null;
}

/** One plain sentence per poker term that feedback and exercises show, matched by the start of the label. */
const TERMS: [RegExp, string][] = [
  [/^Nash/, "Die Entscheidung, die ein perfekt spielender Gegner nicht ausnutzen kann."],
  [/^EV-Verlust|^EV /, "EV heißt Erwartungswert: was eine Entscheidung im Schnitt bringt, gemessen in Big Blinds (bb)."],
  [/^Erwartet \(EV\)/, "Was dein All-in im Schnitt wert war, unabhängig davon, wie die Karten fielen."],
  [/^(Shove|Call)-Range/, "Anteil aller Starthände, mit denen man hier all-in geht bzw. callt."],
  [/^Effektiver Stack/, "Der kleinere der beteiligten Stacks. Mehr kann in dieser Hand nicht gewonnen oder verloren werden."],
  [/^Benötigte Equity/, "So oft musst du mindestens gewinnen, damit sich der Call lohnt."],
  [/^(Deine )?Equity/, "Dein Anteil am Pot, wenn alle Karten ausgeteilt werden."],
  [/^Pot Odds/, "Verhältnis zwischen dem Pot und dem, was du zahlen musst."],
  [/^Totes Geld/, "Chips im Pot von Spielern, die schon gefoldet haben."],
  [/^Schmutzige Outs/, "Karten, die dir helfen, dem Gegner aber eine noch bessere Hand geben können."],
  [/^Halbe Outs/, "Karten, die dir nur manchmal helfen. Sie zählen halb."],
  [/^Outs/, "Karten, die deine Hand nach vorn bringen."],
  [/^Regel von 2 und 4|^Korrigierte Regel/, "Outs mal 2 für eine Karte, Outs mal 4 für zwei Karten ergibt grob die Trefferchance in Prozent."],
  [/^Glück/, "Ergebnis minus Erwartungswert. Über viele Hände gleicht es sich aus."],
  [/^Blockiert/, "Kombinationen, die es wegen deiner eigenen Karten nicht geben kann."],
  [/^Mögliche Kombos/, "Wie viele Kartenpaare der Gegner halten kann."],
  [/^Chip-EV/, "Gerechnet in Chips, nicht in Preisgeld."],
  [/^ICM/, "Rechnet Chips in Preisgeld um. Ohne ICM zählt jeder Chip gleich viel."],
];

/** Terms inside running text (questions, model notes) that get the same plain explanation. */
const INLINE: [RegExp, string][] = [
  [/Heads-up-Nash|Nash/, "Die Entscheidung, die ein perfekt spielender Gegner nicht ausnutzen kann."],
  [/Chip-EV/, "Gerechnet in Chips, nicht in Preisgeld."],
  [/ICM/, "Rechnet Chips in Preisgeld um. Ohne ICM zählt jeder Chip gleich viel."],
  [/totes Geld|Totes Geld/, "Chips im Pot von Spielern, die schon gefoldet haben."],
  [/Effektiver Stack|effektive[rn]? Stack/, "Der kleinere der beteiligten Stacks. Mehr kann in dieser Hand nicht gewonnen oder verloren werden."],
  [/Equity/, "Wie oft du gewinnst, wenn alle Karten ausgeteilt werden – dein Anteil am Pot."],
  [/Outs/, "Karten, die deine Hand nach vorn bringen."],
];

/** Splits text into plain parts and explained terms, for dotted-underline tooltips. */
export function withTerms(text: string): { text: string; tip: string | null }[] {
  const re = new RegExp(INLINE.map(([r]) => r.source).join("|"), "g");
  const out: { text: string; tip: string | null }[] = [];
  let last = 0;
  for (const m of text.matchAll(re)) {
    if (m.index > last) out.push({ text: text.slice(last, m.index), tip: null });
    out.push({ text: m[0], tip: INLINE.find(([r]) => new RegExp(`^(?:${r.source})$`).test(m[0]))?.[1] ?? null });
    last = m.index + m[0].length;
  }
  if (last < text.length) out.push({ text: text.slice(last), tip: null });
  return out;
}

/** The first explained term in a text, for a one-line hint under a question. */
export function firstTerm(text: string): { term: string; tip: string } | null {
  const hit = withTerms(text).find((p) => p.tip);
  return hit?.tip ? { term: hit.text, tip: hit.tip } : null;
}

export function explain(label: string): string | null {
  return TERMS.find(([re]) => re.test(label))?.[1] ?? null;
}
