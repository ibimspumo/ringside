// Browser preview only: a few made-up tasks that move, so the task center can be designed
// and screenshotted without the Rust backend. `?jobs=1` turns them on.
import type { Job, JobsView } from "./types";

const listeners = new Set<(v: JobsView) => void>();
const on = typeof window !== "undefined" && new URLSearchParams(window.location.search).has("jobs");
const t0 = Date.now();
let jobs: Job[] = [];
let timer: ReturnType<typeof setInterval> | null = null;

function job(id: number, part: Partial<Job>): Job {
  return {
    id,
    kind: "solver",
    title: "",
    detail: null,
    status: "running",
    progress: null,
    steps: null,
    eta_secs: null,
    elapsed_ms: 0,
    started_at: t0,
    finished_at: null,
    cancellable: true,
    cancelling: false,
    by_client: false,
    error: null,
    target: { page: "solver", id: null },
    ...part,
  };
}

function seed() {
  jobs = [
    job(4, {
      kind: "solver",
      title: "Lösen: BTN gegen BB, Single-Raised-Pot",
      detail: "Iteration 140 · noch 0,92 % ausbeutbar, Ziel 0,50 %",
      progress: 0.58,
      elapsed_ms: 41_000,
      eta_secs: 30,
    }),
    job(3, {
      kind: "preflop",
      title: "Preflop-Chart rechnen: 8 Spieler, 45 BB",
      detail: "Iteration 3200 von höchstens 20000",
      progress: 0.31,
      elapsed_ms: 52_000,
      eta_secs: 115,
      target: { page: "charts", id: null },
    }),
    job(2, {
      kind: "library",
      title: "Bibliothek: CO gegen BB auf Kd7s2c",
      detail: "Rechnet, solange du die App nicht benutzt",
      progress: 0.12,
      elapsed_ms: 18_000,
      target: { page: "library", id: null },
    }),
    job(1, {
      kind: "coach",
      title: "Coach bespricht ein Turnier",
      detail: "Bericht fertig",
      status: "done",
      progress: 1,
      elapsed_ms: 96_000,
      finished_at: t0 - 4 * 60_000,
      target: { page: "coach", id: 3_811_220_034 },
    }),
  ];
}

function emit() {
  const view = demoList();
  listeners.forEach((f) => f(view));
}

function tick() {
  for (const j of jobs) {
    if (j.status !== "running") continue;
    j.elapsed_ms += 1000;
    if (j.cancelling) {
      Object.assign(j, { status: "cancelled", cancelling: false, finished_at: Date.now() });
      continue;
    }
    const step = j.kind === "solver" ? 0.07 : j.kind === "preflop" ? 0.008 : 0.004;
    j.progress = Math.min(1, (j.progress ?? 0) + step);
    j.eta_secs = j.progress >= 1 ? null : (j.elapsed_ms / 1000) * ((1 - j.progress) / j.progress);
    if (j.progress >= 1) {
      Object.assign(j, { status: "done", finished_at: Date.now(), detail: "Fertig · 0,48 % ausbeutbar", target: { page: "solver", id: 1 } });
    }
  }
  emit();
}

export function demoList(): JobsView {
  if (on && !jobs.length) seed();
  if (on && !timer) timer = setInterval(tick, 1000);
  const locked = new URLSearchParams(window.location.search).get("locked") === "1";
  const view = structuredClone(jobs).map((j) => (locked && j.status === "running" ? { ...j, status: "paused" as const } : j));
  return { jobs: view, locked };
}

export function demoCancel(id: number) {
  const j = jobs.find((x) => x.id === id);
  if (j) j.cancelling = true;
  emit();
}

export function demoClear() {
  jobs = jobs.filter((j) => j.status === "running" || j.status === "paused");
  emit();
}

export function onChanged(f: (v: JobsView) => void): () => void {
  listeners.add(f);
  return () => void listeners.delete(f);
}
