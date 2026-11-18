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

/** "Vom Button", "Aus dem Big Blind", "Von UTG". */
export function fromPos(pos: string): string {
  if (pos === "SB" || pos === "BB") return `Aus dem ${POS_NAME[pos]}`;
  const name = POS_NAME[pos];
  return name && !name.startsWith("UTG") ? `Vom ${name}` : `Von ${pos}`;
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

export const TERM = {
  bb: "BB (Big Blind): der große Pflichteinsatz. Stacks und Gewinne werden in BB gezählt, damit sie für jede Blindstufe gleich lesbar sind.",
  stack: "Stack: wie viele Chips du vor der Hand hast, gemessen in Big Blinds. Mit 10 BB und weniger gilt fast nur noch all-in oder Fold.",
  shove: "Shove: sofort all-in gehen. Mit kurzem Stack die beste Waffe, weil der Gegner nur noch callen oder folden kann.",
  ante: "Ante: ein kleiner Pflichteinsatz, den jeder Spieler vor der Hand zahlt. Er macht den Pot größer, also lohnt sich früheres All-in.",
  bbAnte: "BB-Ante: der Big Blind zahlt die Ante für den ganzen Tisch. So spielen die meisten Turniere auf PokerStars.",
  equilibrium:
    "Gleichgewicht (Nash): keiner kann mehr gewinnen, indem er allein anders spielt. Gegen jeden Gegner verlierst du damit nichts – gegen schwache gewinnst du oft mehr, wenn du abweichst.",
  model:
    "Modell: Preflop wird exakt gerechnet, das Spiel nach dem Flop aber nur geschätzt. Gute Richtschnur, aber kein Solver-Ergebnis.",
  ev: "EV (Erwartungswert): was die Aktion im Schnitt einbringt, in Big Blinds. Höher ist besser; Folden kostet nichts mehr, als schon im Pot liegt.",
  combos:
    "Kombos: so viele Kartenkombinationen gibt es von dieser Hand. Paare 6, suited 4, offsuit 12 – offsuit-Hände bekommst du also dreimal so oft.",
  mixed:
    "Gemischt: Beide Aktionen bringen fast gleich viel. Im Gleichgewicht spielst du sie mal so, mal so; am Tisch ist keine der beiden ein Fehler.",
  reach: "Wie oft diese Situation an diesem Platz überhaupt vorkommt, von allen Händen aus gezählt.",
  exploit:
    "Ausnutzbar: wie viel ein perfekter Gegner gegen diese Strategie noch zusätzlich gewinnen könnte. Nahe 0 heißt: so gut wie fertig gerechnet.",
  live: "Für diese Stacktiefe gibt es keine vorgerechnete Tabelle. Ringside rechnet sie auf deinem Rechner; das Ergebnis wird gespeichert.",
  precomputed: "Vorgerechnet: Diese Tabelle ist fertig gerechnet und wird sofort geladen.",
  icm: "Chip-EV ohne ICM: gerechnet in Chips. Kurz vor dem Geld oder am Finaltisch spielst du besser etwas enger, weil Ausscheiden dort Preisgeld kostet.",
} as const;

export const pct = (x: number, d = 1) => x.toLocaleString("de-DE", { maximumFractionDigits: d });

/** A BB difference without sign, two decimals: "0,40". */
export const amount = (x: number) => Math.abs(x).toLocaleString("de-DE", { minimumFractionDigits: 2, maximumFractionDigits: 2 });
