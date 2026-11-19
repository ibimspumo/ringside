import type { Action, Hand } from "../types";

/** What each seat means, for a hover on a position name. */
export const POSITION_HELP: Record<string, string> = {
  UTG: "Under the Gun: als Erster vor dem Flop dran, nach dem Flop fast immer zuerst. Schwerste Position – hier nur starke Hände.",
  "UTG+1": "Direkt nach UTG: früh dran, fast so schwer wie UTG.",
  LJ: "Lojack: mittlere Position, drei Plätze vor dem Button.",
  HJ: "Hijack: zwei Plätze vor dem Button, schon recht späte Position.",
  CO: "Cutoff: direkt vor dem Button. Hier lohnen sich viele Hände.",
  BTN: "Button: nach dem Flop immer als Letzter dran. Beste Position am Tisch – hier gewinnen die meisten Spieler am meisten.",
  SB: "Small Blind: Pflichteinsatz und nach dem Flop zuerst dran. Fast alle Spieler verlieren hier auf Dauer.",
  BB: "Big Blind: großer Pflichteinsatz. Ein Minus ist hier normal; wichtig ist, nicht mehr als nötig zu verlieren.",
};

/** A few plain words under a position code in tables. */
export const POSITION_PLAIN: Record<string, string> = {
  UTG: "als Erster dran",
  "UTG+1": "früh",
  LJ: "Mitte",
  HJ: "Mitte, eher spät",
  CO: "vor dem Button",
  BTN: "als Letzter dran",
  SB: "kleiner Pflichteinsatz",
  BB: "großer Pflichteinsatz",
};

/**
 * Position names as the stats engine assigns them: the big blind is whoever posted it, the small
 * blind only counts when posted from the seat right before, and the button sits before that.
 */
export function positionsOf(hand: Hand): Map<string, string> {
  const active = hand.seats.filter((s) => !s.sitting_out);
  const n = active.length;
  const out = new Map<string, string>();
  if (n < 2) return out;
  const posted = (kind: Action["kind"]) => hand.actions.find((a) => a.street === "preflop" && a.kind === kind)?.player;
  let big = active.findIndex((s) => s.name === posted("post_big_blind"));
  if (big < 0) {
    const after = active.findIndex((s) => s.seat >= hand.button_seat);
    big = ((after < 0 ? 0 : after) + 2) % n;
  }
  const order = Array.from({ length: n }, (_, i) => active[(big + 1 + i) % n].name);
  if (n === 2) {
    out.set(order[0], "BTN");
    out.set(order[1], "BB");
    return out;
  }
  const tail = ["BTN"];
  if (posted("post_small_blind") === order[n - 2]) tail.push("SB");
  tail.push("BB");
  const LATE = ["LJ", "HJ", "CO"];
  const EARLY = ["UTG", "UTG+1", "UTG+2", "UTG+3", "UTG+4"];
  const rest = n - tail.length;
  const early = Math.max(0, rest - LATE.length);
  const labels = [...EARLY.slice(0, early), ...LATE.slice(LATE.length - (rest - early)), ...tail];
  order.forEach((name, i) => out.set(name, labels[i]));
  return out;
}
