import { jobsApi, onJobsChanged } from "./api";
import { isActive } from "./kinds";
import type { Job, JobsView } from "./types";

const TOAST_MS = 9000;

/** The task center's state; Main keeps `page` current so a finish elsewhere is announced. */
export const jobs = $state({
  list: [] as Job[],
  locked: false,
  open: false,
  /** Tasks that finished while another page was open. */
  toasts: [] as Job[],
  page: "",
});

const seen = new Map<number, Job["status"]>();
/** Tasks whose finish another note already announces. */
const silenced = new Set<number>();
let started = false;

function apply(v: JobsView) {
  for (const j of v.jobs) {
    const before = seen.get(j.id);
    seen.set(j.id, j.status);
    const finished = before !== undefined && (before === "running" || before === "paused") && !isActive(j);
    if (finished && (j.status === "done" || j.status === "failed") && j.target.page !== jobs.page && !silenced.has(j.id)) {
      jobs.toasts = [...jobs.toasts.filter((t) => t.id !== j.id), j].slice(-3);
      setTimeout(() => dismiss(j.id), TOAST_MS);
    }
  }
  jobs.list = v.jobs;
  jobs.locked = v.locked;
}

export function dismiss(id: number) {
  jobs.toasts = jobs.toasts.filter((t) => t.id !== id);
}

/** No "Fertig" note for this task: its own announces the result. */
export function silence(id: number) {
  silenced.add(id);
  dismiss(id);
}

/** Listens once for the whole app; the list comes from the backend, events only update it. */
export function startJobs() {
  if (started) return;
  started = true;
  onJobsChanged(apply).then(() => jobsApi.list().then(apply));
}

export const cancelJob = (id: number) => jobsApi.cancel(id).catch(() => {});
export const clearJobs = () => jobsApi.clear();
