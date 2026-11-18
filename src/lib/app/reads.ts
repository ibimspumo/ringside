// Reading an opponent from the numbers: type from VPIP and PFR, the move against it, and how
// sure that is. Rules of thumb and texts as in crates/ringside-learn/src/hud.rs, so the
// "Gegner lesen" drill and the player pages judge alike.
import { stat } from "../stats";
import type { StatId, StatValue } from "../types";

export type PlayerType = "nit" | "tag" | "lag" | "station";

export const TYPE: Record<PlayerType, { name: string; long: string; sketch: string; counter: string; why: string; band: [number, number] }> = {
  nit: {
    name: "Nit",
    long: "Nit (sehr eng)",
    sketch: "Spielt nur sehr starke Hände.",
    counter: "Öfter seine Blinds stehlen, gegen seine Raises folden.",
    why: "Er foldet fast alles. Wenn er doch erhöht, hat er eine starke Hand.",
    band: [0, 15],
  },
  tag: {
    name: "TAG",
    long: "TAG (eng und aggressiv)",
    sketch: "Spielt wenige Hände, die aber mit Raise.",
    counter: "Solide spielen, ohne große Anpassung.",
    why: "Er spielt eng und meist mit Raise. Große Anpassungen gegen ihn lohnen selten.",
    band: [15, 27],
  },
  lag: {
    name: "LAG",
    long: "LAG (locker und aggressiv)",
    sketch: "Spielt viele Hände und erhöht oft.",
    counter: "Öfter mitgehen und mit starken Händen selbst erhöhen.",
    why: "Er erhöht auch mit schwachen Händen. Starke Hände bekommen von ihm viel Geld.",
    band: [27, 100],
  },
  station: {
    name: "Calling Station",
    long: "Calling Station (locker und passiv)",
    sketch: "Spielt viele Hände, callt aber meist nur.",
    counter: "Viele Value-Bets, kaum bluffen.",
    why: "Er foldet selten. Bluffs gehen nicht durch, gute Hände werden bezahlt.",
    band: [27, 100],
  },
};

/** The type by rule of thumb, `null` in between. */
export function classify(vpip: number, pfr: number): PlayerType | null {
  const aggression = pfr / Math.max(vpip, 1e-9);
  if (vpip < 15) return "nit";
  if (vpip < 27 && aggression >= 0.7) return "tag";
  if (vpip >= 28 && aggression >= 0.6) return "lag";
  if (vpip >= 30 && aggression <= 0.35) return "station";
  return null;
}

/** Wilson 95 % interval of a share `p` (0–100) measured over `n` hands, in percent. */
export function interval(p: number, n: number): [number, number] {
  const q = p / 100;
  const z = 1.96;
  const mid = (q + (z * z) / (2 * n)) / (1 + (z * z) / n);
  const half = (z / (1 + (z * z) / n)) * Math.sqrt((q * (1 - q)) / n + (z * z) / (4 * n * n));
  return [100 * Math.max(0, mid - half), 100 * Math.min(1, mid + half)];
}

/** Whether the VPIP interval stays inside the type's range; `null` too close to call. */
function trusted(t: PlayerType, vpip: number, hands: number): boolean | null {
  const [lo, hi] = interval(vpip, hands);
  const [a, b] = TYPE[t].band;
  const edges = [a, b].filter((e) => e >= 1 && e < 99);
  if (edges.some((e) => Math.abs(lo - e) < 1 || Math.abs(hi - e) < 1)) return null;
  return lo >= a && hi <= b;
}

export type Sureness = "sure" | "maybe" | "unsure";

export interface Read {
  type: PlayerType | null;
  hands: number;
  sure: Sureness;
  /** "nach 412 Händen stabil", "nach 27 Händen noch unsicher". */
  note: string;
}

export function read(stats: StatValue[]): Read | null {
  const vpip = stat(stats, "vpip");
  const pfr = stat(stats, "pfr");
  const hands = stat(stats, "hands")?.value ?? vpip?.opp ?? 0;
  if (!vpip || vpip.value === null || !pfr || pfr.value === null || !hands) return null;
  const type = classify(vpip.value, pfr.value);
  const ok = type ? trusted(type, vpip.value, hands) : false;
  const sure: Sureness = ok === true ? "sure" : ok === null ? "maybe" : "unsure";
  const word = sure === "sure" ? "klares Muster" : sure === "maybe" ? "knapp" : "noch unsicher";
  return { type, hands, sure, note: `nach ${hands.toLocaleString("de-DE")} ${hands === 1 ? "Hand" : "Händen"} ${word}` };
}

/** Below this many chances a value is still noise; it stays visible but recedes. */
export const SMALL_SAMPLE = 20;

/** How a single value is backed: "aus 8 Händen, noch unsicher", "aus 31 Gelegenheiten"; `short` leaves the warning to the mark. */
export function sampleNote(s: StatValue | undefined, short = false): string {
  if (!s || !s.opp) return "noch keine Gelegenheit";
  const perHand = s.id === "vpip" || s.id === "pfr";
  const what = perHand ? (s.opp === 1 ? "Hand" : "Händen") : s.opp === 1 ? "Gelegenheit" : "Gelegenheiten";
  return `aus ${s.opp.toLocaleString("de-DE")} ${what}${!short && s.opp < SMALL_SAMPLE ? ", noch unsicher" : ""}`;
}


/** What a clearly low or high value means for your play: one line each, shown only off the table. */
export const HINT: Partial<Record<StatId, { low: string; high: string }>> = {
  vpip: { low: "Geht er mit, hat er meist etwas Starkes.", high: "Mit guten Händen auf Wert setzen, wenig bluffen." },
  pfr: { low: "Erhöht er doch, ist die Hand meist stark.", high: "Seine Raises sind oft schwach: mit starken Händen 3-betten." },
  three_bet: { low: "Seine 3-Bets sind meist sehr stark: ohne Top-Hand folden.", high: "Gegen seine 3-Bets mit guten Händen weiterspielen." },
  fold_to_three_bet: { low: "3-Bets gegen ihn nur mit Hand, er geht oft mit.", high: "Gegen seine Raises öfter 3-betten, er gibt meist auf." },
  steal: { low: "Klaut er doch, ist die Hand meist stark.", high: "Gegen seine Steals aus den Blinds öfter erhöhen." },
  fold_to_steal: { low: "Steals gegen ihn nur mit guter Hand.", high: "Seine Blinds öfter stehlen." },
  cbet: { low: "Setzt er am Flop nach, glaub ihm eher.", high: "Gegen seine C-Bets öfter callen oder erhöhen." },
  fold_to_cbet: { low: "C-Bets gegen ihn nur mit Hand oder starkem Draw.", high: "Nach deinem Raise am Flop fast immer nachsetzen." },
  wtsd: { low: "Er gibt vor dem Showdown oft auf: Druck lohnt sich.", high: "Er geht oft bis zum Showdown: kaum bluffen." },
  wsd: { low: "Er zeigt oft schwache Hände: dünner auf Wert setzen.", high: "Kommt er zum Showdown, hat er meist etwas." },
  af: { low: "Setzt er doch, hat er meist etwas.", high: "Er setzt viel, auch ohne Hand: öfter mitgehen." },
  afq: { low: "Passiv: seine Bets ernst nehmen.", high: "Sehr aggressiv: mit mittleren Händen öfter mitgehen." },
};
