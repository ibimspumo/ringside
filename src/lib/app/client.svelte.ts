import { api, on } from "../api";

/** Whether a poker client runs (PokerStars or GGPoker) and which; `running` is `null` until the backend answered. */
export const pokerClient = $state<{ running: boolean | null; name: string | null }>({ running: null, name: null });

let watching = false;

function update(running: boolean) {
  pokerClient.running = running;
  if (running) api.pokerClientName().then((n) => (pokerClient.name = n));
  else pokerClient.name = null;
}

/** Starts following the client once per app; later calls do nothing. */
export function watchPokerClient() {
  if (watching) return;
  watching = true;
  on<boolean>("poker-client", update).then(() => api.pokerClientRunning().then(update));
}

/**
 * Ratings, recommendations and model comparisons wait while the client runs, and until the
 * backend answered, so none flashes up at a table.
 */
export function adviceLocked(): boolean {
  return pokerClient.running !== false;
}

