import type { StatId } from "../../types";
import { alpha, ev, hitChance, icm, mdf, neededEquity, num, pct, potOdds, setOnFlop } from "./math";
import { pairs, run } from "./poker";

export type Level = 1 | 2 | 3;
export const LEVELS: Record<Level, string> = { 1: "Einsteiger", 2: "Fortgeschritten", 3: "Erfahren" };

export type UnitId =
  | "grundlagen"
  | "mathe"
  | "preflop"
  | "position"
  | "postflop"
  | "turnier"
  | "pushfold"
  | "icm"
  | "bounty"
  | "hud"
  | "mental";

export const UNITS: { id: UnitId; title: string }[] = [
  { id: "grundlagen", title: "Grundlagen" },
  { id: "mathe", title: "Mathe-Kern" },
  { id: "preflop", title: "Preflop" },
  { id: "position", title: "Position und Initiative" },
  { id: "postflop", title: "Postflop" },
  { id: "turnier", title: "Turnier und Stacktiefe" },
  { id: "pushfold", title: "Push/Fold" },
  { id: "icm", title: "ICM und Bubble" },
  { id: "bounty", title: "Bounty-Turniere" },
  { id: "hud", title: "Gegner lesen mit HUD-Werten" },
  { id: "mental", title: "Mentales Spiel" },
];

export type WidgetId = "pot-odds" | "position" | "outs" | "rule-24" | "combos" | "stack" | "cbet";

export type WidgetProps = { mode?: "call" | "mdf" | "alpha" | "bb" | "m"; hand?: "AA" | "AK"; blocked?: boolean };

export type SeatMark = { pos: string; chip?: string; state?: "fold" | "allin" | "hero" | "active"; note?: string };
export type BarRow = { label: string; value: number; display: string; tone?: "pos" | "neg" | "amber" | "base" };

export type Block =
  | { kind: "points"; items: string[] }
  | { kind: "avoid"; items: string[] }
  | { kind: "note"; text: string }
  | {
      kind: "formula";
      name: string;
      num: string;
      den?: string;
      after?: string;
      example?: { given: string; calc: string; result: string };
      heuristic?: boolean;
    }
  | { kind: "table"; seats: 6 | 8 | 9; marks: SeatMark[]; caption: string }
  | { kind: "cards"; rows: { label: string; cards: string[]; tag?: string }[]; board?: string[]; highlight?: string[]; caption: string }
  | { kind: "matchup"; hero: { label: string; cards: string[] }; villain: { label: string; cards: string[] }; board: string[]; caption: string }
  | { kind: "scale"; stat: StatId; max: number; low: number; high: number; labels: [string, string, string]; ratio?: boolean; caption: string }
  | { kind: "bars"; rows: BarRow[]; max?: number; schematic?: boolean; caption: string }
  | { kind: "compare"; legend: [string, string]; rows: { label: string; a: number; b: number }[]; caption: string }
  | { kind: "tree"; root: string; branches: { label: string; p: number; result: number }[]; unit: string; caption: string }
  | { kind: "matrix"; range: string[]; caption: string }
  | { kind: "sample"; rows: { k: number; n: number }[]; caption: string }
  | { kind: "widget"; widget: WidgetId; props?: WidgetProps; caption?: string };

export type Article = {
  id: string;
  title: string;
  unit: UnitId;
  level: Level;
  oneLiner: string;
  blocks: Block[];
  related: string[];
  stat?: StatId;
  /** The page teaches a rule of thumb, not exact maths. */
  heuristic?: boolean;
  /** Extra search words. */
  aliases?: string[];
};

/* Worked examples, computed once so the text can never drift from the maths. ----------------- */

const flopToRiver = (outs: number) => hitChance(outs, 2, 47);

const set = setOnFlop();
const impliedNeeded = (2 * (1 - set)) / set - 5;

const evCall = ev([
  { p: 0.3, result: 30 },
  { p: 0.7, result: -10 },
]);

const semiEq = 9 / 46;

const pfEv = ev([
  { p: 0.6, result: 1.5 },
  { p: 0.4, result: 0.4 * 20 - 9.5 },
]);

const ICM_STACKS = [5000, 3000, 2000];
const ICM_PAY = [50, 30, 20];
const icmNow = icm(ICM_STACKS, ICM_PAY);

// Four left, three paid. Hero (3.000) calls an all-in from the 4.000 stack.
const BUBBLE_PAY = [50, 30, 20, 0];
const bNow = icm([3000, 4000, 2000, 1000], BUBBLE_PAY)[0];
const bWin = icm([6000, 1000, 2000, 1000], BUBBLE_PAY)[0];
const bLose = icm([0, 7000, 2000, 1000], BUBBLE_PAY)[0];
const bubbleFactor = (bNow - bLose) / (bWin - bNow);
const bubbleNeed = bubbleFactor / (1 + bubbleFactor);

const pctBar = (label: string, x: number, tone?: BarRow["tone"]): BarRow => ({ label, value: x, display: pct(x), tone });

/** Definition block for a HUD stat: hits ÷ chances, with a small worked example. */
function statFormula(name: string, numText: string, denText: string, k: number, n: number): Block {
  return {
    kind: "formula",
    name,
    num: numText,
    den: denText,
    example: { given: `${k} von ${n} Gelegenheiten`, calc: `${k} ÷ ${n}`, result: pct(k / n, 0) },
  };
}

const SMALL_SAMPLE = "Unter etwa 30 Gelegenheiten ist der Wert noch sehr wackelig. Schau auf den Bruch, nicht nur auf die Prozentzahl.";

/* The articles ------------------------------------------------------------------------------- */

export const ARTICLES: Article[] = [
  /* Grundlagen */
  {
    id: "big-blinds",
    title: "In Big Blinds denken",
    unit: "grundlagen",
    level: 1,
    oneLiner: "20.000 Chips sind nur groß, wenn die Blinds klein sind. Darum misst du Stacks in Big Blinds.",
    blocks: [
      { kind: "widget", widget: "stack", props: { mode: "bb" }, caption: "Zieh am Level: Dein Stack bleibt gleich, die Big Blinds schrumpfen." },
      { kind: "formula", name: "Stack in Big Blinds", num: "Chips", den: "Big Blind", example: { given: "20.000 Chips, Blinds 400/800", calc: "20.000 ÷ 800", result: "25 bb" } },
      {
        kind: "points",
        items: ["Strategie hängt an der Zahl der Big Blinds, nicht an den Chips.", "Jedes neue Level macht deinen Stack kleiner, auch wenn du nichts verlierst."],
      },
    ],
    related: ["m-ratio", "effektiver-stack", "push-fold"],
    aliases: ["bb", "stack", "blinds"],
  },

  /* Mathe-Kern */
  {
    id: "outs",
    title: "Outs",
    unit: "mathe",
    level: 1,
    oneLiner: "Outs sind die Karten im Deck, die deine Hand voraussichtlich zur besten machen.",
    blocks: [
      { kind: "widget", widget: "outs", caption: "Tipp alle Karten an, die dir helfen. Dann auflösen." },
      {
        kind: "points",
        items: ["Jede Karte nur einmal zählen, auch wenn sie zwei Draws trifft.", "Karten, die dem Gegner noch mehr helfen, sind schmutzige Outs: lieber abziehen."],
      },
      { kind: "avoid", items: ["Ein Out, das dir nur ein Paar bringt, ist gegen starke Gegner oft keins."] },
    ],
    related: ["regel-2-4", "pot-odds", "equity"],
    aliases: ["draw", "flushdraw", "straßendraw"],
  },
  {
    id: "regel-2-4",
    title: "Regel 2/4",
    unit: "mathe",
    level: 1,
    heuristic: true,
    oneLiner: "Outs mal 2 ergibt grob die Chance für eine Karte, Outs mal 4 für zwei Karten.",
    blocks: [
      { kind: "widget", widget: "rule-24", caption: "Faustregel gegen die exakte Kurve: Ab etwa 9 Outs schätzt ×4 zu hoch." },
      {
        kind: "formula",
        name: "Schätzung vom Flop bis zum River",
        num: "Outs × 4",
        heuristic: true,
        example: { given: "9 Outs, zwei Karten kommen", calc: "9 × 4 = 36 %", result: `exakt ${pct(flopToRiver(9))}` },
      },
      { kind: "points", items: ["Nur eine Karte kommt (Turn oder River)? Dann Outs × 2.", "Bei mehr als 8 Outs ziehst du vom ×4-Wert so viel ab, wie du über 8 liegst."] },
    ],
    related: ["outs", "pot-odds", "semi-bluff"],
    aliases: ["2/4", "4/2", "wahrscheinlichkeit"],
  },
  {
    id: "pot-odds",
    title: "Pot Odds",
    unit: "mathe",
    level: 1,
    oneLiner: "Der Preis eines Calls zeigt dir, wie oft du mindestens gewinnen musst, damit er sich lohnt.",
    blocks: [
      { kind: "widget", widget: "pot-odds", props: { mode: "call" }, caption: "Zieh die Bet-Größe: Je größer die Bet, desto mehr Equity brauchst du." },
      {
        kind: "formula",
        name: "Benötigte Equity",
        num: "Bet",
        den: "Pot + 2 × Bet",
        example: { given: "Pot 100, Gegner setzt 50", calc: "50 ÷ (100 + 100)", result: pct(potOdds(100, 50)) },
      },
      { kind: "points", items: ["Hast du mehr Equity als nötig, ist der Call auf lange Sicht ein Gewinn.", "Der Pot im Nenner enthält die Bet des Gegners und deinen Call."] },
    ],
    related: ["equity", "regel-2-4", "implizite-odds"],
    aliases: ["odds", "call", "preis"],
  },
  {
    id: "equity",
    title: "Equity",
    unit: "mathe",
    level: 1,
    oneLiner: "Dein Anteil am Pot, wenn alle restlichen Karten jetzt aufgedeckt würden.",
    blocks: [
      {
        kind: "matchup",
        hero: { label: "Du", cards: ["Ah", "Kh"] },
        villain: { label: "Gegner", cards: ["Qc", "Jd"] },
        board: ["Qh", "7h", "2c"],
        caption: "Alle möglichen Turn- und River-Karten durchgezählt.",
      },
      { kind: "points", items: ["Du liegst noch hinten und bist trotzdem Favorit: Jedes Herz, Ass und jeder König hilft dir.", "Equity sagt, wie viel vom Pot dir im Schnitt gehört, nicht ob du die Hand gewinnst."] },
    ],
    related: ["pot-odds", "range", "equity-realisierung"],
    aliases: ["gewinnchance", "anteil"],
  },
  {
    id: "range",
    title: "Range",
    unit: "mathe",
    level: 2,
    oneLiner: "Du kennst die Karten des Gegners nicht. Also denkst du an alle Hände, die er hier plausibel spielt.",
    blocks: [
      {
        kind: "matrix",
        range: [
          ...pairs("2", "A"),
          ...run("A", "2", "K", "s"),
          ...run("K", "9", "Q", "s"),
          ...run("Q", "9", "J", "s"),
          "JTs",
          "T9s",
          "98s",
          "87s",
          ...run("A", "T", "K", "o"),
          "KQo",
          "KJo",
        ],
        caption: "Beispiel-Range für eine Eröffnung aus mittlerer Position. Eine Annahme, keine Lösung.",
      },
      { kind: "points", items: ["Oben rechts stehen suited Hände, unten links offsuit, auf der Diagonale Paare.", "Jede Aktion des Gegners streicht Hände aus seiner Range."] },
    ],
    related: ["kombinatorik", "blocker", "equity"],
    aliases: ["handbereich", "matrix"],
  },
  {
    id: "implizite-odds",
    title: "Implizite Odds",
    unit: "mathe",
    level: 2,
    heuristic: true,
    oneLiner: "Ein Call kann sich auch ohne passende Pot Odds lohnen, wenn du nach dem Treffer noch viel gewinnst.",
    blocks: [
      {
        kind: "bars",
        rows: [
          { label: "Pot jetzt", value: 5, display: "5 bb" },
          { label: "Dein Call", value: 2, display: "2 bb", tone: "neg" },
          { label: "Später nötig", value: impliedNeeded, display: `${num(impliedNeeded)} bb`, tone: "amber" },
        ],
        caption: `Kleines Paar sucht ein Set. Trefferchance auf dem Flop: ${pct(set)}.`,
      },
      {
        kind: "formula",
        name: "Später mindestens zu gewinnen",
        num: "Call × (1 − Chance)",
        den: "Chance",
        after: "− Pot",
        example: { given: `Call 2 bb, Pot 5 bb, Chance ${pct(set)}`, calc: "2 × 0,882 ÷ 0,118 − 5", result: `${num(impliedNeeded)} bb` },
      },
      { kind: "avoid", items: ["Wenn der Gegner kurz ist: Was er nicht hat, kann er dir nicht zahlen.", "Mit dominierten Händen: Du triffst und verlierst trotzdem mehr."] },
    ],
    related: ["pot-odds", "effektiver-stack", "spr"],
    aliases: ["set mining", "implied odds"],
  },
  {
    id: "ev",
    title: "Erwartungswert (EV)",
    unit: "mathe",
    level: 2,
    oneLiner: "Was eine Entscheidung im Schnitt bringt, wenn du sie sehr oft triffst.",
    blocks: [
      {
        kind: "tree",
        root: "Call 10 in einen Pot von 30",
        branches: [
          { label: "Du gewinnst", p: 0.3, result: 30 },
          { label: "Du verlierst", p: 0.7, result: -10 },
        ],
        unit: "Chips",
        caption: "Jeder Ast: Chance mal Ergebnis. Die Summe ist der EV.",
      },
      { kind: "formula", name: "EV", num: "Summe aus Chance × Ergebnis", example: { given: "30 % gewinnen 30, 70 % verlieren 10", calc: "9 − 7", result: `${num(evCall)} Chips` } },
      { kind: "points", items: ["Positiver EV heißt: Auf lange Sicht verdienst du mit dieser Entscheidung.", "Eine einzelne Hand sagt fast nichts über den EV."] },
    ],
    related: ["entscheidung-ergebnis", "fold-equity", "pot-odds"],
    aliases: ["erwartungswert", "expected value"],
  },

  /* Preflop */
  {
    id: "kombinatorik",
    title: "Kombinatorik",
    unit: "preflop",
    level: 2,
    oneLiner: "Ein Paar gibt es auf 6 Arten, eine suited Hand auf 4, eine offsuit Hand auf 12.",
    blocks: [
      { kind: "widget", widget: "combos", caption: "Wähl einen Handtyp. Jedes Feld ist eine echte Kartenkombination." },
      { kind: "points", items: ["AK kommt mit 16 Kombos fast dreimal so oft vor wie AA mit 6.", "Insgesamt gibt es 1.326 Starthände."] },
    ],
    related: ["blocker", "range"],
    aliases: ["kombos", "combos"],
  },
  {
    id: "blocker",
    title: "Blocker",
    unit: "preflop",
    level: 3,
    oneLiner: "Hältst du eine Karte, kann der Gegner sie nicht haben. Das macht manche seiner Hände seltener.",
    blocks: [
      { kind: "widget", widget: "combos", props: { hand: "AK", blocked: true }, caption: "Ein Pik-Ass in deiner Hand streicht alle Kombos mit diesem Ass." },
      { kind: "points", items: ["Ein Ass in deiner Hand halbiert die Asse-Kombos des Gegners.", "Gute Bluffs halten oft Karten, die die starken Hände des Gegners blocken."] },
    ],
    related: ["kombinatorik", "value-bluff"],
    aliases: ["blocken", "removal"],
  },

  /* Position und Initiative */
  {
    id: "position",
    title: "Position",
    unit: "grundlagen",
    level: 1,
    oneLiner: "Wer später dran ist, hat schon gesehen, was die anderen tun, und entscheidet mit mehr Wissen.",
    blocks: [
      { kind: "widget", widget: "position", caption: "Tipp einen Sitz an. Preflop und postflop gilt eine andere Reihenfolge." },
      { kind: "points", items: ["Preflop handeln die Blinds zuletzt, ab dem Flop zuerst.", "Der Button handelt ab dem Flop immer zuletzt: die beste Position."] },
    ],
    related: ["equity-realisierung", "c-bet-warum", "steal"],
    aliases: ["sitz", "button", "utg", "cutoff"],
  },
  {
    id: "equity-realisierung",
    title: "Equity-Realisierung",
    unit: "position",
    level: 3,
    heuristic: true,
    oneLiner: "Nicht jede Hand kann ihre rechnerische Gewinnchance auch nutzen, vor allem nicht ohne Position.",
    blocks: [
      {
        kind: "bars",
        schematic: true,
        rows: [
          { label: "Rohe Equity", value: 1, display: "" },
          { label: "Mit Position", value: 0.92, display: "" },
          { label: "Ohne Position", value: 0.72, display: "" },
        ],
        caption: "Schema ohne echte Werte: Wer ohne Position spielt, muss öfter vor dem Showdown aufgeben.",
      },
      { kind: "points", items: ["Suited und verbundene Hände realisieren mehr als lose Offsuit-Hände.", "Genaue Zahlen liefert nur ein Solver. Hier zählt die Richtung."] },
    ],
    related: ["position", "equity"],
  },
  {
    id: "c-bet-warum",
    title: "Warum macht man eine C-Bet?",
    unit: "position",
    level: 2,
    oneLiner: "Wer preflop erhöht hat, hält im Schnitt die stärkeren Hände und trifft viele Flops besser.",
    blocks: [
      { kind: "widget", widget: "cbet", caption: "Wähl einen Flop: Wer hält öfter Top Pair oder besser?" },
      { kind: "points", items: ["Auf hohen, trockenen Flops liegt der Raiser vorne: Hier lohnt oft eine kleine C-Bet.", "Auf tiefen, verbundenen Flops holt der Caller auf: öfter checken."] },
      { kind: "avoid", items: ["Gegen mehrere Gegner: Einer trifft fast immer.", "Gegen jemanden, der kaum foldet."] },
    ],
    related: ["cbet", "fold-cbet", "position"],
    aliases: ["c-bet", "continuation bet", "initiative", "range-vorteil"],
  },

  /* Postflop */
  {
    id: "fold-equity",
    title: "Fold Equity",
    unit: "postflop",
    level: 2,
    oneLiner: "Der Gewinn, der entsteht, weil der Gegner aufgibt, obwohl er vielleicht vorne liegt.",
    blocks: [
      {
        kind: "tree",
        root: "Bluff: 6 in einen Pot von 10",
        branches: [
          { label: "Gegner foldet", p: 0.45, result: 10 },
          { label: "Gegner callt", p: 0.55, result: -6 },
        ],
        unit: "Chips",
        caption: "Selbst ohne Chance am Showdown kann eine Bet Gewinn bringen.",
      },
      {
        kind: "formula",
        name: "Nötige Fold-Quote",
        num: "Bet",
        den: "Pot + Bet",
        example: { given: "Bet 6, Pot 10", calc: "6 ÷ 16", result: pct(alpha(10, 6)) },
      },
    ],
    related: ["alpha", "semi-bluff", "ev"],
    aliases: ["fe", "bluff"],
  },
  {
    id: "semi-bluff",
    title: "Semi-Bluff",
    unit: "postflop",
    level: 2,
    oneLiner: "Du setzt mit einer Hand, die noch hinten liegt, aber besser werden kann.",
    blocks: [
      {
        kind: "cards",
        rows: [{ label: "Du", cards: ["Jh", "Th"], tag: "Flush-Draw" }],
        board: ["Ah", "8h", "4c", "2s"],
        highlight: ["Jh", "Th", "Ah", "8h"],
        caption: "Turn: 9 Herz bleiben, 46 Karten sind unbekannt.",
      },
      {
        kind: "tree",
        root: "Bet 7 in einen Pot von 10",
        branches: [
          { label: "Gegner foldet", p: 0.3, result: 10 },
          { label: "Call, Herz kommt", p: 0.7 * semiEq, result: 17 },
          { label: "Call, kein Herz", p: 0.7 * (1 - semiEq), result: -7 },
        ],
        unit: "Chips",
        caption: "Annahme: Der Gegner foldet 30 %. Zwei Wege zum Gewinn.",
      },
      { kind: "points", items: ["Ohne Draw wäre dieselbe Bet ein reiner Bluff und bräuchte mehr Folds.", "Faustregel: Je mehr Outs, desto eher setzt du, statt zu checken."] },
    ],
    related: ["fold-equity", "outs", "value-bluff"],
    aliases: ["draw", "semibluff"],
  },
  {
    id: "value-bluff",
    title: "Value-Bet und Bluff",
    unit: "postflop",
    level: 2,
    heuristic: true,
    oneLiner: "Eine Value-Bet soll von schlechteren Händen gecallt werden, ein Bluff bessere Hände zum Folden bringen.",
    blocks: [
      {
        kind: "cards",
        board: ["Kh", "9c", "4d", "2s", "7h"],
        rows: [
          { label: "Value", cards: ["Kd", "Qs"], tag: "Schlechtere Könige zahlen" },
          { label: "Checken", cards: ["9h", "8h"], tag: "Zu schwach zum Setzen, zu stark zum Bluffen" },
          { label: "Bluff", cards: ["6c", "5c"], tag: "Verpasster Draw" },
        ],
        caption: "Derselbe River, drei Pläne.",
      },
      { kind: "points", items: ["Große Bets bestehen vor allem aus sehr starken Händen und Bluffs.", "Mittlere Hände checken lieber und gewinnen am Showdown."] },
    ],
    related: ["alpha", "mdf", "blocker"],
    aliases: ["value", "bluff", "polarisierung"],
  },
  {
    id: "mdf",
    title: "Minimum Defense Frequency",
    unit: "postflop",
    level: 3,
    oneLiner: "So viel deiner Range muss weiterspielen, damit reine Bluffs des Gegners kein Geld verdienen.",
    blocks: [
      { kind: "widget", widget: "pot-odds", props: { mode: "mdf" }, caption: "Je größer die Bet, desto weniger musst du verteidigen." },
      { kind: "formula", name: "MDF", num: "Pot", den: "Pot + Bet", example: { given: "Pot 100, Bet 75", calc: "100 ÷ 175", result: pct(mdf(100, 75)) } },
      { kind: "avoid", items: ["Gegen Spieler, die kaum bluffen: Dann darfst du deutlich mehr folden."] },
    ],
    related: ["alpha", "pot-odds", "value-bluff"],
    aliases: ["verteidigen", "defense"],
  },
  {
    id: "alpha",
    title: "Alpha",
    unit: "postflop",
    level: 3,
    oneLiner: "So oft muss ein Bluff funktionieren, damit er sich lohnt.",
    blocks: [
      { kind: "widget", widget: "pot-odds", props: { mode: "alpha" }, caption: "Ein kleiner Bluff muss seltener klappen als ein großer." },
      { kind: "formula", name: "Alpha", num: "Bet", den: "Pot + Bet", example: { given: "Pot 100, Bet 50", calc: "50 ÷ 150", result: pct(alpha(100, 50)) } },
    ],
    related: ["mdf", "fold-equity", "value-bluff"],
    aliases: ["bluff", "break-even"],
  },
  {
    id: "spr",
    title: "Stack-to-Pot-Ratio",
    unit: "postflop",
    level: 2,
    heuristic: true,
    oneLiner: "Restlicher Stack geteilt durch den Pot. Ist die Zahl klein, bist du mit guten Händen schnell gebunden.",
    blocks: [
      {
        kind: "bars",
        rows: [
          { label: "Pot am Flop", value: 10, display: "10 bb" },
          { label: "Effektiver Stack", value: 25, display: "25 bb" },
        ],
        caption: "SPR 2,5: Ein Top Pair geht hier meist bis zum All-in.",
      },
      { kind: "formula", name: "SPR", num: "Effektiver Stack", den: "Pot", example: { given: "Stack 25 bb, Pot 10 bb", calc: "25 ÷ 10", result: "2,5" } },
      { kind: "points", items: ["Faustregel: Bis etwa 3 ist Top Pair stark genug für alles.", "Über etwa 10 brauchst du für ein All-in mehr als ein Paar."] },
    ],
    related: ["effektiver-stack", "implizite-odds"],
    aliases: ["commitment"],
  },

  /* Turnier */
  {
    id: "effektiver-stack",
    title: "Effektiver Stack",
    unit: "turnier",
    level: 1,
    oneLiner: "Es geht nie um mehr als den kleineren der beiden Stacks. Nur der zählt.",
    blocks: [
      {
        kind: "bars",
        rows: [
          { label: "Dein Stack", value: 45, display: "45 bb" },
          { label: "Gegner", value: 12, display: "12 bb" },
          { label: "Effektiv", value: 12, display: "12 bb", tone: "amber" },
        ],
        caption: "Du kannst diesem Gegner höchstens 12 bb abnehmen oder an ihn verlieren.",
      },
      { kind: "points", items: ["Plane jede Hand mit dem effektiven Stack, nicht mit deinem eigenen.", "Wer dich covert, kann dich aus dem Turnier werfen."] },
    ],
    related: ["big-blinds", "spr", "push-fold"],
    aliases: ["covern", "effective stack"],
  },
  {
    id: "m-ratio",
    title: "M-Ratio und Stacktiefe",
    unit: "turnier",
    level: 2,
    heuristic: true,
    oneLiner: "M zeigt, wie viele Runden du überlebst, ohne zu spielen. Die Zonen sind eine Faustregel nach Harrington.",
    blocks: [
      { kind: "widget", widget: "stack", props: { mode: "m" }, caption: "Antes zählen bei M voll mit. Darum fällt M schneller als die Big Blinds." },
      { kind: "formula", name: "M", num: "Stack", den: "Small Blind + Big Blind + alle Antes", example: { given: "12.000 Chips, 200/400, 8 × 50 Ante", calc: "12.000 ÷ 1.000", result: "M 12" } },
      { kind: "points", items: ["Heute rechnen die meisten in Big Blinds. M hilft, wenn Antes groß sind."] },
    ],
    related: ["big-blinds", "push-fold"],
    aliases: ["m", "harrington", "zonen"],
  },

  /* Push/Fold */
  {
    id: "push-fold",
    title: "Push/Fold",
    unit: "pushfold",
    level: 2,
    oneLiner: "Bei sehr kurzem Stack bleibt oft nur All-in oder Fold. Das Gleichgewicht zeigt, welche Hände dafür reichen.",
    blocks: [
      {
        kind: "table",
        seats: 8,
        marks: [
          { pos: "SB", state: "allin", chip: "10 bb", note: "All-in" },
          { pos: "BB", state: "active", note: "Call oder Fold?" },
          ...["UTG", "UTG+1", "LJ", "HJ", "CO", "BTN"].map((pos) => ({ pos, state: "fold" as const })),
        ],
        caption: "Alle folden zum Small Blind. Er schiebt 10 bb.",
      },
      {
        kind: "formula",
        name: "EV eines Shoves",
        num: "Fold % × Pot + Call % × (Equity × Endpot − Einsatz)",
        example: { given: "Pot 1,5 bb, Einsatz 9,5 bb, Fold 60 %, Equity 40 %", calc: "0,9 + 0,4 × (8 − 9,5)", result: `${num(pfEv)} bb` },
      },
      { kind: "points", items: ["Unter etwa 10–12 bb ist für einen normalen Raise kein Platz mehr (Faustregel).", "Nash heißt: Keiner kann sich allein verbessern. Es ist kein Lesen des Gegners."] },
    ],
    related: ["effektiver-stack", "icm", "fold-equity"],
    aliases: ["shove", "all-in", "nash", "jam"],
  },

  /* ICM */
  {
    id: "icm",
    title: "ICM",
    unit: "icm",
    level: 3,
    oneLiner: "Im Turnier sind Chips nicht gleich Geld. ICM rechnet Chipstände in Anteile am Preisgeld um.",
    blocks: [
      {
        kind: "compare",
        legend: ["Anteil Chips", "Anteil Preisgeld"],
        rows: ICM_STACKS.map((s, i) => ({ label: `Spieler ${"ABC"[i]}`, a: s / 10000, b: icmNow[i] / 100 })),
        caption: "Drei Spieler, Auszahlung 50 / 30 / 20 %. Modell: Malmuth-Harville.",
      },
      { kind: "points", items: ["Der Chip Leader hat mehr Chips als Geldanteil, der Short Stack weniger.", "Verdoppeln bringt weniger, als Ausscheiden kostet."] },
      { kind: "avoid", items: ["ICM kennt keine Position und kein Können. Es ist ein Modell, keine Wahrheit."] },
    ],
    related: ["bubble-factor", "push-fold"],
    aliases: ["independent chip model", "preisgeld", "deal"],
  },
  {
    id: "bubble-factor",
    title: "Bubble Factor und Risk Premium",
    unit: "icm",
    level: 3,
    oneLiner: "Kurz vor dem Geld kostet ein Verlust mehr, als ein Gewinn bringt. Du brauchst mehr Equity für einen Call.",
    blocks: [
      {
        kind: "bars",
        rows: [
          { label: "Verlust bei Niederlage", value: bNow - bLose, display: `${num(bNow - bLose)} %`, tone: "neg" },
          { label: "Gewinn bei Sieg", value: bWin - bNow, display: `${num(bWin - bNow)} %`, tone: "pos" },
        ],
        caption: "Vier übrig, drei bezahlt. Du (3.000) callst das All-in des 4.000-Stacks. Werte in % des Preispools.",
      },
      {
        kind: "formula",
        name: "Benötigte Equity",
        num: "Bubble Factor",
        den: "1 + Bubble Factor",
        example: { given: `Bubble Factor ${num(bubbleFactor, 2)}`, calc: `${num(bubbleFactor, 2)} ÷ ${num(1 + bubbleFactor, 2)}`, result: `${pct(bubbleNeed)} statt 50 %` },
      },
      { kind: "points", items: ["Ohne ICM würden hier 50 % Equity für den Call reichen.", "Am meisten leiden mittlere Stacks, die von einem größeren angegriffen werden."] },
    ],
    related: ["icm", "push-fold"],
    aliases: ["bubble", "risk premium", "bf"],
  },

  /* Bounty */
  {
    id: "bounty-pot-odds",
    title: "Bounty-bereinigte Pot Odds",
    unit: "bounty",
    level: 2,
    oneLiner: "Im PKO zählt das Kopfgeld des Gegners wie zusätzliche Chips im Pot, wenn du ihn coverst.",
    blocks: [
      {
        kind: "bars",
        rows: [pctBar("Ohne Bounty", neededEquity(4000, 6000)), pctBar("Mit Bounty", neededEquity(4000, 8000), "pos")],
        caption: "Call 4.000 in einen Pot von 6.000. Annahme: Das Bounty ist 2.000 Chips wert.",
      },
      {
        kind: "formula",
        name: "Benötigte Equity",
        num: "Call",
        den: "Pot nach Call + Bounty in Chips",
        example: { given: "Call 4.000, Pot nach Call 10.000, Bounty 2.000", calc: "4.000 ÷ 12.000", result: pct(4000 / 12000) },
      },
      { kind: "avoid", items: ["Wenn der Gegner dich covert: Dann gewinnst du sein Bounty nicht, aber riskierst deins."] },
    ],
    related: ["pot-odds", "effektiver-stack"],
    aliases: ["pko", "bounty", "kopfgeld", "knockout"],
  },

  /* Mentales Spiel */
  {
    id: "entscheidung-ergebnis",
    title: "Entscheidung statt Ergebnis",
    unit: "mental",
    level: 1,
    oneLiner: "Ob du gut gespielt hast, misst du am EV, nicht daran, ob du die Hand gewonnen hast.",
    blocks: [
      {
        kind: "matchup",
        hero: { label: "Du", cards: ["9s", "9d"] },
        villain: { label: "Gegner", cards: ["As", "Ks"] },
        board: ["9c", "6s", "2s"],
        caption: "All-in am Flop mit einem Set. Du bist klarer Favorit und verlierst trotzdem manchmal.",
      },
      { kind: "points", items: ["Eine verlorene Hand kann richtig gespielt sein. Eine gewonnene falsch.", "Beurteile Hände nach Preis, Range und EV. Das schützt vor Tilt."] },
    ],
    related: ["ev", "stichprobe"],
    aliases: ["bad beat", "tilt", "varianz", "resulting"],
  },

  /* HUD */
  {
    id: "stichprobe",
    title: "Stichprobe: Wann du einer Zahl trauen kannst",
    unit: "hud",
    level: 2,
    oneLiner: "5 von 12 ist nicht dasselbe wie 50 von 120. Kleine Stichproben schwanken stark.",
    blocks: [
      {
        kind: "sample",
        rows: [
          { k: 5, n: 12 },
          { k: 50, n: 120 },
          { k: 500, n: 1200 },
        ],
        caption: "Alle drei zeigen 42 %. Der Balken zeigt, wo der wahre Wert mit 95 % Sicherheit liegt (Wilson-Intervall).",
      },
      { kind: "points", items: ["Solange der Balken breit ist, nur grob einordnen.", "Häufige Werte wie VPIP werden schneller verlässlich als seltene wie Fold auf 3-Bet."] },
    ],
    related: ["vpip", "pfr"],
    aliases: ["sample", "hände", "verlässlich"],
  },
  {
    id: "vpip",
    title: "VPIP",
    unit: "hud",
    level: 1,
    stat: "vpip",
    oneLiner: "Wie oft jemand freiwillig Geld in den Pot legt. Der Big Blind ohne Raise zählt nicht.",
    blocks: [
      { kind: "scale", stat: "vpip", max: 60, low: 20, high: 35, labels: ["Eng", "Normal", "Locker"], caption: "Schwellen und Farben aus deinen HUD-Einstellungen. Faustregel, je nach Format verschieden." },
      statFormula("VPIP", "Hände mit Call oder Raise preflop", "Alle Hände", 18, 80),
      { kind: "points", items: ["Hoher VPIP heißt: viele schwache Hände. Spiel mit Value gegen ihn.", "Zusammen mit PFR siehst du, ob er callt oder erhöht."] },
      { kind: "avoid", items: [SMALL_SAMPLE] },
    ],
    related: ["pfr", "stichprobe", "wtsd"],
    aliases: ["voluntarily put money in pot", "locker", "loose"],
  },
  {
    id: "pfr",
    title: "PFR",
    unit: "hud",
    level: 1,
    stat: "pfr",
    oneLiner: "Wie oft jemand vor dem Flop erhöht. Die Lücke zu VPIP zeigt, wie passiv er ist.",
    blocks: [
      { kind: "scale", stat: "pfr", max: 50, low: 12, high: 25, labels: ["Passiv", "Normal", "Aggressiv"], caption: "Schwellen und Farben aus deinen HUD-Einstellungen. Faustregel, je nach Format verschieden." },
      statFormula("PFR", "Hände mit Raise preflop", "Alle Hände", 13, 80),
      { kind: "points", items: ["VPIP 35 und PFR 8: callt viel, erhöht selten. Ein passiver Caller.", "Liegen beide nah beieinander, spielt er aggressiv."] },
      { kind: "avoid", items: [SMALL_SAMPLE] },
    ],
    related: ["vpip", "three-bet", "steal"],
    aliases: ["preflop raise", "raise"],
  },
  {
    id: "three-bet",
    title: "3-Bet",
    unit: "hud",
    level: 2,
    stat: "three_bet",
    oneLiner: "Wie oft jemand ein Open-Raise noch einmal erhöht.",
    blocks: [
      {
        kind: "table",
        seats: 8,
        marks: [
          { pos: "CO", state: "active", chip: "2,2 bb", note: "Open" },
          { pos: "BTN", state: "hero", chip: "7 bb", note: "3-Bet" },
          ...["UTG", "UTG+1", "LJ", "HJ"].map((pos) => ({ pos, state: "fold" as const })),
        ],
        caption: "Der Button erhöht das Open des Cutoffs: eine 3-Bet.",
      },
      { kind: "scale", stat: "three_bet", max: 20, low: 4, high: 10, labels: ["Selten", "Normal", "Oft"], caption: "Schwellen und Farben aus deinen HUD-Einstellungen. Faustregel, je nach Format verschieden." },
      statFormula("3-Bet", "3-Bets", "Gelegenheiten nach einem Open", 3, 40),
      { kind: "points", items: ["Wer selten 3-bettet, hat dann fast immer eine starke Hand.", "Wer oft 3-bettet, blufft auch. Gegen ihn kannst du weiter callen oder 4-betten."] },
    ],
    related: ["fold-three-bet", "pfr"],
    aliases: ["3bet", "reraise", "re-raise"],
  },
  {
    id: "fold-three-bet",
    title: "Fold auf 3-Bet",
    unit: "hud",
    level: 2,
    stat: "fold_to_three_bet",
    oneLiner: "Wie oft jemand nach seinem eigenen Raise auf eine 3-Bet aufgibt.",
    blocks: [
      { kind: "scale", stat: "fold_to_three_bet", max: 100, low: 45, high: 70, labels: ["Hält fest", "Normal", "Gibt oft auf"], caption: "Schwellen und Farben aus deinen HUD-Einstellungen. Faustregel, je nach Format verschieden." },
      statFormula("Fold auf 3-Bet", "Folds", "Male, die er eine 3-Bet bekam", 7, 10),
      { kind: "points", items: ["Hoher Wert: Gegen ihn lohnen auch leichte 3-Bets.", "Niedriger Wert: 3-bette ihn vor allem mit starken Händen."] },
      { kind: "avoid", items: ["Diese Situation ist selten. Der Wert braucht sehr viele Hände, bis er stimmt."] },
    ],
    related: ["three-bet", "stichprobe"],
    aliases: ["f3b", "fold to 3bet"],
  },
  {
    id: "steal",
    title: "Steal",
    unit: "hud",
    level: 1,
    stat: "steal",
    oneLiner: "Wie oft jemand aus Cutoff, Button oder Small Blind eröffnet, wenn alle vor ihm gefoldet haben.",
    blocks: [
      {
        kind: "table",
        seats: 8,
        marks: [
          { pos: "BTN", state: "hero", chip: "2 bb", note: "Steal" },
          { pos: "SB", state: "active" },
          { pos: "BB", state: "active" },
          ...["UTG", "UTG+1", "LJ", "HJ", "CO"].map((pos) => ({ pos, state: "fold" as const })),
        ],
        caption: "Alle folden bis zum Button. Er greift die Blinds an.",
      },
      { kind: "scale", stat: "steal", max: 80, low: 25, high: 45, labels: ["Zurückhaltend", "Normal", "Klaut viel"], caption: "Schwellen und Farben aus deinen HUD-Einstellungen. Faustregel, je nach Format verschieden." },
      statFormula("Steal", "Opens aus CO, BTN oder SB", "Male, die alle davor gefoldet haben", 9, 20),
      { kind: "points", items: ["Wer viel klaut, hat oft schwache Hände: Verteidige im Big Blind breiter.", "Wer kaum klaut, öffnet spät nur mit echten Händen."] },
    ],
    related: ["fold-steal", "position"],
    aliases: ["ats", "blinds klauen", "attempt to steal"],
  },
  {
    id: "fold-steal",
    title: "Fold auf Steal",
    unit: "hud",
    level: 1,
    stat: "fold_to_steal",
    oneLiner: "Wie oft die Blinds gegen einen späten Open aufgeben.",
    blocks: [
      { kind: "scale", stat: "fold_to_steal", max: 100, low: 50, high: 75, labels: ["Verteidigt", "Normal", "Gibt Blinds her"], caption: "Schwellen und Farben aus deinen HUD-Einstellungen. Faustregel, je nach Format verschieden." },
      statFormula("Fold auf Steal", "Folds im Blind", "Steals gegen ihn", 16, 20),
      { kind: "points", items: ["Hoher Wert: Öffne gegen diesen Blind sehr breit.", "Niedriger Wert: Steal mit Händen, die gecallt gut weiterspielen."] },
      { kind: "avoid", items: [SMALL_SAMPLE] },
    ],
    related: ["steal", "position"],
    aliases: ["fst", "fold to steal", "blinds verteidigen"],
  },
  {
    id: "cbet",
    title: "C-Bet",
    unit: "hud",
    level: 2,
    stat: "cbet",
    oneLiner: "Wie oft jemand am Flop weiter setzt, nachdem er preflop erhöht hat.",
    blocks: [
      { kind: "scale", stat: "cbet", max: 100, low: 45, high: 70, labels: ["Selten", "Normal", "Fast immer"], caption: "Schwellen und Farben aus deinen HUD-Einstellungen. Faustregel, je nach Format verschieden." },
      statFormula("C-Bet", "C-Bets am Flop", "Flops als Preflop-Raiser", 14, 20),
      { kind: "points", items: ["Sehr hoher Wert: Die Bet sagt wenig über seine Hand. Call oder Raise öfter.", "Niedriger Wert: Setzt er doch, hat er meist etwas."] },
    ],
    related: ["c-bet-warum", "fold-cbet"],
    aliases: ["continuation bet", "cb"],
  },
  {
    id: "fold-cbet",
    title: "Fold auf C-Bet",
    unit: "hud",
    level: 2,
    stat: "fold_to_cbet",
    oneLiner: "Wie oft jemand am Flop auf eine C-Bet aufgibt.",
    blocks: [
      { kind: "scale", stat: "fold_to_cbet", max: 100, low: 35, high: 60, labels: ["Hält fest", "Normal", "Gibt oft auf"], caption: "Schwellen und Farben aus deinen HUD-Einstellungen. Faustregel, je nach Format verschieden." },
      statFormula("Fold auf C-Bet", "Folds auf C-Bet", "C-Bets gegen ihn", 12, 20),
      { kind: "points", items: ["Hoher Wert: C-Bet gegen ihn auch ohne Treffer.", "Niedriger Wert: Setz vor allem mit Händen, die gecallt werden wollen."] },
    ],
    related: ["cbet", "c-bet-warum", "fold-equity"],
    aliases: ["fcb", "fold to cbet"],
  },
  {
    id: "wtsd",
    title: "WTSD",
    unit: "hud",
    level: 2,
    stat: "wtsd",
    oneLiner: "Wie oft jemand nach gesehenem Flop bis zum Showdown geht.",
    blocks: [
      { kind: "scale", stat: "wtsd", max: 60, low: 22, high: 32, labels: ["Gibt früh auf", "Normal", "Geht oft mit"], caption: "Schwellen und Farben aus deinen HUD-Einstellungen. Faustregel, je nach Format verschieden." },
      statFormula("WTSD", "Showdowns", "Gesehene Flops", 9, 30),
      { kind: "points", items: ["Hoher WTSD und niedriger W$SD: eine Calling Station. Nicht bluffen, dünn auf Value setzen.", "Niedriger WTSD: Er gibt oft auf. Bluffs funktionieren besser."] },
    ],
    related: ["wsd", "vpip"],
    aliases: ["went to showdown", "showdown", "calling station"],
  },
  {
    id: "wsd",
    title: "W$SD",
    unit: "hud",
    level: 2,
    stat: "wsd",
    oneLiner: "Wie viele seiner Showdowns jemand gewinnt.",
    blocks: [
      { kind: "scale", stat: "wsd", max: 100, low: 45, high: 55, labels: ["Zeigt oft Schwaches", "Normal", "Zeigt meist Starkes"], caption: "Schwellen und Farben aus deinen HUD-Einstellungen. Faustregel, je nach Format verschieden." },
      statFormula("W$SD", "Gewonnene Showdowns", "Alle Showdowns", 11, 20),
      { kind: "points", items: ["Hoher Wert: Wenn er bis zum River geht, hat er es meist. Gute Folds lohnen sich.", "Niedriger Wert: Er geht mit zu schwachen Händen mit."] },
      { kind: "avoid", items: ["Showdowns sind selten. Unter etwa 30 Showdowns sagt der Wert wenig."] },
    ],
    related: ["wtsd", "stichprobe"],
    aliases: ["won money at showdown", "wsd"],
  },
  {
    id: "af",
    title: "AF (Aggressionsfaktor)",
    unit: "hud",
    level: 2,
    stat: "af",
    oneLiner: "Wie viele Bets und Raises jemand nach dem Flop pro Call macht.",
    blocks: [
      { kind: "scale", stat: "af", max: 6, low: 1.5, high: 3.5, ratio: true, labels: ["Passiv", "Normal", "Sehr aggressiv"], caption: "Schwellen und Farben aus deinen HUD-Einstellungen. Faustregel, je nach Format verschieden." },
      { kind: "formula", name: "AF", num: "Bets + Raises", den: "Calls", example: { given: "20 Bets, 8 Raises, 10 Calls", calc: "28 ÷ 10", result: "2,8×" } },
      { kind: "points", items: ["Checks und Folds zählen nicht mit. Darum zeigt AF nicht, wie oft er aggressiv ist.", "Dafür gibt es AFq."] },
    ],
    related: ["afq", "pfr"],
    aliases: ["aggression", "aggressionsfaktor"],
  },
  {
    id: "afq",
    title: "AFq (Aggressions-Frequenz)",
    unit: "hud",
    level: 2,
    stat: "afq",
    oneLiner: "Welcher Anteil seiner Aktionen nach dem Flop aggressiv ist.",
    blocks: [
      { kind: "scale", stat: "afq", max: 100, low: 35, high: 55, labels: ["Passiv", "Normal", "Sehr aggressiv"], caption: "Schwellen und Farben aus deinen HUD-Einstellungen. Faustregel, je nach Format verschieden." },
      statFormula("AFq", "Bets + Raises", "Bets + Raises + Calls + Folds", 28, 60),
      { kind: "points", items: ["Anders als AF zählen hier auch Folds. Der Wert ist deshalb robuster.", "Hoher Wert: Er setzt oft. Lass ihn öfter bluffen und callen."] },
    ],
    related: ["af", "wtsd"],
    aliases: ["aggression frequency", "aggressions-frequenz"],
  },
];

export const ARTICLE_BY_ID: Record<string, Article> = Object.fromEntries(ARTICLES.map((a) => [a.id, a]));

/** Lower-case and fold umlauts so "strasse" finds "Straße". */
export function fold(text: string): string {
  return text.toLocaleLowerCase("de-DE").replace(/ä/g, "a").replace(/ö/g, "o").replace(/ü/g, "u").replace(/ß/g, "ss").replace(/[^a-z0-9$%/+ ]/g, " ");
}

export function searchText(a: Article): string {
  return fold([a.title, a.oneLiner, ...(a.aliases ?? [])].join(" "));
}

/** All prose a reader sees on the page, for the word budget. */
export function proseWords(a: Article): number {
  const parts: string[] = [a.oneLiner];
  for (const b of a.blocks) {
    if (b.kind === "points" || b.kind === "avoid") parts.push(...b.items);
    else if (b.kind === "note") parts.push(b.text);
    if ("caption" in b && b.caption) parts.push(b.caption);
  }
  return parts.join(" ").split(/\s+/).filter(Boolean).length;
}
