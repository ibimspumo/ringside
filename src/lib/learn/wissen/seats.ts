export type Seats = 6 | 8 | 9;

/** Positions in preflop action order. */
export const POSITIONS: Record<Seats, string[]> = {
  6: ["UTG", "HJ", "CO", "BTN", "SB", "BB"],
  8: ["UTG", "UTG+1", "LJ", "HJ", "CO", "BTN", "SB", "BB"],
  9: ["UTG", "UTG+1", "UTG+2", "LJ", "HJ", "CO", "BTN", "SB", "BB"],
};

/** Postflop the blinds act first and the button last. */
export function postflopOrder(seats: Seats): string[] {
  const pre = POSITIONS[seats];
  return [...pre.slice(-2), ...pre.slice(0, -2)];
}

export const POSITION_INFO: Record<string, { name: string; text: string }> = {
  UTG: { name: "Under the Gun", text: "Preflop zuerst dran, alle anderen sitzen noch hinter dir. Hier spielst du am engsten." },
  "UTG+1": { name: "Frühe Position", text: "Fast alle kommen noch nach dir. Nur starke Hände eröffnen." },
  "UTG+2": { name: "Frühe Position", text: "Noch viele Spieler hinter dir. Die Range bleibt eng." },
  LJ: { name: "Lojack", text: "Mittlere Position. Die Range wird langsam breiter." },
  HJ: { name: "Hijack", text: "Zwei Plätze vor dem Button. Hier beginnen viele Steals." },
  CO: { name: "Cutoff", text: "Direkt vor dem Button. Nur noch drei Spieler hinter dir." },
  BTN: { name: "Button", text: "Ab dem Flop immer zuletzt dran. Die beste Position am Tisch." },
  SB: { name: "Small Blind", text: "Zahlt einen halben Blind und handelt ab dem Flop zuerst. Der schwierigste Sitz." },
  BB: { name: "Big Blind", text: "Zahlt den vollen Blind und handelt preflop zuletzt. Verteidigt oft zu einem guten Preis." },
};
