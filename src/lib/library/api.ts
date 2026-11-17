import { call, on } from "../api";
import * as demo from "./demo";
import type { LibraryMode, LibraryOverview, OpenedSpot, WorkerStatus } from "./types";

export const libraryApi = {
  overview: () => call<LibraryOverview>("library_overview", {}, demo.overview),
  setMode: (mode: LibraryMode) => call<void>("library_set_mode", { mode }, () => demo.setMode(mode)),
  open: (scenario: string, flop: string) =>
    call<OpenedSpot>("library_open", { scenario, flop }, () => demo.open(scenario, flop)),
};

export const onLibraryStatus = (handler: (s: WorkerStatus) => void) => on<WorkerStatus>("library-status", handler);
