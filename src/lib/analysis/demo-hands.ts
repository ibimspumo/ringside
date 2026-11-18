// The demo hands beside the main one and what the real analysis says about each of them
// (crates/ringside-analysis on these hands), so every source of a rating can be seen in the
// browser preview with true numbers.
import type { Hand } from "../types";
import type { Decision } from "./types";

export const DEMO_HANDS: Hand[] = [
  {
    "id": 250000000001,
    "game": "tournament",
    "tournament_id": 4046789300,
    "variant": "Hold'em No Limit",
    "table_name": "4046789300 10",
    "max_seats": 6,
    "button_seat": 4,
    "small_blind": 25000,
    "big_blind": 50000,
    "currency": null,
    "played_at": "2024/01/15 21:05:10",
    "seats": [
      {
        "bounty": null,
        "name": "Alice",
        "seat": 1,
        "sitting_out": false,
        "stack": 620000
      },
      {
        "bounty": null,
        "name": "Bob",
        "seat": 2,
        "sitting_out": false,
        "stack": 980000
      },
      {
        "bounty": null,
        "name": "Carol",
        "seat": 3,
        "sitting_out": false,
        "stack": 740000
      },
      {
        "bounty": null,
        "name": "Hero",
        "seat": 4,
        "sitting_out": false,
        "stack": 500000
      },
      {
        "bounty": null,
        "name": "Dave",
        "seat": 5,
        "sitting_out": false,
        "stack": 1120000
      },
      {
        "bounty": null,
        "name": "Eve",
        "seat": 6,
        "sitting_out": false,
        "stack": 830000
      }
    ],
    "hero": "Hero",
    "hero_cards": [
      "Ah",
      "7c"
    ],
    "board": [],
    "actions": [
      {
        "all_in": false,
        "amount": 6000,
        "kind": "post_ante",
        "player": "Alice",
        "street": "preflop",
        "to": null
      },
      {
        "all_in": false,
        "amount": 6000,
        "kind": "post_ante",
        "player": "Bob",
        "street": "preflop",
        "to": null
      },
      {
        "all_in": false,
        "amount": 6000,
        "kind": "post_ante",
        "player": "Carol",
        "street": "preflop",
        "to": null
      },
      {
        "all_in": false,
        "amount": 6000,
        "kind": "post_ante",
        "player": "Hero",
        "street": "preflop",
        "to": null
      },
      {
        "all_in": false,
        "amount": 6000,
        "kind": "post_ante",
        "player": "Dave",
        "street": "preflop",
        "to": null
      },
      {
        "all_in": false,
        "amount": 6000,
        "kind": "post_ante",
        "player": "Eve",
        "street": "preflop",
        "to": null
      },
      {
        "all_in": false,
        "amount": 25000,
        "kind": "post_small_blind",
        "player": "Dave",
        "street": "preflop",
        "to": null
      },
      {
        "all_in": false,
        "amount": 50000,
        "kind": "post_big_blind",
        "player": "Eve",
        "street": "preflop",
        "to": null
      },
      {
        "all_in": false,
        "amount": 0,
        "kind": "fold",
        "player": "Alice",
        "street": "preflop",
        "to": null
      },
      {
        "all_in": false,
        "amount": 0,
        "kind": "fold",
        "player": "Bob",
        "street": "preflop",
        "to": null
      },
      {
        "all_in": false,
        "amount": 0,
        "kind": "fold",
        "player": "Carol",
        "street": "preflop",
        "to": null
      },
      {
        "all_in": false,
        "amount": 0,
        "kind": "fold",
        "player": "Hero",
        "street": "preflop",
        "to": null
      },
      {
        "all_in": false,
        "amount": 100000,
        "kind": "raise",
        "player": "Dave",
        "street": "preflop",
        "to": 125000
      },
      {
        "all_in": false,
        "amount": 0,
        "kind": "fold",
        "player": "Eve",
        "street": "preflop",
        "to": null
      }
    ],
    "returned": [
      [
        "Dave",
        75000
      ]
    ],
    "shown": [],
    "collected": [
      [
        "Dave",
        136000
      ]
    ],
    "showdown": false,
    "total_pot": 136000,
    "rake": 0,
    "bounties_won": [],
    "bounty_currency": null
  },
  {
    "id": 250000000000,
    "game": "cash",
    "tournament_id": null,
    "variant": "Hold'em No Limit",
    "table_name": "Aquila II",
    "max_seats": 6,
    "button_seat": 4,
    "small_blind": 500,
    "big_blind": 1000,
    "currency": "USD",
    "played_at": "2024/01/15 20:20:10",
    "seats": [
      {
        "bounty": null,
        "name": "Alice",
        "seat": 1,
        "sitting_out": false,
        "stack": 100000
      },
      {
        "bounty": null,
        "name": "Bob",
        "seat": 2,
        "sitting_out": false,
        "stack": 100000
      },
      {
        "bounty": null,
        "name": "Carol",
        "seat": 3,
        "sitting_out": false,
        "stack": 100000
      },
      {
        "bounty": null,
        "name": "Hero",
        "seat": 4,
        "sitting_out": false,
        "stack": 100000
      },
      {
        "bounty": null,
        "name": "Dave",
        "seat": 5,
        "sitting_out": false,
        "stack": 100000
      },
      {
        "bounty": null,
        "name": "Eve",
        "seat": 6,
        "sitting_out": false,
        "stack": 100000
      }
    ],
    "hero": "Hero",
    "hero_cards": [
      "Ah",
      "Qd"
    ],
    "board": [
      "Qs",
      "7c",
      "2d",
      "5h",
      "Jc"
    ],
    "actions": [
      {
        "all_in": false,
        "amount": 500,
        "kind": "post_small_blind",
        "player": "Dave",
        "street": "preflop",
        "to": null
      },
      {
        "all_in": false,
        "amount": 1000,
        "kind": "post_big_blind",
        "player": "Eve",
        "street": "preflop",
        "to": null
      },
      {
        "all_in": false,
        "amount": 0,
        "kind": "fold",
        "player": "Alice",
        "street": "preflop",
        "to": null
      },
      {
        "all_in": false,
        "amount": 0,
        "kind": "fold",
        "player": "Bob",
        "street": "preflop",
        "to": null
      },
      {
        "all_in": false,
        "amount": 0,
        "kind": "fold",
        "player": "Carol",
        "street": "preflop",
        "to": null
      },
      {
        "all_in": false,
        "amount": 2500,
        "kind": "raise",
        "player": "Hero",
        "street": "preflop",
        "to": 2500
      },
      {
        "all_in": false,
        "amount": 0,
        "kind": "fold",
        "player": "Dave",
        "street": "preflop",
        "to": null
      },
      {
        "all_in": false,
        "amount": 1500,
        "kind": "call",
        "player": "Eve",
        "street": "preflop",
        "to": null
      },
      {
        "all_in": false,
        "amount": 0,
        "kind": "check",
        "player": "Eve",
        "street": "flop",
        "to": null
      },
      {
        "all_in": false,
        "amount": 1800,
        "kind": "bet",
        "player": "Hero",
        "street": "flop",
        "to": null
      },
      {
        "all_in": false,
        "amount": 1800,
        "kind": "call",
        "player": "Eve",
        "street": "flop",
        "to": null
      },
      {
        "all_in": false,
        "amount": 0,
        "kind": "check",
        "player": "Eve",
        "street": "turn",
        "to": null
      },
      {
        "all_in": false,
        "amount": 0,
        "kind": "check",
        "player": "Hero",
        "street": "turn",
        "to": null
      },
      {
        "all_in": false,
        "amount": 6000,
        "kind": "bet",
        "player": "Eve",
        "street": "river",
        "to": null
      },
      {
        "all_in": false,
        "amount": 6000,
        "kind": "call",
        "player": "Hero",
        "street": "river",
        "to": null
      }
    ],
    "returned": [],
    "shown": [
      [
        "Eve",
        [
          "Qc",
          "Tc"
        ]
      ],
      [
        "Hero",
        [
          "Ah",
          "Qd"
        ]
      ]
    ],
    "collected": [
      [
        "Hero",
        20100
      ]
    ],
    "showdown": true,
    "total_pot": 21100,
    "rake": 1000,
    "bounties_won": [],
    "bounty_currency": null
  },
  {
    "id": 249999999999,
    "game": "tournament",
    "tournament_id": 4046789300,
    "variant": "Hold'em No Limit",
    "table_name": "4046789300 10",
    "max_seats": 6,
    "button_seat": 3,
    "small_blind": 30000,
    "big_blind": 60000,
    "currency": null,
    "played_at": "2024/01/15 21:15:10",
    "seats": [
      {
        "bounty": null,
        "name": "Alice",
        "seat": 1,
        "sitting_out": false,
        "stack": 620000
      },
      {
        "bounty": null,
        "name": "Bob",
        "seat": 2,
        "sitting_out": false,
        "stack": 980000
      },
      {
        "bounty": null,
        "name": "Carol",
        "seat": 3,
        "sitting_out": false,
        "stack": 740000
      },
      {
        "bounty": null,
        "name": "Hero",
        "seat": 4,
        "sitting_out": false,
        "stack": 65000
      },
      {
        "bounty": null,
        "name": "Dave",
        "seat": 5,
        "sitting_out": false,
        "stack": 1120000
      },
      {
        "bounty": null,
        "name": "Eve",
        "seat": 6,
        "sitting_out": false,
        "stack": 830000
      }
    ],
    "hero": "Hero",
    "hero_cards": [
      "8d",
      "3s"
    ],
    "board": [
      "Kd",
      "9s",
      "4c",
      "2h",
      "Jd"
    ],
    "actions": [
      {
        "all_in": false,
        "amount": 7500,
        "kind": "post_ante",
        "player": "Alice",
        "street": "preflop",
        "to": null
      },
      {
        "all_in": false,
        "amount": 7500,
        "kind": "post_ante",
        "player": "Bob",
        "street": "preflop",
        "to": null
      },
      {
        "all_in": false,
        "amount": 7500,
        "kind": "post_ante",
        "player": "Carol",
        "street": "preflop",
        "to": null
      },
      {
        "all_in": false,
        "amount": 7500,
        "kind": "post_ante",
        "player": "Hero",
        "street": "preflop",
        "to": null
      },
      {
        "all_in": false,
        "amount": 7500,
        "kind": "post_ante",
        "player": "Dave",
        "street": "preflop",
        "to": null
      },
      {
        "all_in": false,
        "amount": 7500,
        "kind": "post_ante",
        "player": "Eve",
        "street": "preflop",
        "to": null
      },
      {
        "all_in": false,
        "amount": 30000,
        "kind": "post_small_blind",
        "player": "Hero",
        "street": "preflop",
        "to": null
      },
      {
        "all_in": false,
        "amount": 60000,
        "kind": "post_big_blind",
        "player": "Dave",
        "street": "preflop",
        "to": null
      },
      {
        "all_in": false,
        "amount": 0,
        "kind": "fold",
        "player": "Eve",
        "street": "preflop",
        "to": null
      },
      {
        "all_in": false,
        "amount": 0,
        "kind": "fold",
        "player": "Alice",
        "street": "preflop",
        "to": null
      },
      {
        "all_in": false,
        "amount": 150000,
        "kind": "raise",
        "player": "Bob",
        "street": "preflop",
        "to": 150000
      },
      {
        "all_in": false,
        "amount": 0,
        "kind": "fold",
        "player": "Carol",
        "street": "preflop",
        "to": null
      },
      {
        "all_in": true,
        "amount": 27500,
        "kind": "call",
        "player": "Hero",
        "street": "preflop",
        "to": null
      },
      {
        "all_in": false,
        "amount": 0,
        "kind": "fold",
        "player": "Dave",
        "street": "preflop",
        "to": null
      }
    ],
    "returned": [
      [
        "Bob",
        90000
      ]
    ],
    "shown": [
      [
        "Bob",
        [
          "Ac",
          "Td"
        ]
      ],
      [
        "Hero",
        [
          "8d",
          "3s"
        ]
      ]
    ],
    "collected": [
      [
        "Bob",
        222500
      ]
    ],
    "showdown": true,
    "total_pot": 222500,
    "rake": 0,
    "bounties_won": [],
    "bounty_currency": null
  }
];

export const DEMO_DECISIONS: Record<number, Decision[]> = {
  "250000000002": [
    {
      "action_index": 6,
      "best": "Raise auf 4,5 BB",
      "engine": "demo",
      "ev_loss_bb": 1.52,
      "hand_id": 250000000002,
      "note": "Preflop-Modell (kein Gleichgewicht des echten Spiels): 6 Spieler, alle mit 100 BB, ohne Ante; nächste vorgerechnete Tiefe: 100 BB statt 110,7 BB.",
      "played": "Call 0,5 BB",
      "played_freq": 0.008,
      "pot_bb": 2.5,
      "rating": "blunder",
      "reason": null,
      "reliability": null,
      "solver": null,
      "source": "model",
      "spot": "vs_limp",
      "status": "rated",
      "strategy": [
        {
          "ev_bb": -0.5,
          "freq": 0.0,
          "kind": "fold",
          "label": "Fold",
          "played": false
        },
        {
          "ev_bb": 1.06,
          "freq": 0.008,
          "kind": "call",
          "label": "Call",
          "played": true
        },
        {
          "ev_bb": 2.58,
          "freq": 0.992,
          "kind": "raise",
          "label": "Raise auf 4,5 BB",
          "played": false
        }
      ],
      "street": "preflop",
      "why": "Gerechnet für SB mit 100 BB Tiefe."
    },
    {
      "action_index": 8,
      "best": "Bet 3 BB",
      "engine": "demo",
      "ev_loss_bb": 0.451,
      "hand_id": 250000000002,
      "note": "Einschätzung (keine Lösung): Multiway-Pot (3 Spieler am Flop) wird nicht bewertet. Ranges aus der Vorgeschichte (am Flop aus dem Preflop-Modell), Equity gegen alle Gegner zugleich, Fold Equity aus den Pot Odds der Gegner. In 73 % der Prüf-Spots lag sie höchstens eine Stufe neben dem Solver, bei guten Zügen wie bei Fehlern.",
      "played": "Check",
      "played_freq": 0.0,
      "pot_bb": 3.0,
      "rating": "blunder",
      "reason": null,
      "reliability": 0.731,
      "solver": null,
      "source": "estimate",
      "spot": "flop",
      "status": "rated",
      "strategy": [
        {
          "ev_bb": 3.052,
          "freq": 0.0,
          "kind": "check",
          "label": "Check",
          "played": true
        },
        {
          "ev_bb": 3.503,
          "freq": 0.333,
          "kind": "bet",
          "label": "Bet 0,99 BB",
          "played": false
        },
        {
          "ev_bb": 3.503,
          "freq": 0.333,
          "kind": "bet",
          "label": "Bet 2 BB",
          "played": false
        },
        {
          "ev_bb": 3.503,
          "freq": 0.333,
          "kind": "bet",
          "label": "Bet 3 BB",
          "played": false
        }
      ],
      "street": "flop",
      "why": "Deine Hand hat gegen die wahrscheinlichen Ranges (2 Gegner) etwa 93 % Equity. Stack zu Pot (SPR): 33."
    },
    {
      "action_index": 11,
      "best": "Raise auf 8 BB",
      "engine": "demo",
      "ev_loss_bb": 0.0,
      "hand_id": 250000000002,
      "note": "Einschätzung (keine Lösung): Multiway-Pot (3 Spieler am Flop) wird nicht bewertet. Ranges aus der Vorgeschichte (am Flop aus dem Preflop-Modell), Equity gegen alle Gegner zugleich, Fold Equity aus den Pot Odds der Gegner. In 73 % der Prüf-Spots lag sie höchstens eine Stufe neben dem Solver, bei guten Zügen wie bei Fehlern.",
      "played": "Raise auf 8 BB",
      "played_freq": 0.5,
      "pot_bb": 7.0,
      "rating": "best",
      "reason": null,
      "reliability": 0.731,
      "solver": null,
      "source": "estimate",
      "spot": "flop",
      "status": "rated",
      "strategy": [
        {
          "ev_bb": 8.781,
          "freq": 0.0,
          "kind": "fold",
          "label": "Fold",
          "played": false
        },
        {
          "ev_bb": 9.566,
          "freq": 0.0,
          "kind": "call",
          "label": "Call",
          "played": false
        },
        {
          "ev_bb": 11.708,
          "freq": 0.5,
          "kind": "raise",
          "label": "Raise auf 10,5 BB",
          "played": false
        },
        {
          "ev_bb": 11.708,
          "freq": 0.5,
          "kind": "raise",
          "label": "Raise auf 8 BB",
          "played": true
        }
      ],
      "street": "flop",
      "why": "Deine Hand hat gegen die wahrscheinlichen Ranges (2 Gegner) etwa 94 % Equity. Für einen Call brauchst du rund 22 % (Pot Odds). Stack zu Pot (SPR): 13,9."
    },
    {
      "action_index": 14,
      "best": "Bet 21 BB",
      "engine": "demo",
      "ev_loss_bb": 0.0,
      "hand_id": 250000000002,
      "note": "Einschätzung (keine Lösung): Multiway-Pot (3 Spieler am Flop) wird nicht bewertet. Ranges aus der Vorgeschichte (am Flop aus dem Preflop-Modell), Equity gegen alle Gegner zugleich, Fold Equity aus den Pot Odds der Gegner. In 73 % der Prüf-Spots lag sie höchstens eine Stufe neben dem Solver, bei guten Zügen wie bei Fehlern.",
      "played": "Bet 15 BB",
      "played_freq": 0.333,
      "pot_bb": 21.0,
      "rating": "mix",
      "reason": null,
      "reliability": 0.731,
      "solver": null,
      "source": "estimate",
      "spot": "turn",
      "status": "rated",
      "strategy": [
        {
          "ev_bb": 25.82,
          "freq": 0.0,
          "kind": "check",
          "label": "Check",
          "played": false
        },
        {
          "ev_bb": 29.484,
          "freq": 0.333,
          "kind": "bet",
          "label": "Bet 6,9 BB",
          "played": false
        },
        {
          "ev_bb": 29.484,
          "freq": 0.333,
          "kind": "bet",
          "label": "Bet 13,9 BB",
          "played": true
        },
        {
          "ev_bb": 29.484,
          "freq": 0.333,
          "kind": "bet",
          "label": "Bet 21 BB",
          "played": false
        }
      ],
      "street": "turn",
      "why": "Deine Hand hat gegen die wahrscheinlichen Ranges (den Gegner) etwa 98 % Equity. Stack zu Pot (SPR): 4,3."
    },
    {
      "action_index": 16,
      "best": "Bet 30 BB",
      "engine": "demo",
      "ev_loss_bb": 0.0,
      "hand_id": 250000000002,
      "note": "Einschätzung (keine Lösung): Multiway-Pot (3 Spieler am Flop) wird nicht bewertet. Ranges aus der Vorgeschichte (am Flop aus dem Preflop-Modell), Equity gegen alle Gegner zugleich, Fold Equity aus den Pot Odds der Gegner. In 73 % der Prüf-Spots lag sie höchstens eine Stufe neben dem Solver, bei guten Zügen wie bei Fehlern.",
      "played": "Bet 30 BB",
      "played_freq": 0.25,
      "pot_bb": 51.0,
      "rating": "best",
      "reason": null,
      "reliability": 0.731,
      "solver": null,
      "source": "estimate",
      "spot": "river",
      "status": "rated",
      "strategy": [
        {
          "ev_bb": 76.336,
          "freq": 0.0,
          "kind": "check",
          "label": "Check",
          "played": false
        },
        {
          "ev_bb": 85.435,
          "freq": 0.25,
          "kind": "bet",
          "label": "Bet 16,8 BB",
          "played": false
        },
        {
          "ev_bb": 85.435,
          "freq": 0.25,
          "kind": "bet",
          "label": "Bet 33,7 BB",
          "played": false
        },
        {
          "ev_bb": 85.435,
          "freq": 0.25,
          "kind": "bet",
          "label": "Bet 51 BB",
          "played": false
        },
        {
          "ev_bb": 85.435,
          "freq": 0.25,
          "kind": "bet",
          "label": "Bet 30 BB",
          "played": true
        }
      ],
      "street": "river",
      "why": "Deine Hand hat gegen die wahrscheinlichen Ranges (den Gegner) etwa 96 % Equity. Stack zu Pot (SPR): 1,5."
    },
    {
      "action_index": 18,
      "best": "All-in",
      "engine": "demo",
      "ev_loss_bb": 1.629,
      "hand_id": 250000000002,
      "note": "Einschätzung (keine Lösung): Multiway-Pot (3 Spieler am Flop) wird nicht bewertet. Ranges aus der Vorgeschichte (am Flop aus dem Preflop-Modell), Equity gegen alle Gegner zugleich, Fold Equity aus den Pot Odds der Gegner. In 73 % der Prüf-Spots lag sie höchstens eine Stufe neben dem Solver, bei guten Zügen wie bei Fehlern.",
      "played": "Call 46 BB",
      "played_freq": 0.0,
      "pot_bb": 157.0,
      "rating": "good",
      "reason": null,
      "reliability": 0.731,
      "solver": null,
      "source": "estimate",
      "spot": "river",
      "status": "rated",
      "strategy": [
        {
          "ev_bb": 98.733,
          "freq": 0.0,
          "kind": "fold",
          "label": "Fold",
          "played": false
        },
        {
          "ev_bb": 130.016,
          "freq": 0.0,
          "kind": "call",
          "label": "Call",
          "played": true
        },
        {
          "ev_bb": 131.644,
          "freq": 1.0,
          "kind": "all_in",
          "label": "All-in",
          "played": false
        }
      ],
      "street": "river",
      "why": "Deine Hand hat gegen die wahrscheinlichen Ranges (den Gegner) etwa 84 % Equity. Für einen Call brauchst du rund 23 % (Pot Odds). Stack zu Pot (SPR): 0."
    }
  ],
  "250000000001": [
    {
      "action_index": 11,
      "best": "All-in",
      "engine": "demo",
      "ev_loss_bb": 0.985,
      "hand_id": 250000000001,
      "note": "Push/Fold-Gleichgewicht in Chips (ohne ICM): 6 Spieler, alle mit 10 BB, Ante für alle; echte Stacks ungleich (10 bis 22,4 BB).",
      "played": "Fold",
      "played_freq": 0.0,
      "pot_bb": 2.22,
      "rating": "blunder",
      "reason": null,
      "reliability": null,
      "solver": null,
      "source": "equilibrium",
      "spot": "shove",
      "status": "rated",
      "strategy": [
        {
          "ev_bb": 0.86,
          "freq": 1.0,
          "kind": "all_in",
          "label": "All-in",
          "played": false
        },
        {
          "ev_bb": -0.125,
          "freq": 0.0,
          "kind": "fold",
          "label": "Fold",
          "played": true
        }
      ],
      "street": "preflop",
      "why": "Mit A7o bei 10 BB effektiv gehört deine Hand in die Shove-Range – geschoben werden hier die besten 40 % aller Hände."
    }
  ],
  "250000000000": [
    {
      "action_index": 5,
      "best": "Raise auf 2,5 BB",
      "engine": "demo",
      "ev_loss_bb": 0.0,
      "hand_id": 250000000000,
      "note": "Preflop-Modell (kein Gleichgewicht des echten Spiels): 6 Spieler, alle mit 100 BB, ohne Ante.",
      "played": "Raise auf 2,5 BB",
      "played_freq": 0.992,
      "pot_bb": 1.5,
      "rating": "best",
      "reason": null,
      "reliability": null,
      "solver": null,
      "source": "model",
      "spot": "steal",
      "status": "rated",
      "strategy": [
        {
          "ev_bb": 0.0,
          "freq": 0.0,
          "kind": "fold",
          "label": "Fold",
          "played": false
        },
        {
          "ev_bb": 1.69,
          "freq": 0.008,
          "kind": "limp",
          "label": "Limp",
          "played": false
        },
        {
          "ev_bb": 1.53,
          "freq": 0.992,
          "kind": "raise",
          "label": "Raise auf 2,5 BB",
          "played": true
        }
      ],
      "street": "preflop",
      "why": "Gerechnet für BTN mit 100 BB Tiefe."
    },
    {
      "action_index": 9,
      "best": "Bet 1,8 BB",
      "engine": "demo",
      "ev_loss_bb": 0.0,
      "hand_id": 250000000000,
      "note": "Postflop-Solver heads-up. Ranges aus dem Preflop-Modell (kein Gleichgewicht des echten Spiels): 6 Spieler, alle mit 100 BB, ohne Ante. Einsatzgrößen Flop 33 %, Raise 60 %, Turn 75 %, River 66 %/75 %, All-in ab 150 % Pot. Gelöst bis 0,9 % des Pots Ausbeutbarkeit (80 Iterationen).",
      "played": "Bet 1,8 BB",
      "played_freq": 1.0,
      "pot_bb": 5.5,
      "rating": "best",
      "reason": null,
      "reliability": null,
      "solver": "{\"allin_threshold\":150.0,\"board\":[\"Qs\",\"7c\",\"2d\"],\"hero\":{\"cards\":[\"Ah\",\"Qd\"],\"player\":\"ip\"},\"ip\":{\"note\":\"BTN: Raise auf 2,5 BB\",\"position\":\"BTN\",\"range\":\"77+, 66-22:0.95, ATs+, A9s-A2s:0.95, KQs, KJs-K2s:0.95, Q5s+:0.95, Q4s:0.55, J8s+:0.95, J7s:0.35, T8s+:0.95, T7s:0.05, 98s:0.95, 97s:0.2, 87s, 76s, AJo+, ATo-A2o:0.95, K8o+:0.95, K7o:0.35, Q9o+:0.95, JTo:0.95\"},\"oop\":{\"note\":\"BB: Call\",\"position\":\"BB\",\"range\":\"33:0.95, 22, A9s:0.75, A8s-A6s, A5s:0.75, A4s-A2s, KTs:0.7, K9s-K2s, Q9s:0.6, Q8s-Q2s, J8s:0.7, J7s-J5s, J4s:0.15, T8s:0.9, T7s-T6s, 97s-96s, 87s:0.5, 86s-85s, 74s+, 64s+, 53s+, 43s, ATo-A2o, KJo-K7o, K6o:0.2, Q9o+, JTo:0.45, J9o:0.75, T9o\"},\"path\":[{\"action\":0}],\"players\":6,\"pot\":5.5,\"saved\":null,\"sizes\":[{\"bet\":[33.0],\"raise\":[60.0]},{\"bet\":[75.0],\"raise\":[]},{\"bet\":[66.0,75.0],\"raise\":[]}],\"source\":\"Hand #250000000000, Analyse\",\"stack\":97.5}",
      "source": "solver",
      "spot": "c_bet",
      "status": "rated",
      "strategy": [
        {
          "ev_bb": 7.938,
          "freq": 0.0,
          "kind": "check",
          "label": "Check",
          "played": false
        },
        {
          "ev_bb": 8.1,
          "freq": 1.0,
          "kind": "bet",
          "label": "Bet 1,8 BB",
          "played": true
        }
      ],
      "street": "flop",
      "why": "Deine Hand hat hier gegen die Range des Gegners etwa 89 % Equity."
    },
    {
      "action_index": 12,
      "best": "Bet 6,9 BB",
      "engine": "demo",
      "ev_loss_bb": 0.926,
      "hand_id": 250000000000,
      "note": "Postflop-Solver heads-up. Ranges aus dem Preflop-Modell (kein Gleichgewicht des echten Spiels): 6 Spieler, alle mit 100 BB, ohne Ante. Einsatzgrößen Flop 33 %, Raise 60 %, Turn 75 %, River 66 %/75 %, All-in ab 150 % Pot. Gelöst bis 0,9 % des Pots Ausbeutbarkeit (80 Iterationen).",
      "played": "Check",
      "played_freq": 0.0,
      "pot_bb": 9.14,
      "rating": "mistake",
      "reason": null,
      "reliability": null,
      "solver": "{\"allin_threshold\":150.0,\"board\":[\"Qs\",\"7c\",\"2d\"],\"hero\":{\"cards\":[\"Ah\",\"Qd\"],\"player\":\"ip\"},\"ip\":{\"note\":\"BTN: Raise auf 2,5 BB\",\"position\":\"BTN\",\"range\":\"77+, 66-22:0.95, ATs+, A9s-A2s:0.95, KQs, KJs-K2s:0.95, Q5s+:0.95, Q4s:0.55, J8s+:0.95, J7s:0.35, T8s+:0.95, T7s:0.05, 98s:0.95, 97s:0.2, 87s, 76s, AJo+, ATo-A2o:0.95, K8o+:0.95, K7o:0.35, Q9o+:0.95, JTo:0.95\"},\"oop\":{\"note\":\"BB: Call\",\"position\":\"BB\",\"range\":\"33:0.95, 22, A9s:0.75, A8s-A6s, A5s:0.75, A4s-A2s, KTs:0.7, K9s-K2s, Q9s:0.6, Q8s-Q2s, J8s:0.7, J7s-J5s, J4s:0.15, T8s:0.9, T7s-T6s, 97s-96s, 87s:0.5, 86s-85s, 74s+, 64s+, 53s+, 43s, ATo-A2o, KJo-K7o, K6o:0.2, Q9o+, JTo:0.45, J9o:0.75, T9o\"},\"path\":[{\"action\":0},{\"action\":1},{\"action\":1},{\"card\":\"5h\"},{\"action\":0}],\"players\":6,\"pot\":5.5,\"saved\":null,\"sizes\":[{\"bet\":[33.0],\"raise\":[60.0]},{\"bet\":[75.0],\"raise\":[]},{\"bet\":[66.0,75.0],\"raise\":[]}],\"source\":\"Hand #250000000000, Analyse\",\"stack\":97.5}",
      "source": "solver",
      "spot": "turn",
      "status": "rated",
      "strategy": [
        {
          "ev_bb": 12.639,
          "freq": 0.0,
          "kind": "check",
          "label": "Check",
          "played": true
        },
        {
          "ev_bb": 13.565,
          "freq": 1.0,
          "kind": "bet",
          "label": "Bet 6,9 BB",
          "played": false
        }
      ],
      "street": "turn",
      "why": "Deine Hand hat hier gegen die Range des Gegners etwa 93 % Equity."
    },
    {
      "action_index": 14,
      "best": "Raise auf 28,6 BB",
      "engine": "demo",
      "ev_loss_bb": 3.036,
      "hand_id": 250000000000,
      "note": "Einschätzung (keine Lösung): Deine Hand erreicht diesen Knoten im Solver nicht (frühere Aktion nie gespielt). Ranges aus der Vorgeschichte (am Flop aus dem Preflop-Modell), Equity gegen alle Gegner zugleich, Fold Equity aus den Pot Odds der Gegner. In 73 % der Prüf-Spots lag sie höchstens eine Stufe neben dem Solver, bei guten Zügen wie bei Fehlern.",
      "played": "Call 6 BB",
      "played_freq": 0.0,
      "pot_bb": 15.1,
      "rating": "blunder",
      "reason": null,
      "reliability": 0.731,
      "solver": null,
      "source": "estimate",
      "spot": "river",
      "status": "rated",
      "strategy": [
        {
          "ev_bb": 19.621,
          "freq": 0.0,
          "kind": "fold",
          "label": "Fold",
          "played": false
        },
        {
          "ev_bb": 23.126,
          "freq": 0.0,
          "kind": "call",
          "label": "Call",
          "played": true
        },
        {
          "ev_bb": 26.161,
          "freq": 1.0,
          "kind": "raise",
          "label": "Raise auf 28,6 BB",
          "played": false
        }
      ],
      "street": "river",
      "why": "Deine Hand hat gegen die wahrscheinlichen Ranges (den Gegner) etwa 95 % Equity. Für einen Call brauchst du rund 28 % (Pot Odds). Stack zu Pot (SPR): 5,9."
    }
  ],
  "249999999999": [
    {
      "action_index": 12,
      "best": null,
      "engine": "demo",
      "ev_loss_bb": null,
      "hand_id": 249999999999,
      "note": "",
      "played": "Call 0,46 BB (all-in)",
      "played_freq": null,
      "pot_bb": null,
      "rating": null,
      "reason": null,
      "reliability": null,
      "solver": null,
      "source": "forced",
      "spot": "vs_open",
      "status": "forced",
      "strategy": [],
      "street": "preflop",
      "why": "Nur noch ein paar Chips dahinter: Der Call kostet so wenig im Verhältnis zum Pot, dass er mit jeder Hand richtig ist."
    }
  ]
};
