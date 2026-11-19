import { inTauri } from "../api";
import type { AnswerSpec, LearnAnswer } from "../types";

/**
 * Browser-demo switches for screenshots of states that are otherwise only reachable by clicking:
 * `learn=today|spots|path|own|trainer|wissen`, `skill=<kind>`, `level=1|2|3`, `article=<id>`, `session=run|done`, `answer=1`
 * (`pick=<n>` answers a practice spot with its n-th choice, from 0), `task=<n>` (a round opens at its n-th task, from 0),
 * `focus=<spot key>` (a round on one kind of spot), `weak=1` (scrolled to the weaknesses), `spots=run` (a round of the
 * learning spots), `spots-topic=<topic>` (a round of one topic), `spots-seen=1` (some spots already answered), `spots-range=1` (the range
 * of an answered learning spot open).
 * Always null in the app.
 */
export function preview(name: string): string | null {
  if (inTauri || typeof window === "undefined") return null;
  return new URLSearchParams(window.location.search).get(name);
}

// Screenshots should show settled states, not frames of an entry animation.
if (preview("learn") !== null && typeof document !== "undefined") {
  const style = document.createElement("style");
  style.textContent = "*, *::before, *::after { animation: none !important; transition: none !important; }";
  document.head.append(style);
}

/** A plausible answer for `answer=1`, so the feedback state can be shown. */
export function previewAnswer(spec: AnswerSpec): LearnAnswer {
  switch (spec.type) {
    case "choice":
      return { choice: spec.options[0].id };
    case "cards":
      return { cards: spec.from.slice(0, Math.max(1, Math.floor(spec.from.length / 3))) };
    case "seat":
      return { seat: spec.seats[0] };
    case "number":
      return { number: Math.round((spec.min + (spec.max - spec.min) * 0.4) / spec.step) * spec.step };
  }
}
