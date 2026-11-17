/** Display names of the coach's concepts; the list itself lives in crates/ringside-coach/src/concepts.rs. */
export const CONCEPT_NAMES: Record<string, string> = {
  push_fold_sb: "Push/Fold im Small Blind",
  push_fold_call: "Callen gegen einen Shove",
  short_stack: "Kurzer Stack außerhalb der Blinds",
  pot_odds: "Pot Odds",
  equity: "Equity gegen Ranges",
  outs: "Outs und Draws",
  position: "Position",
  preflop_range: "Starthände und Ranges",
  fold_equity: "Fold Equity und Semi-Bluffs",
  value_bet: "Value Bets und Bluffs",
  cbet: "Continuation Bet",
  stack_depth: "Stacktiefe und SPR",
  icm: "ICM und Bubble",
  bounty: "Bounty-Turniere",
  all_in_ev: "All-in-EV und Varianz",
};

export const conceptName = (id: string) => CONCEPT_NAMES[id] ?? id;
