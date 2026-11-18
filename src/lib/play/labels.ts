// Words for the practice with whole hands: grades, choices, sources and the plain-language
// "why" after each move. Postflop reasons come from the solver page's explain.ts.
import { actionColors } from "../solver/actions";
import { lesson as postflopLesson } from "../solver/explain";
import type { SolverAction } from "../solver/types";
import { betterLine, mixText, playsLine, sameEv, type Who } from "./words";
import type { Choice, EndReason, Grade, HandView, Lesson, PlayAnte, PlayConfig, PlayMode, Source, Verdict } from "./types";

export const num = (x: number, d = 1) => x.toLocaleString("de-DE", { maximumFractionDigits: d });
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
  push_fold: "Kurze Stacks",
  postflop: "Nur Postflop",
};
export const ANTE_LABEL: Record<PlayAnte, string> = { none: "Ohne Ante", classic: "Ante für alle", big_blind: "BB-Ante" };

export function choiceLabel(c: Choice): { label: string; size: string } {
  const pct = c.pct_pot !== null ? ` · ${Math.round(c.pct_pot)} %` : "";
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
      return { label: "All-in", size: bbText(c.to_bb) };
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
  library: "Spot-Bibliothek",
  saved: "Gespeicherte Lösung",
};

/** The small line under every verdict: where the answer comes from and what it assumes. */
export function sourceLine(source: Source, config: PlayConfig, stack: number, exploit: number | null): string {
  const table = `${config.players} Spieler, ${num(stack, 0)} BB, ${ANTE_LABEL[config.ante]}`;
  switch (source) {
    case "model":
      return `Quelle: Preflop-Modell von Ringside (${table}) – ein Modell, kein exaktes Gleichgewicht.`;
    case "equilibrium":
      return `Quelle: Push/Fold-Gleichgewicht (${table}, Chip-EV).`;
    default: {
      const acc = exploit !== null ? `, Genauigkeit ${num(exploit, 1)} % des Pots` : "";
      return `Quelle: ${SOURCE_NAME[source]} (Heads-up, gewählte Größen${acc}).`;
    }
  }
}

export const SOURCE_SHORT = SOURCE_NAME;

const WHO: Record<Source, Who> = { model: "model", equilibrium: "equilibrium", solver: "solution", library: "solution", saved: "solution" };

/** The headline of a graded move: what was better and what it cost, the mix, or nothing. */
export function headlineOf(v: Verdict, cards: string[]): string | null {
  const j = v.judgement;
  if (!j || !v.strategy) return null;
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
  if (!v.strategy) {
    return "Diese Hand kommt in der Lösung hier gar nicht an – sie wäre vorher anders gespielt worden. Deshalb gibt es für diesen Zug keine Bewertung.";
  }
  const strategy = v.strategy;
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

export const END_TEXT: Record<EndReason, string> = {
  folded: "",
  showdown: "",
  left_line:
    "Hier wäre die Hand anders weitergegangen. Für diese Linie ist kein Spot vorbereitet – dein Zug ist trotzdem bewertet.",
  flop_unsolved: "Am Flop endet diese Hand: Es war noch kein passender Spot vorbereitet. Der Vorrat füllt sich im Hintergrund.",
  not_stored: "Ab hier ist in der gespeicherten Lösung nichts mehr hinterlegt, deshalb endet die Hand an dieser Stelle.",
};

export function resultLine(h: HandView): string {
  const o = h.outcome;
  if (!o) return "";
  if (o.reason !== "folded" && o.reason !== "showdown") return END_TEXT[o.reason];
  const net = o.net[h.hero];
  const won = o.winners.includes(h.hero);
  if (o.reason === "folded") {
    if (won) return `Alle anderen folden – du gewinnst den Pot (${signedBB(net)}).`;
    return h.events.some((e) => e.type === "action" && e.seat === h.hero && e.choice.kind === "fold")
      ? `Du foldest (${signedBB(net)}).`
      : `${h.positions[o.winners[0]]} gewinnt den Pot (${signedBB(net)}).`;
  }
  const made = o.made[h.hero];
  if (won && o.winners.length > 1) return `Geteilter Pot${made ? ` mit ${made}` : ""} (${signedBB(net)}).`;
  if (won) return `Du gewinnst beim Showdown${made ? ` mit ${made}` : ""} (${signedBB(net)}).`;
  const w = o.winners[0];
  return `${h.positions[w]} gewinnt${o.made[w] ? ` mit ${o.made[w]}` : ""} (${signedBB(net)}).`;
}

export function etaText(secs: number | null): string {
  if (secs === null || !Number.isFinite(secs)) return "";
  if (secs < 10) return "gleich fertig";
  if (secs < 60) return `ca. ${Math.ceil(secs / 5) * 5} s`;
  return `ca. ${Math.ceil(secs / 60)} min`;
}

export function configLine(c: PlayConfig): string {
  const stack = c.mixed_stacks ? "gemischte Stacks" : bbText(c.stack_bb, 0);
  return `${MODE_LABEL[c.mode]} · ${c.players}-max · ${stack} · ${ANTE_LABEL[c.ante]}`;
}
