/**
 * Ranges as 169 class weights for the 13×13 editor, with the same notation as the Rust parser
 * (crates/ringside-engine/src/range.rs), which stays the authority when solving.
 */

export const RANKS = "AKQJT98765432";
export const CLASSES = 169;

/** Like HandClass::index: row * 13 + col with the ace first, suited above the diagonal. */
export function classLabel(i: number): string {
  const r = Math.floor(i / 13);
  const c = i % 13;
  const hi = RANKS[Math.min(r, c)];
  const lo = RANKS[Math.max(r, c)];
  return r === c ? hi + lo : r < c ? `${hi}${lo}s` : `${hi}${lo}o`;
}

export function classCombos(i: number): number {
  const r = Math.floor(i / 13);
  const c = i % 13;
  return r === c ? 6 : r < c ? 4 : 12;
}

/** Grid index of a class given by ranks (0 = ace … 12 = deuce). */
function index(a: number, b: number, suited: boolean): number {
  const [hi, lo] = a <= b ? [a, b] : [b, a];
  return hi === lo || suited ? hi * 13 + lo : lo * 13 + hi;
}

/** Grid index of two cards like "Ah", "Kd". */
export function comboClass(a: string, b: string): number {
  return index(RANKS.indexOf(a[0].toUpperCase()), RANKS.indexOf(b[0].toUpperCase()), a[1] === b[1]);
}

type Spec = { hi: number; lo: number; suit: boolean | null };

function spec(s: string): Spec | null {
  const m = /^([AKQJT2-9])([AKQJT2-9])([so])?$/i.exec(s.trim());
  if (!m) return null;
  const a = RANKS.indexOf(m[1].toUpperCase());
  const b = RANKS.indexOf(m[2].toUpperCase());
  const suit = m[3] ? m[3].toLowerCase() === "s" : null;
  if (a === b && suit !== null) return null;
  return { hi: Math.min(a, b), lo: Math.max(a, b), suit };
}

function classesOf(s: Spec): number[] {
  if (s.hi === s.lo) return [index(s.hi, s.lo, false)];
  if (s.suit !== null) return [index(s.hi, s.lo, s.suit)];
  return [index(s.hi, s.lo, true), index(s.hi, s.lo, false)];
}

function expand(item: string): number[] | null {
  if (item.endsWith("+")) {
    const s = spec(item.slice(0, -1));
    if (!s) return null;
    // Pairs climb to aces; other hands raise the kicker up to one below the high card.
    if (s.hi === s.lo) return Array.from({ length: s.hi + 1 }, (_, r) => index(r, r, false));
    const out: number[] = [];
    for (let lo = s.hi + 1; lo <= s.lo; lo++) out.push(...classesOf({ ...s, lo }));
    return out;
  }
  if (item.includes("-")) {
    const [x, y] = item.split("-");
    const a = spec(x);
    const b = spec(y);
    if (!a || !b) return null;
    const out: number[] = [];
    if (a.hi === a.lo && b.hi === b.lo) {
      for (let r = Math.min(a.hi, b.hi); r <= Math.max(a.hi, b.hi); r++) out.push(index(r, r, false));
      return out;
    }
    if (a.hi !== b.hi || a.suit !== b.suit || a.hi === a.lo || b.hi === b.lo) return null;
    for (let lo = Math.min(a.lo, b.lo); lo <= Math.max(a.lo, b.lo); lo++) out.push(...classesOf({ ...a, lo }));
    return out;
  }
  const s = spec(item);
  return s ? classesOf(s) : null;
}

/**
 * Class weights of a range text; `null` when a part is not understood. Single combos
 * ("AhKd") count as their share of the class.
 */
export function parseRange(text: string): number[] | null {
  const w = new Array<number>(CLASSES).fill(0);
  const partial = new Array<number>(CLASSES).fill(0);
  for (const raw of text.split(",")) {
    const token = raw.trim();
    if (!token) continue;
    let item = token;
    let weight = 1;
    const colon = token.indexOf(":");
    if (colon >= 0) {
      item = token.slice(0, colon).trim();
      weight = Number(token.slice(colon + 1).trim());
      if (!Number.isFinite(weight) || weight < 0 || weight > 1) return null;
    }
    const lower = item.toLowerCase();
    if (lower === "random" || lower === "any" || lower === "100%") {
      w.fill(weight);
      continue;
    }
    const combo = /^([AKQJT2-9][cdhs])([AKQJT2-9][cdhs])$/i.exec(item);
    if (combo) {
      if (combo[1].toLowerCase() === combo[2].toLowerCase()) return null;
      const i = comboClass(combo[1], combo[2]);
      partial[i] += weight / classCombos(i);
      continue;
    }
    const classes = expand(item);
    if (!classes) return null;
    for (const i of classes) w[i] = weight;
  }
  return w.map((x, i) => Math.min(1, x + partial[i]));
}

function fmtWeight(w: number): string {
  return w >= 0.995 ? "" : `:${Math.round(w * 100) / 100}`;
}

/** Compact notation of class weights: pairs, then suited and offsuit hands by high card. */
export function formatRange(w: number[]): string {
  const parts: string[] = [];
  const runs = (cells: { rank: number; w: number }[], name: (r: number) => string, top: number) => {
    let i = 0;
    while (i < cells.length) {
      const x = cells[i].w;
      if (x <= 0) {
        i++;
        continue;
      }
      let j = i;
      while (j + 1 < cells.length && Math.abs(cells[j + 1].w - x) < 0.005) j++;
      const first = cells[i].rank;
      const last = cells[j].rank;
      const text = i === j ? name(first) : first === top ? `${name(last)}+` : `${name(first)}-${name(last)}`;
      parts.push(text + fmtWeight(x));
      i = j + 1;
    }
  };
  runs(
    Array.from({ length: 13 }, (_, r) => ({ rank: r, w: w[index(r, r, false)] })),
    (r) => RANKS[r] + RANKS[r],
    0,
  );
  for (const suited of [true, false]) {
    for (let hi = 0; hi < 12; hi++) {
      const cells = [];
      for (let lo = hi + 1; lo < 13; lo++) cells.push({ rank: lo, w: w[index(hi, lo, suited)] });
      runs(cells, (lo) => `${RANKS[hi]}${RANKS[lo]}${suited ? "s" : "o"}`, hi + 1);
    }
  }
  return parts.join(",");
}

/** Combos in the range, weights summed, before any board cards are removed. */
export function rangeCombos(w: number[]): number {
  return w.reduce((sum, x, i) => sum + x * classCombos(i), 0);
}
