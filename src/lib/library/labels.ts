import type { LibraryPower, LibraryScenario, Shared, ShareState } from "./types";

/** Opener first, as in the scenario id ("srp-btn-bb-40", "3bp-co-btn-75"). */
function seats(s: LibraryScenario): [string, string] {
  const [, a, b] = s.id.split("-");
  return [(a ?? s.ip).toUpperCase(), (b ?? s.oop).toUpperCase()];
}

const NAMES: Record<string, { name: string; at: string; against: string }> = {
  BTN: { name: "Button", at: "am Button", against: "den Button" },
  CO: { name: "Cutoff", at: "im Cutoff", against: "den Cutoff" },
  UTG: { name: "frühe Position", at: "in früher Position", against: "die frühe Position" },
  SB: { name: "Small Blind", at: "im Small Blind", against: "den Small Blind" },
  BB: { name: "Big Blind", at: "im Big Blind", against: "den Big Blind" },
};
const pos = (p: string) => NAMES[p] ?? { name: p, at: `auf ${p}`, against: p };

/** E.g. "Du am Button gegen den Big Blind". You are the player who opened. */
export function titleOf(s: LibraryScenario): string {
  const [opener, other] = seats(s);
  return `Du ${pos(opener).at} gegen ${pos(other).against}`;
}

/** E.g. "40 BB, normaler Raise" or "40 BB, der Big Blind hat 3-gebettet". */
export function situationOf(s: LibraryScenario): string {
  const [, other] = seats(s);
  const what = s.pot_type === "srp" ? "normaler Raise" : `${pos(other).name} hat 3-gebettet`;
  return `${s.stack_bb} BB, ${what}`;
}

/** Title and situation in one line. */
export function labelOf(s: LibraryScenario): string {
  return `${titleOf(s)}, ${situationOf(s)}`;
}

/** What happened before the flop, in words. */
export function storyOf(s: LibraryScenario): string {
  const [opener, other] = seats(s);
  const o = pos(opener).name;
  const x = pos(other).name;
  return s.pot_type === "srp"
    ? `Du erhöhst vom ${o}, der ${x} geht mit.`
    : `Du erhöhst vom ${o}, der ${x} erhöht noch einmal (3-Bet), du gehst mit.`;
}

/** Who acts first after the flop, in words. */
export function firstOf(s: LibraryScenario): string {
  const [opener] = seats(s);
  return s.oop === opener ? "Du handelst nach dem Flop zuerst." : `Der ${pos(s.oop).name} handelt nach dem Flop zuerst, du hast Position.`;
}

/** Stack depth in words. */
export function depthOf(stack: number): string {
  return stack <= 25 ? "kurzer Stack" : stack <= 50 ? "mittlerer Stack" : "tiefer Stack";
}

const SUITS: Record<string, string> = { s: "♠", h: "♥", d: "♦", c: "♣" };

/** "AsKhTd" → [["A","s"],["K","h"],["T","d"]]. */
export function cards(flop: string): [string, string][] {
  return [0, 2, 4].map((i) => [flop[i], flop[i + 1]] as [string, string]);
}

export function suit(s: string): string {
  return SUITS[s] ?? s;
}

/** "Ks7d2c" → "K♠ 7♦ 2♣". */
export function flopName(flop: string): string {
  return cards(flop)
    .map(([r, s]) => r + suit(s))
    .join(" ");
}

/** What the page says about a spot solved here: its sharing state, or only on this computer. */
export type Mark = ShareState | "only_here";

export function markOf(shared: Shared | null): Mark {
  return shared?.state ?? "only_here";
}

/** Short names, for the legend. */
export const MARK_NAME: Record<Mark, string> = {
  only_here: "Nur auf deinem PC",
  submitted: "Hochgeladen, noch nicht freigegeben",
  served: "Hochgeladen, für alle verfügbar",
  rejected: "Nicht angenommen oder zurückgezogen",
  failed: "Hochladen klappte nicht, neuer Versuch folgt",
};

const when = (at: number) =>
  new Date(at * 1000).toLocaleString("de-DE", { day: "numeric", month: "numeric", hour: "2-digit", minute: "2-digit" });

/** The cloud's note (English, technical) as a reason in words, with the note itself. */
function reasonOf(note: string | undefined): string {
  if (!note) return "Ringside Cloud hat keinen Grund genannt.";
  const why = /sample failed/i.test(note)
    ? "Eine Stichprobe von diesem Internetanschluss war beim Nachrechnen falsch, deshalb wurden alle Lösungen von hier zurückgezogen."
    : /operator/i.test(note)
      ? "Der Betreiber hat die Lösungen dieses Internetanschlusses zurückgezogen."
      : /^form:/i.test(note)
        ? "Die Lösung war nicht so aufgebaut, wie Ringside sie erwartet."
        : /exploitab/i.test(note)
          ? "Die Lösung war beim Nachrechnen nicht genau genug."
          : "Die Lösung wurde nicht angenommen.";
  return `${why} (Ringside Cloud: ${note})`;
}

/** The tooltip of a spot solved here: where it stands, and when. */
export function markText(shared: Shared | null, sharing: boolean): string {
  switch (markOf(shared)) {
    case "only_here":
      return sharing
        ? "Auf deinem PC gerechnet, noch nicht hochgeladen. Ringside holt das von selbst nach."
        : "Auf deinem PC gerechnet. Teilen ist aus, der Spot bleibt bei dir.";
    case "submitted":
      return `Auf deinem PC gerechnet und am ${when(shared!.at)} hochgeladen. Ringside Cloud prüft noch, ob er richtig aufgebaut ist, danach bekommen ihn alle.`;
    case "served":
      return `Auf deinem PC gerechnet und am ${when(shared!.at)} hochgeladen. Alle Ringside-Nutzer können ihn jetzt laden.`;
    case "rejected":
      return `Hochgeladen am ${when(shared!.at)}, aber nicht für alle freigegeben. ${reasonOf(shared!.note)} Bei dir kannst du ihn trotzdem ansehen.`;
    case "failed":
      return `Hochladen am ${when(shared!.at)} klappte nicht${shared!.note ? ` (${shared!.note})` : ""}. Ringside versucht es später noch einmal.`;
  }
}

/** Solver threads for a power choice; mirrors `threads_for` in src-tauri/src/library.rs. */
export function threadsFor(power: LibraryPower, cores: number): number {
  const n = power === "gentle" ? Math.ceil(cores / 4) : power === "normal" ? Math.ceil(cores / 2) : cores - 1;
  return Math.max(1, n);
}
