/** „Was als Nächstes?“: the one ranked list of the backend (crates/ringside-core/src/next.rs). */
import type { LearnTarget } from "../app/Leaks.svelte";
import type { SpotType } from "../analysis/types";
import type { SpotFilter } from "../types";
import type { WordId } from "../ui/words";

export type StepKind = "spot" | "leak" | "decision" | "concept" | "review";

/** Which hands „Hände ansehen“ opens. */
export type StepHands =
  | { by: "spot"; key: string; label: string }
  | { by: "decision"; hand_id: number; action_index: number }
  | ({ by: "stat"; label: string } & SpotFilter);

export interface NextStep {
  id: string;
  kind: StepKind;
  /** Plain German: „Gegen eine Erhöhung im Big Blind“. */
  title: string;
  /** One sentence why. */
  why: string;
  /** The number with its meaning: „kostet dich im Schnitt etwa 0,4 BB je Entscheidung, 31-mal gespielt“. */
  number: string;
  /** Glossary id of the poker word in the title. */
  term: WordId | null;
  per_time_bb: number | null;
  times: number | null;
  total_bb: number | null;
  trust: number;
  fresh: number;
  /** The number rests on an estimate, not on a computed solution. */
  estimate: boolean;
  score: number;
  spot_key: string | null;
  /** Spot types of the analysis it is about, for the hand list's filter. */
  spots: SpotType[];
  hand_id: number | null;
  practice: LearnTarget | null;
  hands: StepHands | null;
  /** Article id in „Wissen“. */
  read: string | null;
}
