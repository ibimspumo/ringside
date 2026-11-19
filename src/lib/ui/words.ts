// The glossary: every poker word the app shows, with the word it is shown as and its
// explanation. Pages take words and explanations from here, through `Term` or `explain`.
// Texts built in Rust use a mirror of the display words (crates/ringside-core/src/words.rs);
// a test there keeps both equal, so each entry starts with `id: {` and `word:` on the next line.

export type Word = {
  /** As shown in the app. */
  word: string;
  /** One sentence. */
  short: string;
  /** More for whoever wants it. */
  long?: string;
};

export const WORDS = {
  // Pages and menus.
  leaks: {
    word: "Schwachstellen",
    short: "Wo du im Vergleich zu üblichen Werten oder zur Lösung regelmäßig Chips verlierst.",
    long: "In Profi-Tools heißt das „Leaks“: kleine Löcher, durch die auf Dauer Geld abfließt.",
  },
  solver: {
    word: "Nachrechnen",
    short: "Ringside rechnet eine Hand ab dem Flop genau durch und zeigt, wie man sie am besten spielt.",
    long: "Das Werkzeug dafür heißt Solver: Es sucht die Strategie, die kein Gegner mehr ausnutzen kann.",
  },

  skill_rating: {
    word: "Spielwertung",
    short: "Eine Zahl wie im Schach: Sie zeigt, wie gut du entscheidest, und steigt oder fällt nach jedem analysierten Turnier. Gezählt wird die Entscheidung, nicht ob die Karten gehalten haben.",
    long: "Was eine Entscheidung im Schnitt kostet, zählt als Anteil deines Stacks: Ein falsches All-in wiegt viel, ein kleiner Fehler vor dem Flop wenig, leichte Folds gar nicht.",
  },
  skill_spread: {
    word: "Unsicherheit",
    short: "Wie weit die wahre Zahl noch danebenliegen kann. Am Anfang groß, mit jedem Turnier kleiner; nach einer langen Pause wieder größer.",
  },

  // Money and results.
  bb: {
    word: "BB",
    short: "BB = Big Blind, der große Pflichteinsatz. Ergebnisse in Big Blinds lassen sich über alle Limits und Turnierstufen vergleichen.",
  },
  ev_loss: {
    word: "kostet im Schnitt",
    short: "Wie viele Big Blinds deine Wahl auf lange Sicht im Schnitt weniger einbringt als die beste – nicht in dieser einen Hand.",
    long: "Profis nennen das EV-Verlust.",
  },
  ev: {
    word: "EV",
    short: "EV (Erwartungswert): was eine Hand oder Aktion im Schnitt einbringt, in Big Blinds. Höher ist besser.",
    long: "Wovon gezählt wird, steht jeweils dabei: ab der Entscheidung (Folden ist dann 0) oder ab deinem Stack vor der Hand (dann kostet Folden den schon gesetzten Blind).",
  },
  winrate: {
    word: "Winrate",
    short: "Wie viele Big Blinds du im Schnitt je 100 Hände gewinnst (BB/100). Über 0 heißt: Du gewinnst auf Dauer, unter 0: Du verlierst.",
  },
  all_in_ev: {
    word: "All-in-EV",
    short: "Dein Ergebnis, wenn jedes All-in mit offenen Karten so ausgegangen wäre, wie es im Schnitt ausgeht.",
    long: "Ringside rechnet je Side Pot die exakte Equity × Pot nach Rake − deinen Einsatz. Bounties zählen nicht mit.",
  },
  luck: {
    word: "Glück",
    short: "Tatsächliches Ergebnis − EV in deinen All-ins. Positiv heißt, du hast öfter gewonnen, als die Karten erwarten ließen.",
  },
  showdown: {
    word: "Mit Showdown",
    short: "Am Ende der Hand werden die Karten aufgedeckt. Diese Linie zeigt, was du in solchen Händen gewonnen oder verloren hast.",
  },
  no_showdown: {
    word: "Ohne Showdown",
    short: "Pots, die enden, bevor Karten aufgedeckt werden – weil jemand foldet. Steigt die Linie, gewinnst du Pots durch Druck; fällt sie, gibst du oft auf.",
  },
  bounty: {
    word: "Bounty",
    short: "Geld, das du bekommst, wenn du in einem Knockout-Turnier einen Spieler rauswirfst.",
  },
  roi: {
    word: "ROI",
    short: "ROI (Return on Investment): Gewinn je eingesetztem Buy-in. +10 % heißt: Für 1 $ Buy-in kamen im Schnitt 1,10 $ zurück.",
  },
  itm: {
    word: "ITM",
    short: "ITM (In the Money): Anteil der Turniere, in denen du Preisgeld gewonnen hast.",
  },
  buy_in: {
    word: "Buy-in",
    short: "Was du für die Teilnahme zahlst, mit Gebühr, Bounty-Anteil und Re-Entrys.",
  },
  icm: {
    word: "ICM",
    short: "ICM rechnet Chips in Preisgeld um. Kurz vor dem Geld ist ein Chip, den du verlierst, mehr wert als einer, den du gewinnst.",
  },

  // Stats of the HUD and the leak finder.
  hands: {
    word: "Hände",
    short: "Gemeinsame Hände – je mehr, desto verlässlicher die Werte.",
  },
  vpip: {
    word: "VPIP",
    short: "Wie oft freiwillig Geld in den Pot gelegt wird. Hoch = spielt viele Hände (loose).",
    long: "Die Blinds zählen nicht, nur Calls und Raises vor dem Flop.",
  },
  pfr: {
    word: "Preflop-Raise",
    short: "Wie oft vor dem Flop erhöht wird. Nah an VPIP = aggressiv, weit darunter = passiv.",
  },
  three_bet: {
    word: "3-Bet",
    short: "Die zweite Erhöhung vor dem Flop: Jemand erhöht gegen einen Raise noch einmal (Re-Raise).",
    long: "Darauf folgt die 4-Bet. Wer 3-bettet, hat meist eine starke Hand oder blufft gezielt mit Händen, die gut blocken.",
  },
  fold_to_three_bet: {
    word: "Fold auf 3-Bet",
    short: "Wie oft nach eigenem Raise auf ein Re-Raise gefoldet wird.",
  },
  steal: {
    word: "Steal",
    short: "Blinds klauen: Open-Raise aus Cutoff, Button oder Small Blind, wenn alle davor gefoldet haben.",
  },
  fold_to_steal: {
    word: "Fold auf Steal",
    short: "Wie oft die Blinds gegen einen Steal folden.",
  },
  cbet: {
    word: "C-Bet",
    short: "Nachsetzen am Flop: Wer vor dem Flop erhöht hat, setzt am Flop gleich wieder (Continuation-Bet).",
  },
  fold_to_cbet: {
    word: "Fold auf C-Bet",
    short: "Wie oft auf eine C-Bet am Flop gefoldet wird.",
  },
  wtsd: {
    word: "Showdown",
    short: "Wie oft es nach gesehenem Flop zum Showdown geht.",
  },
  wsd: {
    word: "Gewonnen am Showdown",
    short: "Anteil gewonnener Showdowns.",
  },
  af: {
    word: "Aggression",
    short: "Aggressionsfaktor nach dem Flop: (Bets + Raises) ÷ Calls. 2,8× heißt: auf jeden Call kommen 2,8 Bets oder Raises.",
    long: "Unter 1,5 passiv, 1,5–3,5 normal, über 3,5 sehr aggressiv. Checks und Folds zählen nicht mit.",
  },
  afq: {
    word: "Aggressions-Frequenz",
    short: "Anteil aggressiver Aktionen nach dem Flop.",
  },

  // Spots and solutions.
  equity: {
    word: "Equity",
    short: "Wie oft eine Hand den Pot gewinnen würde, wenn jetzt alle Karten ohne weitere Einsätze aufgedeckt würden – gegen alle Hände, die der Gegner hier haben kann.",
  },
  range: {
    word: "Range",
    short: "Alle Hände, die ein Spieler an dieser Stelle haben kann, gewichtet danach, wie oft er bis hierher so spielt.",
  },
  pot_odds: {
    word: "Pot Odds",
    short: "Wie viel Equity ein Call mindestens braucht: der Call geteilt durch den Pot nach dem Call.",
  },
  spr: {
    word: "SPR",
    short: "SPR (Stack-to-Pot-Ratio): effektiver Stack geteilt durch den Pot.",
    long: "Unter 3 ist mit ein, zwei Einsätzen alles drin; über 8 bleibt viel Spielraum für Turn und River.",
  },
  effective: {
    word: "effektiv",
    short: "Effektiver Stack: der kleinere der beiden Stacks. Mehr kann in dieser Hand keiner gewinnen oder verlieren.",
  },
  oop: {
    word: "OOP",
    short: "OOP („out of position“, ohne Position): handelt nach dem Flop auf jeder Straße zuerst und sieht vorher nicht, was der Gegner tut – ein Nachteil.",
  },
  ip: {
    word: "IP",
    short: "IP („in position“, mit Position): handelt nach dem Flop als Letzter und weiß schon, was der Gegner getan hat – ein Vorteil.",
  },
  exploit: {
    word: "Genauigkeit",
    short: "So viel (in % des Pots) könnte ein perfekter Gegner gegen die Lösung im Schnitt mehr gewinnen. Je kleiner, desto genauer.",
    long: "Unter 1 % ist zum Lernen mehr als genug.",
  },
  mixed: {
    word: "gemischt",
    short: "Die Lösung spielt dieselbe Hand mal so, mal so. Beide Aktionen sind dann etwa gleich viel wert; das Mischen macht dich für den Gegner schwer lesbar.",
  },

  // Exercises.
  nash: {
    word: "Nash",
    short: "Die Entscheidung, die ein perfekt spielender Gegner nicht ausnutzen kann.",
  },
  expected: {
    word: "Erwartet (EV)",
    short: "Was dein All-in im Schnitt wert war, unabhängig davon, wie die Karten fielen.",
  },
  shove_range: {
    word: "Shove-Range",
    short: "Anteil aller Starthände, mit denen man hier all-in geht bzw. callt.",
  },
  open_range: {
    word: "Open-Range",
    short: "Anteil aller Starthände, mit denen man hier als Erster erhöht.",
  },
  needed_equity: {
    word: "Benötigte Equity",
    short: "So oft musst du mindestens gewinnen, damit sich der Call lohnt.",
  },
  dead_money: {
    word: "Totes Geld",
    short: "Chips im Pot von Spielern, die schon gefoldet haben.",
  },
  outs: {
    word: "Outs",
    short: "Karten, die deine Hand nach vorn bringen.",
  },
  dirty_outs: {
    word: "Schmutzige Outs",
    short: "Karten, die dir helfen, dem Gegner aber eine noch bessere Hand geben können.",
  },
  half_outs: {
    word: "Halbe Outs",
    short: "Karten, die dir nur manchmal helfen. Sie zählen halb.",
  },
  rule_2_4: {
    word: "Regel von 2 und 4",
    short: "Outs mal 2 für eine Karte, Outs mal 4 für zwei Karten ergibt grob die Trefferchance in Prozent.",
  },
  blocked: {
    word: "Blockiert",
    short: "Kombinationen, die es wegen deiner eigenen Karten nicht geben kann.",
  },
  combos: {
    word: "Mögliche Kombos",
    short: "Wie viele Kartenpaare der Gegner halten kann.",
  },
  chip_ev: {
    word: "Chip-EV",
    short: "Gerechnet in Chips, nicht in Preisgeld.",
    long: "Kurz vor dem Geld oder am Finaltisch spielst du besser etwas enger, weil Ausscheiden dort Preisgeld kostet.",
  },
  model_strategy: {
    word: "Modell-Strategie",
    short: "Was Ringsides Preflop-Modell mit dieser Hand hier macht.",
  },
  knockout: {
    word: "Kopfgeld",
    short: "Im Bounty-Turnier bekommst du für jeden Rauswurf Geld. Umgerechnet in Big Blinds zählt es wie Chips im Pot.",
  },
  prize_ev: {
    word: "Preisgeld-Erwartung",
    short: "So viel Preisgeld bringt dir dieser Weg im Schnitt, nach ICM gerechnet.",
  },
  m_ratio: {
    word: "M-Zahl",
    short: "Wie viele Runden Blinds und Antes du noch zahlen kannst, ohne einen Pot zu gewinnen.",
  },
  zone: {
    word: "Zone",
    short: "Harringtons Zonen: Je kleiner die M-Zahl, desto weniger Spielraum bleibt.",
  },
  sample: {
    word: "Stichprobe",
    short: "Wenige Hände sagen wenig: Der wahre Wert kann weit vom angezeigten liegen.",
  },

  // Starting-hand charts.
  stack: {
    word: "Stack",
    short: "Wie viele Chips du vor der Hand hast, gemessen in Big Blinds. Mit 10 BB und weniger gilt fast nur noch all-in oder Fold.",
  },
  shove: {
    word: "Shove",
    short: "Sofort all-in gehen. Mit kurzem Stack die beste Waffe, weil der Gegner nur noch callen oder folden kann.",
  },
  ante: {
    word: "Ante",
    short: "Ein kleiner Pflichteinsatz, den jeder Spieler vor der Hand zahlt. Er macht den Pot größer, also lohnt sich früheres All-in.",
  },
  bb_ante: {
    word: "BB-Ante",
    short: "Der Big Blind zahlt die Ante für den ganzen Tisch. So spielen die meisten Turniere auf PokerStars.",
  },
  hand_combos: {
    word: "Kombos",
    short: "So viele Kartenkombinationen gibt es von dieser Hand: Paare 6, suited 4, offsuit 12 – offsuit-Hände bekommst du also dreimal so oft.",
  },
  reach: {
    word: "Kommt vor",
    short: "Wie oft diese Situation an diesem Platz überhaupt vorkommt, von allen Händen aus gezählt.",
  },
  live: {
    word: "Live gerechnet",
    short: "Für diese Stacktiefe gibt es keine vorgerechnete Tabelle. Ringside rechnet sie auf deinem Rechner; das Ergebnis wird gespeichert.",
  },
  precomputed: {
    word: "Vorgerechnet",
    short: "Diese Tabelle ist fertig gerechnet und wird sofort geladen.",
  },

  // Where a rating comes from.
  equilibrium: {
    word: "Gleichgewicht",
    short: "Die Push/Fold-Strategie, die kein Spieler mehr verbessern kann, wenn nur All-in oder Fold erlaubt ist.",
    long: "Gerechnet in Chips, ohne Preisgeld-Druck (ICM), aus Ringsides Tabellen.",
  },
  model: {
    word: "Modell",
    short: "Ringsides Preflop-Modell: Es rechnet die Aktionen vor dem Flop genau und schätzt den Rest der Hand mit einer Formel.",
    long: "Eine gute Richtschnur, aber kein GTO des echten Spiels.",
  },
  solved: {
    word: "Solver",
    short: "Eine echte Berechnung der Hand ab dem Flop, heads-up, mit Ranges aus dem Preflop-Modell und Standard-Einsatzgrößen.",
  },
  estimate: {
    word: "Einschätzung",
    short: "Für diesen Spot gibt es weder Solver noch Modell, etwa weil drei Spieler den Flop sehen. Ringside schätzt – gut als Richtung, nicht exakt.",
    long: "Geschätzt wird: Wie oft gewinnt deine Hand gegen die wahrscheinlichen Hände der Gegner, was kostet der Call (Pot Odds), wie oft folden sie auf einen Bet?",
  },
  forced: {
    word: "Keine Entscheidung",
    short: "Hier gab es nichts zu entscheiden: Alle anderen waren schon all-in, oder der Call der letzten Chips war mit jeder Hand richtig. Zählt nicht in deine Bewertung.",
  },
} satisfies Record<string, Word>;

export type WordId = keyof typeof WORDS;

/** The explanation of a word: its sentence and, where there is one, the longer part. */
export function explain(id: WordId): string {
  const w: Word = WORDS[id];
  return w.long ? `${w.short} ${w.long}` : w.short;
}
