/** Whether the last interaction came from the keyboard, so programmatic focus can follow :focus-visible rules. */
export const input = { keys: false };

if (typeof window !== "undefined") {
  window.addEventListener("keydown", () => (input.keys = true), true);
  window.addEventListener("pointerdown", () => (input.keys = false), true);
}
