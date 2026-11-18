import { call, on } from "../api";
import * as demo from "./demo";
import type { LibraryMode, LibraryOverview, LibraryPower, OpenedSpot, WorkerStatus } from "./types";

export const libraryApi = {
  overview: () => call<LibraryOverview>("library_overview", {}, demo.overview),
  setMode: (mode: LibraryMode) => call<void>("library_set_mode", { mode }, () => demo.setMode(mode)),
  setPower: (power: LibraryPower) => call<void>("library_set_power", { power }, () => demo.setPower(power)),
  /** Starts solving now (also while the app is in use) or stops it. */
  run: (run: boolean) => call<void>("library_run", { run }, () => demo.run(run)),
  open: (scenario: string, flop: string) =>
    call<OpenedSpot>("library_open", { scenario, flop }, () => demo.open(scenario, flop)),
};

export const onLibraryStatus = (handler: (s: WorkerStatus) => void) => on<WorkerStatus>("library-status", handler);
