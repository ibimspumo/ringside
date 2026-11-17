import { call, on } from "../api";
import * as demo from "./demo";
import type { HandView, Lesson, OpenedSpot, PastHand, PlayConfig, PoolStatus } from "./types";

export const playApi = {
  prepare: (config: PlayConfig) => call<PoolStatus>("play_prepare", { config }, () => demo.prepare(config)),
  next: (config: PlayConfig, seed: number | null = null) => call<HandView>("play_next", { config, seed }, () => demo.next(config, seed)),
  repeat: () => call<HandView>("play_repeat", {}, demo.repeat),
  current: () => call<HandView | null>("play_current", {}, demo.current),
  act: (choice: number) => call<HandView>("play_act", { choice }, () => demo.act(choice)),
  hint: () => call<Lesson>("play_hint", {}, demo.hint),
  poolStatus: () => call<PoolStatus>("play_pool_status", {}, demo.poolStatus),
  poolCancel: () => call<void>("play_pool_cancel", {}, demo.poolCancel),
  openSolver: () => call<OpenedSpot>("play_open_solver", {}, demo.openSolver),
  history: (limit: number) => call<PastHand[]>("play_history", { limit }, () => demo.history(limit)),
};

export const onPoolStatus = (handler: (s: PoolStatus) => void) => on<PoolStatus>("play-pool", handler);
