import { api } from "../api";
import type { SkillProfile } from "./types";

/**
 * The Spielwertung shared by the top bar and the profile page. `locked` while PokerStars runs:
 * the backend gives no rating then, and nothing of the last one stays on screen.
 */
export const skill = $state<{ profile: SkillProfile | null; state: "loading" | "ready" | "locked" | "error" }>({
  profile: null,
  state: "loading",
});

let pending: Promise<void> | null = null;

export function loadSkill(): Promise<void> {
  pending ??= api
    .skillProfile()
    .then((p) => {
      skill.profile = p;
      skill.state = "ready";
      // A recalculation takes seconds to a minute; then the rating is back.
      if (p.recalculating) setTimeout(() => void loadSkill(), 3000);
    })
    .catch((e) => {
      skill.profile = null;
      skill.state = String(e).includes("gesperrt") ? "locked" : "error";
    })
    .finally(() => (pending = null));
  return pending;
}
