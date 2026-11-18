import { SOURCE as KIT_SOURCE, type Grade, type Source as KitSource } from "../ui/poker";
import type { Rating, Source } from "./types";

/** The analysis' ratings as the kit's grades (the kit adds "Auch richtig" for mixed strategies). */
const GRADE_OF: Record<Rating, Grade> = {
  best: "best",
  good: "good",
  inaccuracy: "inaccurate",
  mistake: "mistake",
  blunder: "blunder",
};

export const gradeOf = (r: Rating): Grade => GRADE_OF[r];

/** The kit's source note for an analysis source, when the kit knows it. */
export function kitSource(s: Source | null): KitSource | undefined {
  return s && s in KIT_SOURCE ? (s as KitSource) : undefined;
}
