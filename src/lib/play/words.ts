// Sentences about a solution's strategy and a graded move, shared by the practice, the solver
// page and the hand analysis so all say the same in the same words. No imports: the
// grammar test runs it in plain Node (`node --test e2e/words.test.mjs`).

export type MoveKind = "fold" | "check" | "call" | "limp" | "bet" | "raise" | "all_in";

export const pct = (f: number) => `${Math.round(f * 100).toLocaleString("de-DE")} %`;
export const bbNum = (x: number, d = 2) => x.toLocaleString("de-DE", { maximumFractionDigits: d });
export const signed = (x: number, d = 2) => `${x > 0.005 ? "+" : x < -0.005 ? "−" : "±"}${bbNum(Math.abs(x), d)}`;

/** How often, in words. */
export function often(f: number): string {
  if (f >= 0.995) return "immer";
  if (f >= 0.85) return "fast immer";
  if (f >= 0.6) return "meistens";
  if (f >= 0.35) return "oft";
  if (f >= 0.12) return "manchmal";
  return "selten";
}

/** Third person present and the separable part that goes to the end, e.g. "geht … all-in". */
export function conjugate(kind: MoveKind): [string, string] {
  switch (kind) {
    case "fold":
      return ["foldet", ""];
    case "check":
      return ["checkt", ""];
    case "call":
      return ["callt", ""];
    case "limp":
      return ["limpt", ""];
    case "bet":
      return ["setzt", ""];
    case "raise":
      return ["erhöht", ""];
    case "all_in":
      return ["geht", "all-in"];
  }
}

export function infinitive(kind: MoveKind): string {
  switch (kind) {
    case "fold":
      return "folden";
    case "check":
      return "checken";
    case "call":
      return "callen";
    case "limp":
      return "limpen";
    case "bet":
      return "setzen";
    case "raise":
      return "erhöhen";
    case "all_in":
      return "all-in gehen";
  }
}

export function moveName(kind: MoveKind): string {
  return { fold: "Fold", check: "Check", call: "Call", limp: "Limp", bet: "Bet", raise: "Raise", all_in: "All-in" }[kind];
}

/** Who plays the strategy, with the pronoun that refers back to it. */
export type Who = "model" | "equilibrium" | "solution";
const SUBJECT: Record<Who, [string, string]> = {
  model: ["Das Modell", "es"],
  equilibrium: ["Das Gleichgewicht", "es"],
  solution: ["Die Lösung", "sie"],
};

export interface Part {
  kind: MoveKind;
  f: number;
}

/** The parts worth naming (at least 5 %), most frequent first, at most three. */
export function mainParts(parts: Part[]): Part[] {
  return parts
    .filter((p) => p.f >= 0.05)
    .sort((a, b) => b.f - a.f)
    .slice(0, 3);
}

function clause(p: Part, subject?: string): string {
  const [verb, rest] = conjugate(p.kind);
  const tail = `${often(p.f)}${rest ? ` ${rest}` : ""} (${pct(p.f)})`;
  return subject ? `${verb} ${subject} ${tail}` : `${verb} ${tail}`;
}

/** E.g. "Das Modell erhöht mit KJo hier meistens (86 %), sonst callt es manchmal (14 %)." */
export function playsLine(who: Who, hand: string, parts: Part[]): string {
  const [subject, pronoun] = SUBJECT[who];
  const ps = mainParts(parts);
  if (!ps.length) return `${subject} spielt ${hand} hier gar nicht.`;
  const [verb, rest] = conjugate(ps[0].kind);
  let s = `${subject} ${verb} mit ${hand} hier ${often(ps[0].f)}${rest ? ` ${rest}` : ""} (${pct(ps[0].f)})`;
  if (ps[1]) s += `, sonst ${clause(ps[1], pronoun)}`;
  if (ps[2]) s += ` oder ${clause(ps[2])}`;
  return `${s}.`;
}

/** Two EVs that read the same to the hundredth of a BB count as equal. */
export const sameEv = (a: number, b: number) => Math.abs(Math.round(a * 100) - Math.round(b * 100)) <= 1;

/**
 * A rarer move of a mixed strategy worth as much as the main one, answer first:
 * "Auch richtig – gleich viel wert wie Raise. …"
 */
export function mixText(who: Who, hand: string, main: Part & { ev: number | null }, chosen: Part & { ev: number | null }): string {
  const [subject, pronoun] = SUBJECT[who];
  const ev = main.ev !== null && chosen.ev !== null ? ` ${moveName(main.kind)} und ${moveName(chosen.kind)} bringen hier beide etwa ${signed((main.ev + chosen.ev) / 2)} BB.` : "";
  const [v1, r1] = conjugate(main.kind);
  const [v2, r2] = conjugate(chosen.kind);
  const mix = `${subject} ${v1} ${often(main.f)}${r1 ? ` ${r1}` : ""} (${pct(main.f)}) und ${v2} ${often(chosen.f)}${r2 ? ` ${r2}` : ""} (${pct(chosen.f)}), damit ${pronoun} schwer zu lesen ist.`;
  return `Auch richtig – gleich viel wert wie ${moveName(main.kind)}.${ev} ${mix} Einfacher zu merken: mit ${hand} hier ${often(main.f)} ${infinitive(main.kind)}.`;
}

/** Headline of a costly move: "Besser: Check – dein Bet kostet 1,15 BB (12 % des Pots)." */
export function betterLine(best: MoveKind, chosen: MoveKind, lossBb: number, lossPct: number): string {
  return `Besser: ${moveName(best)} – dein ${moveName(chosen)} kostet ${bbNum(lossBb)} BB (${bbNum(lossPct, 0)} % des Pots).`;
}
