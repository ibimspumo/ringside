// Four-colour deck shared by PlayingCard and MiniCard. Suits are paths: the Unicode glyphs turn
// into emoji on some Windows fonts.

export const SUITS: Record<string, { name: string; color: string; path: string }> = {
  s: {
    name: "Pik",
    color: "#1d2026",
    path: "M50 4C64 25 93 39 93 61c0 13-10 21-21 21-8 0-14-4-17-9 1 9 5 16 11 21H34c6-5 10-12 11-21-3 5-9 9-17 9C17 82 7 74 7 61 7 39 36 25 50 4z",
  },
  h: {
    name: "Herz",
    color: "#c8312a",
    path: "M50 92C20 68 5 51 5 32 5 17 16 7 29 7c9 0 16 5 21 13 5-8 12-13 21-13 13 0 24 10 24 25 0 19-15 36-45 60z",
  },
  d: { name: "Karo", color: "#2462c8", path: "M50 3l38 47-38 47-38-47z" },
  c: {
    name: "Kreuz",
    color: "#1f8446",
    path: "M32 30a18 18 0 1 0 36 0a18 18 0 1 0-36 0zM11 60a18 18 0 1 0 36 0a18 18 0 1 0-36 0zM53 60a18 18 0 1 0 36 0a18 18 0 1 0-36 0zM38 52a12 12 0 1 0 24 0a12 12 0 1 0-24 0zM46 58c-1 17-5 28-13 37h34c-8-9-12-20-13-37z",
  },
};

const GLYPH: Record<string, string> = { "♠": "s", "♥": "h", "♦": "d", "♣": "c" };

/** "Th" → { rank: "10", suit } for display; undefined when it is no card. */
export function cardFace(card: string): { rank: string; suit: (typeof SUITS)[string]; key: string } | undefined {
  const m = /^(10|[2-9TJQKA])([shdc♠♥♦♣])$/i.exec(card.trim());
  if (!m) return undefined;
  const s = GLYPH[m[2]] ?? m[2].toLowerCase();
  const r = m[1].toUpperCase();
  return { rank: r === "T" || r === "10" ? "10" : r, suit: SUITS[s], key: `${r === "10" ? "T" : r}${s}` };
}

/** Splits text into plain parts and cards: "Zwei Paare (Th Tc 9h)" → text, cards, text. */
export function splitCards(text: string): ({ text: string } | { card: string })[] {
  const out: ({ text: string } | { card: string })[] = [];
  // Cards may touch ("AsKs", "A♠K♠"); a lowercase letter or digit after one means it is a word.
  const re = /(?<=^|[^A-Za-z0-9]|[shdc♠♥♦♣])(10|[2-9TJQKA])([shdc♠♥♦♣])(?![a-z0-9])/g;
  let at = 0;
  for (const m of text.matchAll(re)) {
    if (m.index > at) out.push({ text: text.slice(at, m.index) });
    out.push({ card: m[0] });
    at = m.index + m[0].length;
  }
  if (at < text.length) out.push({ text: text.slice(at) });
  return out;
}
