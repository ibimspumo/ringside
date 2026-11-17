import { inTauri } from "../api";
import type { AnswerSpec, LearnAnswer } from "../types";

/**
 * Browser-demo switches for screenshots of states that are otherwise only reachable by clicking:
 * `learn=today|path|own|trainer|wissen`, `skill=<kind>`, `article=<id>`, `session=run|done`, `answer=1`.
 * Always null in the app.
 */
export function preview(name: string): string | null {
  if (inTauri || typeof window === "undefined") return null;
  return new URLSearchParams(window.location.search).get(name);
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
