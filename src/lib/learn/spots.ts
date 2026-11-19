/** The learning spots (crates/ringside-analysis/src/spots.rs, src-tauri/src/spots.rs). */
import type { Decision } from "../analysis/types";
import type { Hand } from "../types";
import { demoRating } from "./demo";
import type { DrillResult, DrillSpot } from "./profile";

export interface KeyTopic {
  id: string;
  title: string;
  /** What the topic teaches, one sentence. */
  lesson: string;
}

export interface KeySeen {
  /** The last answer was right; null before the first. */
  right: boolean | null;
  /** Unix seconds of the last answer. */
  at: number | null;
  tries: number;
}

export interface KeySpotRow {
  id: string;
  topic: string;
  position: string;
  stack_bb: number;
  players: number;
  cards: string[];
  seen: KeySeen;
}

export interface KeySpots {
  topics: KeyTopic[];
  spots: KeySpotRow[];
  /** Ids to practise, most urgent first. */
  queue: string[];
}

/** How every hand class plays the spot: shares in percent per action, null where it cannot be. */
export interface SpotGrid {
  actions: { label: string; kind: string }[];
  cells: (number[] | null)[];
  /** Postflop: share of the class's combos that get here, percent. */
  weights?: number[];
}

export interface KeySpotView {
  spot: DrillSpot;
  topic: KeyTopic;
  grid: SpotGrid | null;
}

/* Browser demo: the pack's sample written by the `lern-spots` tool, answers kept in memory. */

interface PackSpot {
  id: string;
  topic: string;
  hand: Hand;
  action_index: number;
  decision: Decision;
  grid: SpotGrid | null;
  stack_bb: number;
  position: string;
  payouts?: number[];
}

interface Pack {
  topics: KeyTopic[];
  spots: PackSpot[];
}

let packCache: Promise<Pack> | null = null;
function demoPack(): Promise<Pack> {
  packCache ??= import("./demo-spots.json?raw").then((m) => JSON.parse(m.default) as Pack);
  return packCache;
}

const demoSeen = new Map<string, KeySeen>();
/** `?spots-seen=1` starts the demo with some answers, so the overview shows progress. */
function seedDemo(spots: PackSpot[]) {
  if (demoSeen.size || typeof window === "undefined" || new URLSearchParams(window.location.search).get("spots-seen") !== "1") return;
  const now = Math.floor(Date.now() / 1000);
  spots.forEach((s, i) => {
    if (i % 3 === 0) demoSeen.set(s.id, { right: i % 9 !== 0, at: now - 86_400 * (i % 5), tries: 1 });
  });
}

function demoQueue({ topics, spots }: Pack): string[] {
  const seen = (s: PackSpot) => demoSeen.get(s.id);
  const wrong = spots.filter((s) => seen(s)?.right === false).sort((a, b) => (seen(a)?.at ?? 0) - (seen(b)?.at ?? 0));
  const fresh: { k: number; t: number; s: PackSpot }[] = [];
  topics.forEach((t, ti) => spots.filter((s) => s.topic === t.id && !seen(s)).forEach((s, k) => fresh.push({ k, t: ti, s })));
  fresh.sort((a, b) => a.k - b.k || a.t - b.t);
  const right = spots.filter((s) => seen(s)?.right === true).sort((a, b) => (seen(a)?.at ?? 0) - (seen(b)?.at ?? 0));
  return [...wrong, ...fresh.map((f) => f.s), ...right].map((s) => s.id);
}

export async function demoKeySpots(): Promise<KeySpots> {
  const pack = await demoPack();
  const spots = pack.spots;
  seedDemo(spots);
  return {
    topics: pack.topics,
    spots: spots.map((s) => ({
      id: s.id,
      topic: s.topic,
      position: s.position,
      stack_bb: s.stack_bb,
      players: s.hand.seats.length,
      cards: s.hand.hero_cards,
      seen: demoSeen.get(s.id) ?? { right: null, at: null, tries: 0 },
    })),
    queue: demoQueue(pack),
  };
}

export async function demoKeySpot(id: string): Promise<KeySpotView> {
  const { topics, spots } = await demoPack();
  const s = spots.find((s) => s.id === id);
  if (!s) throw "Diesen Lern-Spot gibt es nicht.";
  const topic = topics.find((t) => t.id === s.topic) ?? topics[0];
  const stack = String(s.stack_bb).replace(".", ",");
  const paid = (s.payouts ?? []).filter((p) => p > 0).length;
  return {
    spot: {
      key: s.id,
      title: topic.title,
      detail: `${s.hand.seats.length} Spieler · ${stack} BB${paid ? ` · ${paid} kommen ins Geld` : ""}`,
      hand_id: s.hand.id,
      action_index: s.action_index,
      seed: null,
      played_at: s.hand.played_at,
      hand: s.hand,
      hero_cards: s.hand.hero_cards,
      position: s.position,
      choices: s.decision.strategy.map((c) => ({ label: c.label, kind: c.kind ?? "call" })),
      source: s.decision.source,
      reliability: null,
      pot_bb: s.decision.pot_bb,
    },
    topic,
    grid: s.grid,
  };
}

export async function demoKeySpotCheck(id: string, choice: number): Promise<DrillResult> {
  const s = (await demoPack()).spots.find((x) => x.id === id);
  if (!s) throw "Diesen Lern-Spot gibt es nicht.";
  const d = s.decision;
  const { rating, loss, points } = demoRating(d.strategy, choice, d.pot_bb ?? 1);
  const right = rating === "best" || rating === "mix" || rating === "good";
  const before = demoSeen.get(id);
  demoSeen.set(id, { right, at: Math.floor(Date.now() / 1000), tries: (before?.tries ?? 0) + 1 });
  return {
    decision: {
      ...d,
      strategy: d.strategy.map((c, k) => ({ ...c, played: k === choice })),
      played: d.strategy[choice].label,
      played_freq: d.strategy[choice].freq,
      rating,
      ev_loss_bb: loss,
    },
    score: points,
    correct: right,
    then: null,
  };
}
