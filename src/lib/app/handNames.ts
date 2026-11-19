import type { Hand } from "../types";

/**
 * GGPoker writes anonymous hex ids for every opponent ("3a9f0c21"). For display they become
 * "Gegner 1", "Gegner 2" … in seat order from the hero's left; the stored hand keeps the ids.
 */
export function readableNames(hand: Hand): Hand {
  if (hand.site !== "ggpoker") return hand;
  const hero = hand.seats.find((s) => s.name === hand.hero)?.seat ?? 0;
  const order = [...hand.seats].sort((a, b) => ((a.seat - hero + 99) % 99) - ((b.seat - hero + 99) % 99));
  const names = new Map<string, string>();
  for (const s of order) if (s.name !== hand.hero) names.set(s.name, `Gegner ${names.size + 1}`);
  const name = (n: string) => names.get(n) ?? n;
  const pairs = <T>(list: [string, T][]): [string, T][] => list.map(([n, v]) => [name(n), v]);
  return {
    ...hand,
    seats: hand.seats.map((s) => ({ ...s, name: name(s.name) })),
    actions: hand.actions.map((a) => ({ ...a, player: name(a.player) })),
    returned: pairs(hand.returned),
    shown: pairs(hand.shown),
    collected: pairs(hand.collected),
    bounties_won: pairs(hand.bounties_won),
  };
}
