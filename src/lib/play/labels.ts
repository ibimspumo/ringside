// Words for the practice with whole hands: grades, choices, sources and the plain-language
// "why" after each move. Postflop reasons come from the solver page's explain.ts.
import { madeHand } from "../learn/wissen/poker";
import { actionColors } from "../solver/actions";
import { lesson as postflopLesson } from "../solver/explain";
import type { SolverAction } from "../solver/types";
import { betterLine, infinitive, mixText, playsLine, sameEv, type Who } from "./words";
import type { Choice, FreeInfo, Grade, HandView, Lesson, PlayAnte, PlayConfig, PlayMode, SolveWhy, Source, Unrated, Verdict } from "./types";

// Float noise first (6.05 − 2.2 is 3.8499…), so one amount reads the same wherever it is written.
export const num = (x: number, d = 1) => Number(x.toFixed(9)).toLocaleString("de-DE", { maximumFractionDigits: d });
export const bbText = (x: number, d = 1) => `${num(x, d)} BB`;
export const signedBB = (x: number) => `${x > 0.04 ? "+" : x < -0.04 ? "−" : "±"}${num(Math.abs(x))} BB`;

export const GRADE: Record<Grade, { label: string; color: string; lead: string; mark: string }> = {
  best: { label: "Beste Wahl", color: "var(--grade-best)", lead: "Genau so spielt es die Lösung.", mark: "★" },
  mix: { label: "Auch richtig (Mix)", color: "var(--grade-mix)", lead: "Gleich viel wert wie der Hauptzug – volle Punkte.", mark: "≈" },
  good: { label: "Gut", color: "var(--grade-good)", lead: "Gut gespielt – fast so viel wert wie die beste Wahl.", mark: "✓" },
  inaccurate: { label: "Ungenau", color: "var(--grade-inaccurate)", lead: "Knapp daneben, das kostet ein wenig.", mark: "!" },
  mistake: { label: "Fehler", color: "var(--grade-mistake)", lead: "Hier lässt du spürbar etwas liegen.", mark: "✕" },
  blunder: { label: "Grober Fehler", color: "var(--grade-blunder)", lead: "Das wird teuer – schau dir das Warum an.", mark: "✕✕" },
};
export const GRADES: Grade[] = ["best", "mix", "good", "inaccurate", "mistake", "blunder"];
/** In % of the pot at the decision; crates/ringside-play/src/grade.rs decides with the same numbers. */
export const THRESHOLD_TEXT: Record<Grade, string> = {
  best: "der Zug, den die Lösung mit deiner Hand am häufigsten spielt, und er kostet höchstens 0,5 % des Pots",
  mix: "die Lösung spielt den Zug mit deiner Hand mindestens jedes zehnte Mal, und er kostet höchstens 0,5 % des Pots",
  good: "kostet höchstens 2 % des Pots",
  inaccurate: "kostet 2 bis 6 % des Pots",
  mistake: "kostet 6 bis 15 % des Pots",
  blunder: "kostet mehr als 15 % des Pots – als würdest du jeden siebten Pot verschenken",
};

export const MODE_LABEL: Record<PlayMode, string> = {
  full: "Ganze Hände",
  push_fold: "Wenige Chips",
  postflop: "Ab dem Flop",
};
export const ANTE_LABEL: Record<PlayAnte, string> = { none: "Ohne Ante", classic: "Ante für alle", big_blind: "BB-Ante" };

/** `withPct: false` leaves the share of the pot out, e.g. before the flop where sizes read in BB. */
export function choiceLabel(c: Choice, withPct = true): { label: string; size: string } {
  const pct = withPct && c.pct_pot !== null ? ` · ${Math.round(c.pct_pot)} % Pot` : "";
  switch (c.kind) {
    case "fold":
      return { label: "Fold", size: "" };
    case "check":
      return { label: "Check", size: "" };
    case "call":
      return { label: "Call", size: bbText(c.add_bb) };
    case "limp":
      return { label: "Limp", size: bbText(c.add_bb) };
    case "bet":
      return { label: "Bet", size: `${bbText(c.to_bb)}${pct}` };
    case "raise":
      return { label: "Raise", size: `auf ${bbText(c.to_bb)}${pct}` };
    case "all_in":
      return { label: "All-in", size: c.to_bb > c.add_bb + 0.005 ? `auf ${bbText(c.to_bb)}` : bbText(c.to_bb) };
  }
}

/** Short form for the hand's timeline, e.g. "Raise 2,2", "Call", "All-in 40". */
export function choiceShort(c: Choice): string {
  switch (c.kind) {
    case "fold":
      return "Fold";
    case "check":
      return "Check";
    case "call":
      return "Call";
    case "limp":
      return "Limp";
    case "bet":
      return `Bet ${num(c.to_bb)}`;
    case "raise":
      return `Raise ${num(c.to_bb)}`;
    case "all_in":
      return `All-in ${num(c.to_bb)}`;
  }
}

function asSolverAction(c: Choice): SolverAction {
  return { kind: c.kind === "limp" ? "call" : c.kind, amount: c.to_bb, frequency: 0, combos: 0 };
}

export const choiceColors = (choices: Choice[]) => actionColors(choices.map(asSolverAction));

const SOURCE_NAME: Record<Source, string> = {
  model: "Preflop-Modell",
  equilibrium: "Push/Fold-Gleichgewicht",
  solver: "Solver",
  live: "Solver, live gerechnet",
  library: "Spot-Bibliothek",
  saved: "Gespeicherte Lösung",
  estimate: "Einschätzung",
};

/** Why a move has no grade, in one sentence. */
export const UNRATED_TEXT: Record<Unrated, string> = {
  not_reached: "Diese Hand kommt in der Lösung hier gar nicht an – sie wäre vorher anders gespielt worden. Deshalb gibt es für diesen Zug keine Bewertung.",
  multiway: "Nicht bewertet: Mehrweg-Pot, und du hattest hier nur eine Möglichkeit.",
  off_model: "Nicht bewertet: Diese Linie kennt das Preflop-Modell nicht. Es lässt hier nur zwei Spieler weiterspielen; du entscheidest trotzdem selbst.",
  no_solve: "Nicht bewertet: Der Spot wurde nicht fertig gerechnet.",
};

/** The short tag in lists. */
export const UNRATED_SHORT: Record<Unrated, string> = {
  not_reached: "nicht bewertet",
  multiway: "nicht bewertet: Mehrweg-Pot",
  off_model: "nicht bewertet: Linie ohne Modell",
  no_solve: "nicht bewertet: nicht gerechnet",
};

/** The small line under every verdict: where the answer comes from and what it assumes. */
export function sourceLine(source: Source | null, config: PlayConfig, stack: number, exploit: number | null): string {
  const table = `${config.players} Spieler, ${num(stack, 0)} BB, ${ANTE_LABEL[config.ante]}`;
  switch (source) {
    case null:
      return "Quelle: keine – dieser Zug wird nicht bewertet.";
    case "model":
      return `Quelle: Preflop-Modell von Ringside (${table}) – ein Modell, kein exaktes Gleichgewicht.`;
    case "equilibrium":
      return `Quelle: Push/Fold-Gleichgewicht (${table}, Chip-EV).`;
    case "estimate":
      return "Quelle: Einschätzung (Equity gegen wahrscheinliche Ranges gegen Pot Odds).";
    case "live":
      return `Quelle: Solver, live gerechnet (Heads-up, gewählte Größen) – schnell gerechnet${exploit !== null ? `, Genauigkeit ${num(exploit, 1)} % des Pots` : ""}.`;
    default: {
      const acc = exploit !== null ? `, Genauigkeit ${num(exploit, 1)} % des Pots` : "";
      return `Quelle: ${SOURCE_NAME[source]} (Heads-up, gewählte Größen${acc}).`;
    }
  }
}

const WHO: Record<Source, Who> = {
  model: "model",
  equilibrium: "equilibrium",
  solver: "solution",
  live: "solution",
  library: "solution",
  saved: "solution",
  estimate: "solution",
};

/** What the table says while a spot is solved live. */
export function solvingLine(why: SolveWhy): string {
  switch (why) {
    case "new_line":
      return "Neue Linie – Ringside rechnet den Spot jetzt.";
    case "no_spot":
      return "Für diesen Flop war kein Spot vorbereitet – Ringside rechnet ihn jetzt.";
    case "deeper":
      return "Die gespeicherte Lösung endet hier – Ringside rechnet den Spot zu Ende.";
  }
}

/** The banner once a hand plays on without a solution. */
export function freeLine(f: FreeInfo): string {
  const why =
    f.why === "multiway"
      ? "Mehrweg-Pot: Für drei Spieler nach dem Flop gibt es keinen Solver. Deine Züge bewertet hier die Einschätzung – eine Richtung, nicht exakt."
      : f.why === "off_model"
        ? "Diese Linie kennt das Preflop-Modell nicht. Vor dem Flop werden deine Züge nicht bewertet, danach bewertet sie die Einschätzung."
        : `Ohne fertige Lösung${f.note ? ` (${f.note})` : ""}: Deine Züge ab hier bewertet die Einschätzung – eine Richtung, nicht exakt.`;
  return `${why} ${f.rule}`;
}

/** The headline of a graded move: what was better and what it cost, the mix, or nothing. */
export function headlineOf(v: Verdict, cards: string[]): string | null {
  const j = v.judgement;
  if (!j || !v.strategy || !v.source) return null;
  const chosen = v.choices[v.chosen];
  if (j.grade === "mix") {
    const main = v.choices[j.main];
    const ev = (k: number) => (v.evs ? v.evs[k] : null);
    return mixText(WHO[v.source], handName(v, cards), { kind: main.kind, f: v.strategy[j.main], ev: ev(j.main) }, { kind: chosen.kind, f: v.strategy[v.chosen], ev: ev(v.chosen) });
  }
  if ((j.grade === "mistake" || j.grade === "blunder") && j.loss_bb !== null && j.loss_pct !== null) {
    return betterLine(v.choices[j.best].kind, chosen.kind, j.loss_bb, j.loss_pct);
  }
  return null;
}

/** Hand class preflop, the exact cards after the flop (strategies there are per combo). */
function handName(v: Verdict | Lesson, cards: string[]): string {
  return v.street === "preflop" ? v.class : cards.map(cardText).join("");
}

/** The plain-language reason for a verdict: what the solution does with this hand, and why. */
export function reason(v: Verdict | Lesson, chosen: number | null, cards: string[]): string {
  if (!v.strategy || !v.source) return UNRATED_TEXT[v.unrated ?? "not_reached"];
  const strategy = v.strategy;
  if (v.source === "estimate") {
    // The estimate rates the kind of move, not its size; its reason stands in "Warum?".
    const kinds = [...new Set(v.choices.filter((_, k) => strategy[k] > 0).map((c) => cap(infinitive(c.kind))))];
    return `Nach der Einschätzung ist hier ${kinds.join(" oder ")} am besten; die Größe bewertet sie nicht.`;
  }
  const best = v.evs ? v.evs.indexOf(Math.max(...v.evs)) : strategy.indexOf(Math.max(...strategy));
  if (v.node) {
    const hand = v.node.hands.find((h) => sameCards(h.combo, cards));
    if (hand && hand.evs.length === strategy.length && chosen !== null && chosen !== best && !sameEv(v.evs?.[best] ?? 0, v.evs?.[chosen] ?? 0)) {
      return `${postflopLesson(v.node, hand, best)} ${playsLine(WHO[v.source], handName(v, cards), strategy.map((f, k) => ({ kind: v.choices[k].kind, f })))}`;
    }
  }
  const plays = playsLine(WHO[v.source], handName(v, cards), strategy.map((f, k) => ({ kind: v.choices[k].kind, f })));
  if (chosen === null || chosen === best || !v.evs || sameEv(v.evs[best], v.evs[chosen])) return plays;
  const pot = v.pot;
  const odds =
    v.to_call > 0
      ? ` Für einen Call zahlst du ${bbText(v.to_call)} in einen Pot von ${bbText(pot + v.to_call)} – du müsstest dafür rund ${Math.round((v.to_call / (pot + v.to_call)) * 100)} % der Zeit gewinnen.`
      : "";
  return `${plays}${odds}`;
}

const SUIT: Record<string, string> = { s: "♠", h: "♥", d: "♦", c: "♣" };
/** "As" → "A♠", "Td" → "10♦". */
export const cardText = (c: string) => `${c[0] === "T" ? "10" : c[0].toUpperCase()}${SUIT[c[1]?.toLowerCase()] ?? ""}`;

function sameCards(combo: string, cards: string[]): boolean {
  const a = combo.slice(0, 2);
  const b = combo.slice(2, 4);
  return (a === cards[0] && b === cards[1]) || (a === cards[1] && b === cards[0]);
}

export const cap = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);

/** A player's hand at the showdown in words, from the cards where they are known. */
export function madeOf(h: HandView, seat: number): string | null {
  const cards = seat === h.hero ? h.hero_cards : h.outcome?.shown[seat];
  if (cards && h.board.length === 5) return madeHand([...cards, ...h.board]);
  return h.outcome?.made[seat] ?? null;
}

/** What the hand brought you; `withNet: false` leaves the amount out where it stands beside the line. */
export function resultLine(h: HandView, withNet = true): string {
  const o = h.outcome;
  if (!o) return "";
  const net = o.net[h.hero];
  const amount = withNet ? ` (${signedBB(net)})` : "";
  const won = o.winners.includes(h.hero);
  if (o.reason === "folded") {
    if (won) return `Alle anderen folden – du gewinnst den Pot${amount}.`;
    return h.events.some((e) => e.type === "action" && e.seat === h.hero && e.choice.kind === "fold")
      ? `Du foldest${amount}.`
      : `${h.positions[o.winners[0]]} gewinnt den Pot${amount}.`;
  }
  const made = madeOf(h, h.hero);
  const folded = h.events.some((e) => e.type === "action" && e.seat === h.hero && e.choice.kind === "fold");
  if (folded) {
    const w = o.winners.map((s) => h.positions[s]).join(" und ");
    return `Du foldest${amount}. Ohne dich checken die anderen bis zum Showdown durch: ${w} ${o.winners.length > 1 ? "teilen" : "gewinnt"}.`;
  }
  if (won && o.winners.length > 1) return `Geteilter Pot${made ? ` mit ${made}` : ""}${amount}.`;
  if (won) return `Du gewinnst beim Showdown${made ? ` mit ${made}` : ""}${amount}.`;
  const w = o.winners[0];
  const theirs = madeOf(h, w);
  return `${h.positions[w]} gewinnt${theirs ? ` mit ${theirs}` : ""}${amount}.`;
}

export function etaText(secs: number | null): string {
  if (secs === null || !Number.isFinite(secs)) return "";
  if (secs < 10) return "gleich fertig";
  if (secs < 60) return `ca. ${Math.ceil(secs / 5) * 5} s`;
  return `ca. ${Math.ceil(secs / 60)} min`;
}

/** Table sizes of the practice; mirror `ringside_play::table::PLAYERS`. */
export const TABLE_SIZES = [2, 3, 4, 5, 6, 8, 9] as const;

/** "Heads-up", "4 Spieler", "9-max": full tables by their type, short ones by who is left. */
export const tableName = (n: number) => (n === 2 ? "Heads-up" : n <= 5 ? `${n} Spieler` : `${n}-max`);

export function configLine(c: PlayConfig): string {
  const stack = c.mixed_stacks ? "gemischte Stacks" : bbText(c.stack_bb, 0);
  return `${MODE_LABEL[c.mode]} · ${tableName(c.players)} · ${stack} · ${ANTE_LABEL[c.ante]}`;
}
