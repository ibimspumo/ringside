import type { CloudSettings, MalformedHand } from "./types";

/** Browser preview: ?cloudseen=1 hides the first-start notice. */
let store: CloudSettings = {
  fetch: true,
  share: true,
  notice_seen: new URLSearchParams(window.location.search).has("cloudseen"),
  practice_ready: true,
};

let hands: MalformedHand[] = [
  {
    id: 1,
    reason: "stack",
    hand: [
      "PokerStars Hand #1: Tournament #1, $5+$0.50 USD Hold'em No Limit - Level V (50/100) - 2026/11/16 20:10:00 CET [2026/11/16 14:10:00 ET]",
      "Table 'Tisch' 8-max Seat #2 is the button",
      "Seat 1: Hero (8725 in chips, $2.50 bounty)",
      "Seat 2: Spieler 1 (4618 in chips, $1.25 bounty)",
      "Seat 3: Spieler 2 (1200 in chips, $1.90 bounty)",
      "Hero: posts the ante 12",
      "Spieler 1: posts the ante 12",
      "Spieler 2: posts the ante 12",
      "Spieler 2: posts small blind 50",
      "Hero: posts big blind 100",
      "*** HOLE CARDS ***",
      "Dealt to Hero [Ah Ad]",
      "Spieler 1: folds",
      "Spieler 2: raises 1088 to 1188 and is all-in",
      "Hero: calls 1088",
    ].join("\n"),
  },
];

export const settings = () => structuredClone(store);
export const saveSettings = (cloud: CloudSettings) => {
  store = { ...cloud, notice_seen: true };
};
export const ackNotice = () => {
  store = { ...store, notice_seen: true };
};
export const malformed = () => structuredClone(hands);
export const dismiss = (id: number) => {
  hands = hands.filter((h) => h.id !== id);
};
export const sendReport = (id: number) => dismiss(id);
