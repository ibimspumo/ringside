import { call, inTauri, on } from "../api";
import * as demo from "./demo";
import type { JobsView } from "./types";

export const jobsApi = {
  list: () => call<JobsView>("jobs_list", {}, demo.demoList),
  cancel: (id: number) => call<void>("jobs_cancel", { id }, () => demo.demoCancel(id)),
  /** Forgets finished tasks. */
  clear: () => call<void>("jobs_clear", {}, demo.demoClear),
};

export function onJobsChanged(handler: (v: JobsView) => void): Promise<() => void> {
  return inTauri ? on<JobsView>("jobs-changed", handler) : Promise.resolve(demo.onChanged(handler));
}
