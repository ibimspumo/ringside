import { api, on } from "../api";

/** Whether the PokerStars client runs; `null` until the backend answered. */
export const pokerClient = $state<{ running: boolean | null }>({ running: null });

let watching = false;

/** Starts following the client once per app; later calls do nothing. */
export function watchPokerClient() {
  if (watching) return;
  watching = true;
  on<boolean>("poker-client", (r) => (pokerClient.running = r)).then(() => api.pokerClientRunning().then((r) => (pokerClient.running = r)));
}
