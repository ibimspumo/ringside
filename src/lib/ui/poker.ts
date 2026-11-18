// Shared poker vocabulary for the kit: action colours, decision grades, honesty sources.

export type ActionKind = "fold" | "check" | "call" | "bet" | "raise" | "allin" | "limp";

/** Bet and raise sizes darken in four steps, from small (1) to big (4). */
export type SizeStep = 1 | 2 | 3 | 4;

export const ACTION_NAME: Record<ActionKind, string> = {
  fold: "Fold",
  check: "Check",
  call: "Call",
  bet: "Bet",
  raise: "Raise",
  allin: "All-in",
  limp: "Limp",
};

/** CSS colour of an action; bets and raises by size step. */
export function actionColor(kind: ActionKind, step?: SizeStep): string {
  if (kind === "bet" || kind === "raise") return step ? `var(--act-bet-${step})` : kind === "bet" ? "var(--act-bet-2)" : "var(--act-raise)";
  return `var(--act-${kind})`;
}

/** Size steps for the aggressive actions of one node, smallest first; all-ins are always the darkest. */
export function sizeSteps(count: number): SizeStep[] {
  if (count <= 1) return [3];
  return Array.from({ length: count }, (_, k) => (1 + Math.round((k * 3) / (count - 1))) as SizeStep);
}

export type Grade = "best" | "mix" | "good" | "inaccurate" | "mistake" | "blunder";

export const GRADES: Grade[] = ["best", "mix", "good", "inaccurate", "mistake", "blunder"];

/**
 * `symbol` keeps the grades apart without colour: filled marks for best, mix, mistake and
 * blunder, outlined ones for good and inaccurate, and a distinct glyph in each.
 */
export const GRADE: Record<Grade, { label: string; short: string; symbol: string; lead: string; bad: boolean }> = {
  best: { label: "Beste Wahl", short: "Beste", symbol: "✓", lead: "Genau so spielt es die Lösung.", bad: false },
  mix: { label: "Auch richtig", short: "Mix", symbol: "≈", lead: "Die Lösung spielt hier beides – deine Wahl gehört dazu.", bad: false },
  good: { label: "Gut", short: "Gut", symbol: "✓", lead: "Fast so viel wert wie die beste Wahl.", bad: false },
  inaccurate: { label: "Ungenau", short: "Ungenau", symbol: "!", lead: "Knapp daneben, das kostet ein wenig.", bad: false },
  mistake: { label: "Fehler", short: "Fehler", symbol: "✕", lead: "Hier lässt du spürbar etwas liegen.", bad: true },
  blunder: { label: "Grober Fehler", short: "Grob", symbol: "✕✕", lead: "Das wird teuer.", bad: true },
};

export const gradeColor = (g: Grade) => `var(--grade-${g})`;

export type Source = "model" | "equilibrium" | "solver" | "computed" | "estimate" | "opinion" | "locked";

/** Where a number comes from, in one line a beginner understands. */
export const SOURCE: Record<Source, { label: string; text: string }> = {
  model: { label: "Modell", text: "Vereinfachtes Rechenmodell – gut als Richtung, nicht auf die Nachkommastelle." },
  equilibrium: { label: "Gleichgewicht", text: "Strategie, die gegen jeden Gegner nichts verliert. Gegen schwache Gegner geht mehr." },
  solver: { label: "Solver", text: "Vom Solver für genau diesen Spot berechnet." },
  computed: { label: "Berechnet", text: "Von Ringsides Rechenkern exakt nachgerechnet, nicht geschätzt." },
  estimate: { label: "Einschätzung", text: "Aus deinen Händen geschätzt – mit mehr Händen wird es genauer." },
  opinion: { label: "Einschätzung", text: "Meinung des Sprachmodells, etwa über die Ranges der Gegner – Ringside hat sie nicht nachgerechnet." },
  locked: { label: "Gesperrt, solange PokerStars läuft", text: "Hilfen während des Spiels sind bei PokerStars nicht erlaubt. Schließ den Client, dann geht es weiter." },
};
