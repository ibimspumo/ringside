import { call, inTauri, on } from "../api";
import * as demo from "./demo";
import type { HandView, LiveStatus, Lesson, OpenedSpot, PastHand, PlayConfig, PoolStatus } from "./types";

export const playApi = {
  prepare: (config: PlayConfig) => call<PoolStatus>("play_prepare", { config }, () => demo.prepare(config)),
  next: (config: PlayConfig, seed: number | null = null) => call<HandView>("play_next", { config, seed }, () => demo.next(config, seed)),
  repeat: () => call<HandView>("play_repeat", {}, demo.repeat),
  current: () => call<HandView | null>("play_current", {}, demo.current),
  act: (choice: number) => call<HandView>("play_act", { choice }, () => demo.act(choice)),
  hint: () => call<Lesson>("play_hint", {}, demo.hint),
  liveStatus: () => call<LiveStatus | null>("play_live_status", {}, demo.liveStatus),
  /** Stops the live solve; the hand plays on without a grade. */
  liveCancel: () => call<HandView | null>("play_live_cancel", {}, demo.liveCancel),
  poolStatus: () => call<PoolStatus>("play_pool_status", {}, demo.poolStatus),
  poolCancel: () => call<void>("play_pool_cancel", {}, demo.poolCancel),
  /** The page closed; the prepared spots are freed a while later. */
  away: () => call<void>("play_away", {}, () => {}),
  openSolver: () => call<OpenedSpot>("play_open_solver", {}, demo.openSolver),
  history: (limit: number) => call<PastHand[]>("play_history", { limit }, () => demo.history(limit)),
};

export const onPoolStatus = (handler: (s: PoolStatus) => void) => on<PoolStatus>("play-pool", handler);
/** Progress of a live solve. */
export const onLive = (handler: (s: LiveStatus) => void) => (inTauri ? on<LiveStatus>("play-live", handler) : demo.onLive(handler));
/** The hand went on after a live solve. */
export const onHand = (handler: (v: HandView) => void) => (inTauri ? on<HandView>("play-hand", handler) : demo.onHand(handler));
