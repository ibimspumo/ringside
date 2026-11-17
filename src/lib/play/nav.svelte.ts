/** Asks the main view to switch tabs for the practice: into it (e.g. from the learning area), or to the solver. */
export const playNav = $state({ requested: 0, solver: 0 });

export function openPlay() {
  playNav.requested += 1;
}

export function openSolverTab() {
  playNav.solver += 1;
}
