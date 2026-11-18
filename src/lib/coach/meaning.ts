import type { Verdict } from "./types";

/** Verdicts from worst to best, with what each means for the player. */
export const VERDICTS: { id: Verdict; name: string; meaning: string }[] = [
  { id: "fehler", name: "Fehler", meaning: "hat dich Chips gekostet" },
  { id: "fraglich", name: "Fraglich", meaning: "knapp, meist geht es besser" },
  { id: "ok", name: "Okay", meaning: "vertretbar gespielt" },
  { id: "stark", name: "Stark", meaning: "genau richtig gespielt" },
];

export const verdictOf = (v: Verdict) => VERDICTS.find((x) => x.id === v)!;

export const toPractise = (v: Verdict) => v === "fehler" || v === "fraglich";

const POSITIONS: Record<string, string> = {
  BTN: "Button: du handelst nach dem Flop als Letzter",
  SB: "Small Blind: kleiner Pflichteinsatz, nach dem Flop zuerst dran",
  BB: "Big Blind: großer Pflichteinsatz, handelt vor dem Flop als Letzter",
  CO: "Cutoff: der Platz rechts vom Button",
  HJ: "Hijack: zwei Plätze rechts vom Button",
  LJ: "Lojack: drei Plätze rechts vom Button",
  UTG: "Under the Gun: vor dem Flop als Erster dran",
  "UTG+1": "Einer nach Under the Gun, frühe Position",
  MP: "Mittlere Position",
};

export const positionHint = (p: string | null) => (p ? (POSITIONS[p] ?? p) : "Position unbekannt");

/** "openai/gpt-6.1-sol" → "gpt-6.1-sol". */
export const shortModel = (id: string) => id.slice(id.indexOf("/") + 1);

/** Why the coach cannot analyse a tournament yet, or null when it can. */
export function notAnalyzable(r: { prize: number | null; game: string; hands: number }): string | null {
  if (r.prize === null) return "Erst wenn das Turnier für dich vorbei ist";
  if (r.game && !r.game.includes("Hold")) return "Der Coach analysiert bisher nur Hold'em";
  if (r.hands === 0) return "Keine Hände von diesem Turnier gespeichert";
  return null;
}

/** Poker terms the coach's figures use, explained in a sentence where they appear. */
const TERMS: [RegExp, string, string][] = [
  [/\bEV\b/, "EV", "Erwartungswert – was dir die Entscheidung im Schnitt bringt, in Big Blinds"],
  [/Nash/, "Nash", "die Spielweise, die kein Gegner ausnutzen kann"],
  [/Benötigte Equity|Pot Odds/, "Benötigte Equity", "so oft musst du gewinnen, damit sich der Call lohnt"],
  [/Equity gegen/, "Equity", "dein Anteil am Pot, wenn alle Karten aufgedeckt würden"],
  [/\bOuts\b/, "Outs", "Karten, die deine Hand zur besten machen"],
  [/\bRange\b/, "Range", "alle Hände, die der Gegner hier haben kann"],
  [/\bICM\b/, "ICM", "rechnet Chips in Preisgeld um; nahe den Preisrängen zählt Überleben mehr"],
  [/Fold Equity/, "Fold Equity", "der Wert davon, dass der Gegner aufgibt"],
];

/** Explanations for the terms in these figure labels, in order of first use. */
export function termsIn(labels: string[]): { term: string; meaning: string }[] {
  const text = labels.join(" ");
  return TERMS.filter(([re]) => re.test(text)).map(([, term, meaning]) => ({ term, meaning }));
}
