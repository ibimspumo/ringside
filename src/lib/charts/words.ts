// Plain-German words for the chart pages: positions, terms and the answer sentence.

/** An action shown at a seat of the small table. */
export type SeatMark = { text: string; color: string };

const POS_NAME: Record<string, string> = {
  UTG: "UTG",
  "UTG+1": "UTG+1",
  "UTG+2": "UTG+2",
  LJ: "Lojack",
  HJ: "Hijack",
  CO: "Cutoff",
  BTN: "Button",
  SB: "Small Blind",
  BB: "Big Blind",
};

/** Seat names from the first to act preflop to the big blind, as `position_names` in the engine names them. */
const TABLE: Record<number, string[]> = {
  2: ["SB", "BB"],
  3: ["BTN", "SB", "BB"],
  4: ["CO", "BTN", "SB", "BB"],
  5: ["HJ", "CO", "BTN", "SB", "BB"],
  6: ["LJ", "HJ", "CO", "BTN", "SB", "BB"],
  7: ["UTG", "LJ", "HJ", "CO", "BTN", "SB", "BB"],
  8: ["UTG", "UTG+1", "LJ", "HJ", "CO", "BTN", "SB", "BB"],
  9: ["UTG", "UTG+1", "UTG+2", "LJ", "HJ", "CO", "BTN", "SB", "BB"],
};

/** Names of the seats at a table of `players`, seat 0 first. */
export function tablePositions(players: number): string[] {
  return TABLE[players] ?? [];
}

/** "Vom Button", "Aus dem Big Blind", "Von UTG (als Erster dran)". */
export function fromPos(pos: string): string {
  if (pos === "SB" || pos === "BB") return `Aus dem ${POS_NAME[pos]}`;
  const name = POS_NAME[pos];
  if (name && !name.startsWith("UTG")) return `Vom ${name}`;
  return `Von ${pos} (${pos === "UTG" ? "als Erster dran" : "früh"})`;
}

/** "vom Button", "vom Big Blind", "von UTG": who made the bet. */
export function byPos(pos: string): string {
  const name = POS_NAME[pos];
  return name && !name.startsWith("UTG") ? `vom ${name}` : `von ${pos}`;
}

const POS_HINT: Record<string, string> = {
  UTG: "Under the Gun: sitzt links vom Big Blind und muss vor dem Flop als Erster handeln – die schwerste Position.",
  "UTG+1": "Der Platz links von UTG; handelt vor dem Flop als Zweiter.",
  "UTG+2": "Der dritte Platz nach dem Big Blind; noch frühe Position.",
  LJ: "Lojack: vier Plätze vor dem Button. Viele Spieler handeln noch nach dir, also eher vorsichtig.",
  HJ: "Hijack: drei Plätze vor dem Button.",
  CO: "Cutoff: direkt vor dem Button. Nur noch Button und Blinds handeln nach dir.",
  BTN: "Button (Dealer): handelt nach dem Flop immer als Letzter – die beste Position.",
  SB: "Small Blind: zahlt vor dem Austeilen einen halben Big Blind und handelt nach dem Flop zuerst.",
  BB: "Big Blind: zahlt vor dem Austeilen einen ganzen Big Blind und handelt vor dem Flop als Letzter.",
};

export function posHint(pos: string): string {
  return POS_HINT[pos] ?? `${pos}: dein Platz am Tisch.`;
}


export const pct = (x: number, d = 1) => x.toLocaleString("de-DE", { maximumFractionDigits: d });

/** A BB difference without sign, two decimals: "0,40". */
export const amount = (x: number) => Math.abs(x).toLocaleString("de-DE", { minimumFractionDigits: 2, maximumFractionDigits: 2 });
