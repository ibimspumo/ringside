import { api } from "../api";
import { solver } from "./store.svelte";
import type { SolverNode, SolverStep } from "./types";

let run = -1;
const cache = new Map<string, Promise<SolverNode>>();

/** A node of the current solution; asked once per path and solve. */
export function loadNode(path: SolverStep[]): Promise<SolverNode> {
  if (run !== solver.run) {
    cache.clear();
    run = solver.run;
  }
  const key = JSON.stringify(path);
  let hit = cache.get(key);
  if (!hit) {
    hit = api.solverNode(path);
    hit.catch(() => cache.delete(key));
    cache.set(key, hit);
  }
  return hit;
}
