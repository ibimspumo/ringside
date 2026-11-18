/** Enter and leave transitions of the coach: short, on cubic curves, off when the system asks for less motion. */
import { cubicIn, cubicOut } from "svelte/easing";
import { fade, fly, scale } from "svelte/transition";

const still = () => typeof window !== "undefined" && !!window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;

/** Slides up a few pixels while fading in; `delay` staggers lists. */
export const rise = (node: Element, { delay = 0 }: { delay?: number } = {}) =>
  fly(node, { y: 8, duration: still() ? 0 : 240, delay: still() ? 0 : delay, easing: cubicOut });

export const appear = (node: Element) => fade(node, { duration: still() ? 0 : 200, easing: cubicOut });

export const leave = (node: Element) => fade(node, { duration: still() ? 0 : 120, easing: cubicIn });

export const pop = (node: Element) => scale(node, { start: 0.85, duration: still() ? 0 : 200, easing: cubicOut });
