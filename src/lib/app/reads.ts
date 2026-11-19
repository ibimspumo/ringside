// How far a single value can be trusted, and what a clear value means for your play. The
// player type itself comes with the data from the backend (crates/ringside-learn/src/hud.rs).
import type { PlayerType, StatId, StatValue } from "../types";

/** The type in a word a casual player reads at once; the pro name stays in the hover and the detail. */
export const TYPE_PLAIN: Record<PlayerType, string> = { nit: "Vorsichtig", tag: "Solide", lag: "Wild", station: "Zahlt viel" };

/** Below this many chances a value is still noise; it stays visible but recedes. */
export const SMALL_SAMPLE = 20;

/** How a single value is backed: "aus 8 Händen, noch unsicher", "aus 31 Gelegenheiten"; `short` leaves the warning to the mark. */
export function sampleNote(s: StatValue | undefined, short = false): string {
  if (!s || !s.opp) return "noch keine Gelegenheit";
  const perHand = s.id === "vpip" || s.id === "pfr";
  const what = perHand ? (s.opp === 1 ? "Hand" : "Händen") : s.opp === 1 ? "Gelegenheit" : "Gelegenheiten";
  return `aus\u00a0${s.opp.toLocaleString("de-DE")}\u00a0${what}${!short && s.opp < SMALL_SAMPLE ? ", noch unsicher" : ""}`;
}

/** What a clearly low or high value means for your play: one line each, shown only off the table. */
export const HINT: Partial<Record<StatId, { low: string; high: string }>> = {
  vpip: { low: "Geht er mit, hat er meist etwas Starkes.", high: "Mit guten Händen auf Wert setzen, wenig bluffen." },
  pfr: { low: "Erhöht er doch, ist die Hand meist stark.", high: "Seine Raises sind oft schwach: mit starken Händen 3-betten." },
  three_bet: { low: "Seine 3-Bets sind meist sehr stark: ohne Top-Hand folden.", high: "Gegen seine 3-Bets mit guten Händen weiterspielen." },
  fold_to_three_bet: { low: "Gegen ihn nur mit starken Händen 3-betten, selten bluffen: er geht oft mit.", high: "Gegen seine Raises öfter 3-betten, er gibt meist auf." },
  steal: { low: "Klaut er doch, ist die Hand meist stark.", high: "Gegen seine Steals aus den Blinds öfter erhöhen." },
  fold_to_steal: { low: "Seine Blinds nur mit guten Händen angreifen, er wehrt sich oft.", high: "Seine Blinds öfter stehlen." },
  cbet: { low: "Setzt er am Flop nach, glaub ihm eher.", high: "Gegen seine C-Bets öfter callen oder erhöhen." },
  fold_to_cbet: { low: "Am Flop nur mit getroffener Hand oder starkem Draw nachsetzen.", high: "Nach deinem Raise am Flop fast immer nachsetzen." },
  wtsd: { low: "Er gibt vor dem Showdown oft auf: Druck lohnt sich.", high: "Er geht oft bis zum Showdown: kaum bluffen." },
  wsd: { low: "Er zeigt oft schwache Hände: dünner auf Wert setzen.", high: "Kommt er zum Showdown, hat er meist etwas." },
  af: { low: "Setzt er doch, hat er meist etwas.", high: "Er setzt viel, auch ohne Hand: öfter mitgehen." },
  afq: { low: "Passiv: seine Bets ernst nehmen.", high: "Sehr aggressiv: mit mittleren Händen öfter mitgehen." },
};
