// Playing a hand step by step at a table: a cursor over the hand's steps that walks to a
// target at the chosen speed. The speed is the viewer's and kept between sessions.

export type Speed = "slow" | "normal" | "fast" | "instant";

export const SPEEDS: { id: Speed; label: string; ms: number }[] = [
  { id: "slow", label: "Langsam", ms: 850 },
  { id: "normal", label: "Normal", ms: 450 },
  { id: "fast", label: "Schnell", ms: 220 },
  { id: "instant", label: "Sofort", ms: 0 },
];

const STORE = "ringside.table.speed";

function initial(): Speed {
  try {
    const saved = localStorage.getItem(STORE);
    if (saved && SPEEDS.some((s) => s.id === saved)) return saved as Speed;
  } catch {
    // Storage may be unavailable; fall through to the default.
  }
  const reduced = typeof matchMedia !== "undefined" && matchMedia("(prefers-reduced-motion: reduce)").matches;
  return reduced ? "instant" : "normal";
}

/** Shared by every table, so a change on one holds on the others. */
export const speed = $state({ id: initial() });

export function setSpeed(id: Speed) {
  speed.id = id;
  try {
    localStorage.setItem(STORE, id);
  } catch {
    // ignore
  }
}

/** One step faster (`+1`) or slower (`−1`). */
export function nudgeSpeed(by: 1 | -1) {
  const i = SPEEDS.findIndex((s) => s.id === speed.id);
  setSpeed(SPEEDS[Math.max(0, Math.min(SPEEDS.length - 1, i + by))].id);
}

export const stepMs = () => SPEEDS.find((s) => s.id === speed.id)?.ms ?? 450;

/**
 * A cursor that walks to `target` one step at a time. `weight(step)` stretches the pause
 * before a step (e.g. a new street takes longer than a fold).
 */
export class Playback {
  cursor = $state(0);
  target = $state(0);
  #timer: ReturnType<typeof setTimeout> | null = null;
  #weight: (step: number) => number;

  constructor(weight: (step: number) => number = () => 1) {
    this.#weight = weight;
  }

  get running() {
    return this.cursor < this.target;
  }

  /** Jumps to `step` without playing. */
  jump(step: number) {
    this.#stop();
    this.cursor = step;
    this.target = step;
  }

  /** Plays from the cursor to `step`. */
  play(step: number) {
    this.target = step;
    if (this.cursor > step) this.cursor = step;
    this.#tick();
  }

  /** Shows the end of the running animation at once. */
  skip() {
    this.#stop();
    this.cursor = this.target;
  }

  dispose() {
    this.#stop();
  }

  #stop() {
    if (this.#timer !== null) clearTimeout(this.#timer);
    this.#timer = null;
  }

  #tick() {
    this.#stop();
    if (this.cursor >= this.target) return;
    const ms = stepMs();
    if (ms === 0) {
      this.cursor = this.target;
      return;
    }
    this.#timer = setTimeout(() => {
      this.#timer = null;
      this.cursor += 1;
      this.#tick();
    }, ms * this.#weight(this.cursor + 1));
  }
}
