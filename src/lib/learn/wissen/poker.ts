/** Cards are two-letter strings like "Ah", "Td", "7c". */
export const RANKS = "23456789TJQKA";
export const SUITS = "shdc";

export const rankOf = (card: string): number => RANKS.indexOf(card[0]);
export const suitOf = (card: string): string => card[1];

export const DECK: string[] = [...RANKS].reverse().flatMap((r) => [...SUITS].map((s) => r + s));

export function deckWithout(known: string[]): string[] {
  const out = new Set(known);
  return DECK.filter((c) => !out.has(c));
}

export const CATEGORY = [
  "High Card",
  "Paar",
  "Zwei Paare",
  "Drilling",
  "Straße",
  "Flush",
  "Full House",
  "Vierling",
  "Straight Flush",
] as const;

/** Highest straight in a rank bitmask, or -1. The wheel A-2-3-4-5 counts with 5 high (rank 3). */
function straightHigh(mask: number): number {
  for (let hi = 12; hi >= 4; hi--) {
    if (((mask >> (hi - 4)) & 0b11111) === 0b11111) return hi;
  }
  return (mask & 0b1000000001111) === 0b1000000001111 ? 3 : -1;
}

/**
 * Exact strength of the best five out of five to seven cards. Higher is better; equal means split.
 * Encoded as category * 13^5 + kickers in base 13.
 */
export function score(cards: string[]): number {
  const counts = new Array(13).fill(0);
  const bySuit: Record<string, number> = { s: 0, h: 0, d: 0, c: 0 };
  const suitCount: Record<string, number> = { s: 0, h: 0, d: 0, c: 0 };
  let mask = 0;
  for (const c of cards) {
    const r = rankOf(c);
    const s = suitOf(c);
    counts[r]++;
    mask |= 1 << r;
    bySuit[s] |= 1 << r;
    suitCount[s]++;
  }
  const pack = (cat: number, ks: number[]) => ks.slice(0, 5).reduce((acc, k) => acc * 13 + k, cat) * 13 ** (5 - Math.min(5, ks.length));

  const flushSuit = SUITS.split("").find((s) => suitCount[s] >= 5);
  if (flushSuit) {
    const sf = straightHigh(bySuit[flushSuit]);
    if (sf >= 0) return pack(8, [sf]);
  }
  const groups: [number, number][] = [];
  for (let r = 12; r >= 0; r--) if (counts[r]) groups.push([counts[r], r]);
  groups.sort((a, b) => b[0] - a[0] || b[1] - a[1]);
  const ranksDesc = (exclude: number[]) => {
    const out: number[] = [];
    for (let r = 12; r >= 0; r--) if (counts[r] && !exclude.includes(r)) out.push(r);
    return out;
  };

  if (groups[0][0] === 4) return pack(7, [groups[0][1], ...ranksDesc([groups[0][1]])]);
  if (groups[0][0] === 3 && groups[1]?.[0] >= 2) return pack(6, [groups[0][1], groups[1][1]]);
  if (flushSuit) {
    const fr: number[] = [];
    for (let r = 12; r >= 0; r--) if (bySuit[flushSuit] & (1 << r)) fr.push(r);
    return pack(5, fr);
  }
  const st = straightHigh(mask);
  if (st >= 0) return pack(4, [st]);
  if (groups[0][0] === 3) return pack(3, [groups[0][1], ...ranksDesc([groups[0][1]])]);
  if (groups[0][0] === 2 && groups[1]?.[0] === 2) {
    const [a, b] = [groups[0][1], groups[1][1]];
    return pack(2, [a, b, ...ranksDesc([a, b])]);
  }
  if (groups[0][0] === 2) return pack(1, [groups[0][1], ...ranksDesc([groups[0][1]])]);
  return pack(0, ranksDesc([]));
}

export const categoryOf = (cards: string[]): number => Math.floor(score(cards) / 13 ** 5);

function* choose<T>(items: T[], k: number, start = 0, acc: T[] = []): Generator<T[]> {
  if (acc.length === k) {
    yield acc;
    return;
  }
  for (let i = start; i <= items.length - (k - acc.length); i++) yield* choose(items, k, i + 1, [...acc, items[i]]);
}

/** The five cards that make the best hand out of five to seven, strongest first by rank. */
export function bestFive(cards: string[]): string[] {
  let best: string[] = cards.slice(0, 5);
  let top = -1;
  for (const five of choose(cards, Math.min(5, cards.length))) {
    const s = score(five);
    if (s > top) [top, best] = [s, five];
  }
  return [...best].sort((a, b) => rankOf(b) - rankOf(a));
}

/** Exact all-in equity of two hands by enumerating every runout (board of three or four cards). */
export function equity(hero: string[], villain: string[], board: string[]): { win: number; tie: number; lose: number; runouts: number } {
  const rest = deckWithout([...hero, ...villain, ...board]);
  let win = 0;
  let tie = 0;
  let total = 0;
  for (const extra of choose(rest, 5 - board.length)) {
    const b = [...board, ...extra];
    const h = score([...hero, ...b]);
    const v = score([...villain, ...b]);
    if (h > v) win++;
    else if (h === v) tie++;
    total++;
  }
  return { win: win / total, tie: tie / total, lose: (total - win - tie) / total, runouts: total };
}

/** Cards that lift the hand to at least `target` (a CATEGORY index) on the next street. */
export function outsTo(hole: string[], board: string[], target: number): string[] {
  return deckWithout([...hole, ...board]).filter((c) => categoryOf([...hole, ...board, c]) >= target);
}

const SINGLE = ["Zwei", "Drei", "Vier", "Fünf", "Sechs", "Sieben", "Acht", "Neun", "Zehn", "Bube", "Dame", "König", "Ass"];
const PLURAL = ["Zweien", "Dreien", "Vieren", "Fünfen", "Sechsen", "Siebenen", "Achten", "Neunen", "Zehnen", "Buben", "Damen", "Könige", "Asse"];

/** The best hand in words: "Paar Könige", "Zwei Paare, Könige und Achten", "Ass-hoch"; otherwise its category. */
export function madeHand(cards: string[]): string {
  const cat = categoryOf(cards);
  const five = bestFive(cards);
  const count = new Map<number, number>();
  for (const c of five) count.set(rankOf(c), (count.get(rankOf(c)) ?? 0) + 1);
  const ranks = (n: number) => [...count].filter(([, k]) => k === n).map(([r]) => r).sort((a, b) => b - a);
  switch (cat) {
    case 0:
      return `${SINGLE[rankOf(five[0])]}-hoch`;
    case 1:
      return `Paar ${PLURAL[ranks(2)[0]]}`;
    case 2:
      return `Zwei Paare, ${PLURAL[ranks(2)[0]]} und ${PLURAL[ranks(2)[1]]}`;
    case 3:
      return `Drilling ${PLURAL[ranks(3)[0]]}`;
    case 6:
      return `Full House, ${PLURAL[ranks(3)[0]]} und ${PLURAL[ranks(2)[0]]}`;
    case 7:
      return `Vierling ${PLURAL[ranks(4)[0]]}`;
    default:
      return CATEGORY[cat];
  }
}

/* Hand classes and combos ------------------------------------------------------------------------ */

/** "AA", "AKs", "AKo" → every concrete two-card combo of that class. */
export function combosOf(hand: string): [string, string][] {
  const [a, b, kind] = [hand[0], hand[1], hand[2]];
  const out: [string, string][] = [];
  const suits = SUITS.split("");
  if (a === b) {
    for (let i = 0; i < 4; i++) for (let j = i + 1; j < 4; j++) out.push([a + suits[i], b + suits[j]]);
  } else {
    for (const s1 of suits)
      for (const s2 of suits) {
        if ((kind === "s") !== (s1 === s2)) continue;
        out.push([a + s1, b + s2]);
      }
  }
  return out;
}

/** The 169 classes in matrix order: row i, column j; suited above the diagonal. */
export function matrixHand(i: number, j: number): string {
  const r = [...RANKS].reverse();
  if (i === j) return r[i] + r[j];
  return i < j ? r[i] + r[j] + "s" : r[j] + r[i] + "o";
}

const R = (ch: string) => RANKS.indexOf(ch);
const at = (i: number) => RANKS[i];

/** Pairs from `lo` up to `hi`, e.g. pairs("2", "9"). */
export function pairs(lo: string, hi: string): string[] {
  const out: string[] = [];
  for (let i = R(lo); i <= R(hi); i++) out.push(at(i) + at(i));
  return out;
}

/** High card fixed, kicker from `lo` to `hi`: run("A", "2", "J", "s") = A2s … AJs. */
export function run(high: string, lo: string, hi: string, kind: "s" | "o"): string[] {
  const out: string[] = [];
  for (let i = R(lo); i <= R(hi); i++) out.push(high + at(i) + kind);
  return out;
}

export function rangeCombos(range: string[], dead: string[] = []): [string, string][] {
  const d = new Set(dead);
  return range.flatMap(combosOf).filter(([x, y]) => !d.has(x) && !d.has(y));
}

/** How a combo connects with an unpaired flop: strong (two pair+), top pair / overpair, or less. */
export function flopHit(hole: string[], board: string[]): "strong" | "top" | "weak" {
  const cat = categoryOf([...hole, ...board]);
  if (cat >= 2) return "strong";
  if (cat === 1) {
    const top = Math.max(...board.map(rankOf));
    const [a, b] = hole.map(rankOf);
    if (a === b && a > top) return "top";
    if (a === top || b === top) return "top";
  }
  return "weak";
}
