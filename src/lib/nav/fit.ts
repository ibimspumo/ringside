import { tick } from "svelte";

/** Whether the last child of a row ends past the row's inner right edge. */
export function overflows(row: HTMLElement): boolean {
  const last = row.lastElementChild;
  if (!last) return false;
  const box = row.getBoundingClientRect();
  const end = box.right - parseFloat(getComputedStyle(row).paddingRight);
  return last.getBoundingClientRect().right > end + 0.5;
}

/**
 * Folds a row step by step until it fits: `set(0)` shows everything, each higher step drops or
 * shortens one more thing. Measured rather than tied to breakpoints, so long values and many notes
 * at once fold the same way. Calls that arrive while it runs fold once more at the end.
 */
export function folder(max: number, set: (step: number) => void) {
  let running = false;
  let again = false;
  return async function fold(row: HTMLElement | undefined) {
    if (!row) return;
    if (running) {
      again = true;
      return;
    }
    running = true;
    do {
      again = false;
      let step = 0;
      set(step);
      await tick();
      while (step < max && overflows(row)) {
        set(++step);
        await tick();
      }
    } while (again);
    running = false;
  };
}
