/** Browser-only stand-ins for the coach commands (design work, screenshots). Made-up hands. */
import type {
  CoachProgress,
  CoachReport,
  CoachSpot,
  CoachStatus,
  ConceptCount,
  Effort,
  Estimate,
  KeyInfo,
  ModelInfo,
  ReportListItem,
} from "./types";
import { demoTournaments } from "../demo";
import { CONCEPT_NAMES } from "./concepts";

const param = (name: string) =>
  typeof window === "undefined" ? null : new URLSearchParams(window.location.search).get(name);

const ROWS = new Map(demoTournaments().rows.map((t) => [t.id, t]));
/** The first two finished Hold'em tournaments of the demo data. */
const [T1, T2] = [...ROWS.values()].filter((t) => t.prize !== null && t.game.includes("Hold")).map((t) => t.id);
const handsOf = (tournament: number) => ROWS.get(tournament)?.hands ?? 126;
const hand = (tournament: number, n: number) => tournament * 1000 + 999 - n;

let hasKey = param("coach") !== "nokey";

/**
 * Preview states that need clicks in the app: `?run=preparing|confirm|running|stalled|error` opens the
 * analysis of the newest report in that state, `?coach=setup` opens the coach's settings.
 */
export const preview = {
  run: param("run") as "preparing" | "confirm" | "running" | "stalled" | "error" | null,
  setup: param("coach") === "setup",
};
let model = "openai/gpt-6.1-sol";
let effort: Effort = "high";

function spot(over: Partial<CoachSpot> & Pick<CoachSpot, "hand_id" | "verdict" | "concept" | "action" | "why">): CoachSpot {
  return {
    played_at: "2026/11/16 21:14:08",
    hero_cards: ["Ah", "Kc"],
    board: [],
    position: "BTN",
    stack_bb: 24,
    better: null,
    numbers: [],
    ...over,
  };
}

function report(tournament: number, created: number): CoachReport {
  return {
    tournament_id: tournament,
    summary:
      "Vor dem Flop spielst du mit kurzem Stack meist sauber. Zwei Folds im Small Blind kosten dich aber Chips, und am River callst du zu oft gegen große Bets, ohne genug Equity zu haben.",
    focus: [
      "Push/Fold im Small Blind bis 12 bb",
      "River-Calls gegen große Bets mit Pot Odds prüfen",
      "Mit Top Pair auf nassen Boards kleiner setzen",
    ],
    spots: [
      spot({
        hand_id: hand(tournament, 3),
        hero_cards: ["Kh", "6d"],
        position: "SB",
        stack_bb: 8.2,
        action: "Alle folden zu dir, du foldest K6o im Small Blind.",
        verdict: "fehler",
        better: "All-in schieben",
        why: "Mit 8 bb und Antes im Pot schiebt das Gleichgewicht K6o immer. Mit dem Fold gibst du die Blinds und Antes kampflos ab.",
        numbers: [
          { label: "Nash-Shove mit K6o", value: "100 %", source: "berechnet" },
          { label: "EV All-in", value: "+0,61 bb", source: "berechnet" },
          { label: "EV Fold", value: "−0,62 bb", source: "berechnet" },
        ],
        concept: "push_fold_sb",
      }),
      spot({
        hand_id: hand(tournament, 5),
        hero_cards: ["Qs", "Jh"],
        board: ["Ks", "8d", "4c", "2h", "9s"],
        position: "BB",
        stack_bb: 31.5,
        action: "Du callst am River 9 bb in einen Pot von 12 bb.",
        verdict: "fraglich",
        better: "Fold",
        why: "Du brauchst gut 31 % Equity. Gegen eine Range, die am River so groß setzt, schlägst du mit Dame hoch fast nur verpasste Draws, und davon gibt es auf diesem Board wenige.",
        numbers: [
          { label: "Benötigte Equity", value: "31,4 %", source: "berechnet" },
          { label: "Equity gegen K8+, 99, Bluffs mit T9s", value: "18,2 %", source: "berechnet" },
          { label: "Range des Gegners", value: "Top Pair und besser, wenige Bluffs", source: "einschätzung" },
        ],
        concept: "pot_odds",
      }),
      spot({
        hand_id: hand(tournament, 9),
        hero_cards: ["9s", "9d"],
        position: "HJ",
        stack_bb: 14,
        action: "Du schiebst 99 nach einem Limp all-in.",
        verdict: "stark",
        why: "Mit 14 bb nimmst du das tote Geld mit und bist gegen fast jede Call-Range gut aufgestellt.",
        numbers: [
          { label: "Equity gegen 66+, ATs+, AJo+", value: "51,7 %", source: "berechnet" },
          { label: "Call-Range des Limpers", value: "66+, ATs+, AJo+", source: "einschätzung" },
        ],
        concept: "short_stack",
      }),
      spot({
        hand_id: hand(tournament, 14),
        hero_cards: ["Jd", "Td"],
        board: ["9d", "8c", "2d"],
        position: "CO",
        stack_bb: 22,
        action: "Du callst am Flop eine Bet von 2/3 Pot mit Straight- und Flush-Draw.",
        verdict: "ok",
        better: "Check-Raise all-in ist ebenso gut",
        why: "Mit 15 Outs bist du gegen ein Overpair fast Favorit. Callen ist fein, ein Check-Raise all-in nutzt zusätzlich deine Fold Equity.",
        numbers: [
          { label: "Outs gegen QQ", value: "15", source: "berechnet" },
          { label: "Equity gegen QQ", value: "54,3 %", source: "berechnet" },
        ],
        concept: "outs",
      }),
      spot({
        hand_id: hand(tournament, 21),
        hero_cards: ["As", "Qd"],
        board: ["Qc", "7h", "3s", "Td", "Ad"],
        position: "BTN",
        stack_bb: 40,
        action: "Du setzt am River 75 % Pot mit zwei Paaren.",
        verdict: "stark",
        why: "Schlechtere Damen und Asse zahlen, und der Gegner hat nach Check-Call kaum Straßen.",
        numbers: [{ label: "Call-Range des Gegners", value: "Qx, Ax, einige Tx", source: "einschätzung" }],
        concept: "value_bet",
      }),
    ],
    meta: {
      model,
      created_at: created,
      prompt_tokens: 21560,
      completion_tokens: 6765,
      cost_usd: 0.0978,
      seconds: 183.5,
      steps: 2,
      tool_calls: 4,
      hands_total: handsOf(tournament),
      hands_sent: 29,
      downgraded: 0,
    },
  };
}

const reports = new Map<number, CoachReport>(
  param("coach") === "empty" ? [] : [
    [T1, report(T1, 1794930251)],
    [T2, { ...report(T2, 1794843851), spots: report(T2, 0).spots.slice(1, 4) }],
  ],
);

export const status = (): CoachStatus => ({ has_key: hasKey, model, effort, running: false });

export function setKey(): KeyInfo {
  hasKey = true;
  return keyInfo();
}

export const keyInfo = (): KeyInfo => ({ label: "sk-or-v1-a1b…f09", usage: 0.31, limit: 5, free_tier: false });

export function deleteKey() {
  hasKey = false;
}

export function setModel(id: string) {
  model = id;
}

export function setEffort(e: Effort) {
  effort = e;
}

export const models = (): ModelInfo[] => [
  { id: "anthropic/claude-sonnet-5.5", name: "Anthropic: Claude Sonnet 5.5", prompt_per_m: 3, completion_per_m: 15, context_length: 1000000, tools: true, reasoning: true },
  { id: "deepseek/deepseek-v4", name: "DeepSeek: V4", prompt_per_m: 0.3, completion_per_m: 1.2, context_length: 163840, tools: true, reasoning: true },
  { id: "google/gemini-3.5-flash", name: "Google: Gemini 3.5 Flash", prompt_per_m: 0.35, completion_per_m: 2.8, context_length: 1048576, tools: true, reasoning: true },
  { id: "meta-llama/llama-5-70b", name: "Meta: Llama 5 70B", prompt_per_m: 0.12, completion_per_m: 0.4, context_length: 131072, tools: false, reasoning: false },
  { id: "openai/gpt-6.1-sol", name: "OpenAI: GPT-6.1 Sol", prompt_per_m: 2, completion_per_m: 10, context_length: 400000, tools: true, reasoning: true },
  { id: "openai/gpt-6.1-terra", name: "OpenAI: GPT-6.1 Terra", prompt_per_m: 0.4, completion_per_m: 1.6, context_length: 400000, tools: true, reasoning: true },
  { id: "tencent/hy4-flash", name: "Tencent: Hy4 flash", prompt_per_m: 0.035, completion_per_m: 0.14, context_length: 1048576, tools: true, reasoning: true },
];

/** Like `coach_prepare`: fails without a key, and follows the `?run=` preview. */
export async function prepare(tournament: number): Promise<Estimate> {
  if (preview.run === "preparing") return new Promise(() => {});
  if (preview.run === "error")
    throw "OpenRouter ist gerade nicht erreichbar. Prüf deine Internetverbindung und versuch es noch einmal.";
  if (!hasKey) throw "Hinterleg zuerst in den Einstellungen einen OpenRouter-Key.";
  await new Promise((r) => setTimeout(r, 300));
  return estimate(tournament);
}

function estimate(tournament: number): Estimate {
  const m = models().find((x) => x.id === model);
  const price = (p: number, c: number) => (m?.prompt_per_m == null || m.completion_per_m == null ? null : (p * m.prompt_per_m + c * m.completion_per_m) / 1e6);
  const reasons = m?.reasoning ?? true;
  const share = reasons ? { high: 1, medium: 0.6, low: 0.35 }[effort] : 1;
  const completion = Math.round(7900 * share);
  const maxCompletion = Math.round(24100 * share);
  return {
    token: "demo",
    tournament_id: tournament,
    model,
    effort: reasons ? effort : null,
    hands_total: handsOf(tournament),
    hands_sent: 29,
    hands_without_decision: handsOf(tournament) - 29,
    hands_dropped: 0,
    spots: 3,
    input_tokens: 9690,
    prompt_tokens: 31770,
    completion_tokens: completion,
    max_prompt_tokens: 175680,
    max_completion_tokens: maxCompletion,
    prompt_per_m: m?.prompt_per_m ?? null,
    completion_per_m: m?.completion_per_m ?? null,
    cost_usd: price(31770, completion),
    max_cost_usd: price(175680, maxCompletion),
  };
}

type Listener = (p: CoachProgress) => void;
const listeners = new Set<Listener>();
let cancelled = false;

export function onProgress(f: Listener): () => void {
  listeners.add(f);
  return () => listeners.delete(f);
}

export function cancel() {
  cancelled = true;
}

type Phase = Pick<CoachProgress, "step" | "stage" | "message" | "tool"> & {
  /** Seconds the phase lasts; the long one is stretched by `?coach=slow`. */
  seconds: number;
  /** Visible reasoning and output tokens per second while it lasts. */
  reasoning?: number;
  output?: number;
  long?: boolean;
};

const PHASES: Phase[] = [
  { step: 1, stage: "thinking", message: "Das Modell liest die Hände und denkt nach …", tool: null, seconds: 2.5, reasoning: 40 },
  { step: 1, stage: "calling", message: "Das Modell ruft das Werkzeug „Push/Fold-Nash“ auf …", tool: "push_fold_nash", seconds: 1, output: 30 },
  { step: 1, stage: "tool", message: "Ringside rechnet: Push/Fold-Nash", tool: "push_fold_nash", seconds: 0.8 },
  { step: 1, stage: "tool", message: "Ringside rechnet: Equity", tool: "equity", seconds: 0.6 },
  { step: 1, stage: "tool", message: "Ringside rechnet: Pot Odds", tool: "pot_odds", seconds: 0.6 },
  { step: 2, stage: "thinking", message: "Das Modell denkt nach …", tool: null, seconds: 2.5, reasoning: 35, long: true },
  { step: 2, stage: "calling", message: "Das Modell schreibt den Bericht …", tool: "report", seconds: 2, output: 120 },
];

/** Plays a fake run with live progress; `?coach=slow` stays on the second step for minutes. */
export async function run(tournament: number): Promise<CoachReport> {
  cancelled = false;
  const stalled = preview.run === "stalled";
  const slow = param("coach") === "slow" || preview.run !== null;
  // A stalled step has been waiting a while already.
  const started = Date.now() - (stalled ? 95_000 : 0);
  let tools = 0;
  let billed = { prompt: 0, completion: 0, reasoning: 0, cost: 0 };
  let step = 0;
  let stepStarted = started;
  let live = { reasoning: 0, output: 0 };
  for (const ph of PHASES) {
    if (ph.step !== step) {
      if (step) billed = { prompt: 10840, completion: 4721, reasoning: 3980, cost: 0.0688 };
      step = ph.step;
      stepStarted = stalled && ph.long ? Date.now() - 52_000 : Date.now();
      live = { reasoning: 0, output: 0 };
    }
    if (ph.stage === "tool") tools++;
    const seconds = slow && ph.long ? 600 : ph.seconds;
    const until = Date.now() + seconds * 1000;
    do {
      if (cancelled) throw "Abgebrochen.";
      const p: CoachProgress = {
        tournament_id: tournament,
        step: ph.step,
        max_steps: 12,
        stage: ph.stage,
        message: ph.message,
        tool: ph.tool,
        tool_calls: tools,
        prompt_tokens: billed.prompt,
        completion_tokens: billed.completion,
        reasoning_tokens: billed.reasoning,
        cost_usd: billed.cost,
        live_reasoning_tokens: Math.round(live.reasoning),
        live_output_tokens: Math.round(live.output),
        step_seconds: (Date.now() - stepStarted) / 1000,
        total_seconds: (Date.now() - started) / 1000,
        idle_seconds: stalled && ph.long ? (Date.now() - stepStarted) / 1000 - 14 : 0.3,
        idle_limit: 120,
      };
      listeners.forEach((f) => f(p));
      await new Promise((r) => setTimeout(r, 400));
      live.reasoning += ((ph.reasoning ?? 0) * 400) / 1000;
      live.output += ((ph.output ?? 0) * 400) / 1000;
    } while (Date.now() < until);
  }
  if (cancelled) throw "Abgebrochen.";
  const r = report(tournament, Math.floor(Date.now() / 1000));
  reports.set(tournament, r);
  return r;
}

export function list(names: Map<number, { name: string | null; game: string; started_at: string }>): ReportListItem[] {
  return [...reports.values()]
    .sort((a, b) => b.meta.created_at - a.meta.created_at)
    .map((r) => ({
      tournament_id: r.tournament_id,
      created_at: r.meta.created_at,
      model: r.meta.model,
      spots: r.spots.length,
      mistakes: r.spots.filter((s) => s.verdict === "fehler" || s.verdict === "fraglich").length,
      cost_usd: r.meta.cost_usd,
      name: (names.get(r.tournament_id) ?? ROWS.get(r.tournament_id))?.name ?? null,
      game: (names.get(r.tournament_id) ?? ROWS.get(r.tournament_id))?.game ?? "NL Hold'em",
      started_at: (names.get(r.tournament_id) ?? ROWS.get(r.tournament_id))?.started_at ?? "2026/11/16 20:00:00",
    }));
}

export const get = (tournament: number): CoachReport | null => reports.get(tournament) ?? null;

export function remove(tournament: number): boolean {
  return reports.delete(tournament);
}

/** Exercise and articles of the concepts the demo reports use, as in crates/ringside-coach/src/concepts.rs. */
const LINKS: Record<string, { exercise: string | null; articles: string[] }> = {
  push_fold_sb: { exercise: "push_fold_sb", articles: ["push-fold", "effektiver-stack"] },
  short_stack: { exercise: "push_fold_open", articles: ["push-fold", "m-ratio", "fold-equity"] },
  pot_odds: { exercise: "pot_odds", articles: ["pot-odds", "mdf"] },
  outs: { exercise: "outs", articles: ["outs", "regel-2-4", "implizite-odds"] },
  value_bet: { exercise: null, articles: ["value-bluff", "alpha"] },
};

export function concepts(): ConceptCount[] {
  const counts = new Map<string, ConceptCount>();
  for (const r of reports.values()) {
    for (const s of r.spots) {
      const c = counts.get(s.concept) ?? {
        id: s.concept,
        name: CONCEPT_NAMES[s.concept] ?? s.concept,
        exercise: LINKS[s.concept]?.exercise ?? null,
        articles: LINKS[s.concept]?.articles ?? [],
        spots: 0,
        mistakes: 0,
        questionable: 0,
        tournaments: 0,
      };
      c.spots++;
      if (s.verdict === "fehler") c.mistakes++;
      if (s.verdict === "fraglich") c.questionable++;
      counts.set(s.concept, c);
    }
    for (const id of new Set(r.spots.map((s) => s.concept))) counts.get(id)!.tournaments++;
  }
  return [...counts.values()].sort((a, b) => b.mistakes - a.mistakes || b.questionable - a.questionable || b.spots - a.spots);
}
