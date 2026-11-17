// The practice session lives here, so a hand in progress and the session's numbers survive
// switching tabs. The stock of postflop spots keeps filling in the backend meanwhile.
import { playApi, onPoolStatus } from "./api";
import type { Grade, HandView, Lesson, PlayConfig, PoolStatus } from "./types";

const STORE = "ringside.play.config";

export const DEFAULT_CONFIG: PlayConfig = { mode: "full", players: 6, ante: "big_blind", stack_bb: 40, mixed_stacks: false, hero_seat: null };

function loadConfig(): PlayConfig {
  try {
    const saved = JSON.parse(localStorage.getItem(STORE) ?? "null");
    if (saved && typeof saved === "object") return { ...DEFAULT_CONFIG, ...saved };
  } catch {
    // Storage may be unavailable; the defaults are fine.
  }
  return { ...DEFAULT_CONFIG };
}

export interface SessionHand {
  /** Order in the session, from 1. */
  n: number;
  view: HandView;
  /** A replay; it is shown but does not count. */
  repeat: boolean;
}

export const session = $state({
  config: loadConfig(),
  /** The start screen or the table. */
  playing: false,
  hand: null as HandView | null,
  hint: null as Lesson | null,
  /** History entry shown instead of the live table (index into `hand.snapshots`). */
  look: null as number | null,
  hands: [] as SessionHand[],
  pool: null as PoolStatus | null,
  busy: false,
  error: null as string | null,
  /** A hand from the session list opened for review. */
  review: null as SessionHand | null,
});

export function saveConfig(c: PlayConfig) {
  session.config = c;
  try {
    localStorage.setItem(STORE, JSON.stringify(c));
  } catch {
    // ignore
  }
}

let listening = false;

export function listen() {
  if (listening) return;
  listening = true;
  onPoolStatus((s) => (session.pool = s));
  playApi.poolStatus().then((s) => (session.pool = s)).catch(() => {});
}

function errorText(e: unknown): string {
  const msg = typeof e === "string" ? e : e instanceof Error ? e.message : String(e);
  return msg === "gesperrt" ? "Gesperrt, solange PokerStars läuft." : msg;
}

async function guarded(f: () => Promise<void>) {
  if (session.busy) return;
  session.busy = true;
  session.error = null;
  try {
    await f();
  } catch (e) {
    session.error = errorText(e);
  } finally {
    session.busy = false;
  }
}

/** Keeps the finished hand in the session list (once). */
function remember(view: HandView, repeat: boolean) {
  if (!view.outcome) return;
  const last = session.hands[session.hands.length - 1];
  if (last && last.view.seed === view.seed && last.view.events.length === view.events.length && last.repeat === repeat) return;
  session.hands.push({ n: session.hands.length + 1, view, repeat });
}

let currentRepeat = false;

export function show(view: HandView, repeat = currentRepeat) {
  currentRepeat = repeat;
  session.hand = view;
  session.look = null;
  session.hint = null;
  session.review = null;
  remember(view, repeat);
}

export const nextHand = () =>
  guarded(async () => {
    session.playing = true;
    show(await playApi.next($state.snapshot(session.config)), false);
  });

export const repeatHand = () => guarded(async () => show(await playApi.repeat(), true));

export const act = (choice: number) => guarded(async () => show(await playApi.act(choice)));

export const askHint = () =>
  guarded(async () => {
    session.hint = await playApi.hint();
    if (session.hand?.decision) session.hand.decision.hinted = true;
  });

export const prepare = (c: PlayConfig) => playApi.prepare(c).then((s) => (session.pool = s)).catch(() => {});

/** Picks up a hand left on another page. */
export async function resume() {
  const view = await playApi.current().catch(() => null);
  if (view && !session.hand) {
    session.hand = view;
    session.playing = true;
  }
}

// ---------------------------------------------------------------------------------------------
// Session numbers.

export interface Stats {
  hands: number;
  moves: number;
  graded: number;
  /** Mean points 0–100 over graded moves. */
  score: number | null;
  counts: Record<Grade, number>;
  evLoss: number;
  evLossPerHand: number | null;
  net: number;
}

export function stats(hands: SessionHand[]): Stats {
  const counts: Record<Grade, number> = { best: 0, good: 0, inaccurate: 0, mistake: 0, blunder: 0 };
  let moves = 0;
  let graded = 0;
  let points = 0;
  let evLoss = 0;
  let net = 0;
  const counted = hands.filter((h) => !h.repeat);
  const finished = counted.filter((h) => h.view.outcome);
  for (const h of counted) {
    net += h.view.outcome?.net[h.view.hero] ?? 0;
    for (const v of h.view.verdicts) {
      moves += 1;
      if (v.points === null || !v.judgement) continue;
      graded += 1;
      points += v.points;
      counts[v.judgement.grade] += 1;
      evLoss += v.judgement.loss_bb ?? 0;
    }
  }
  return {
    hands: finished.length,
    moves,
    graded,
    score: graded ? points / graded : null,
    counts,
    evLoss,
    evLossPerHand: finished.length ? evLoss / Math.max(1, finished.length) : null,
    net,
  };
}

/** The worst grade of a hand, for the session list. */
export function worst(view: HandView): Grade | null {
  const order: Grade[] = ["best", "good", "inaccurate", "mistake", "blunder"];
  let w = -1;
  for (const v of view.verdicts) if (v.judgement) w = Math.max(w, order.indexOf(v.judgement.grade));
  return w < 0 ? null : order[w];
}
