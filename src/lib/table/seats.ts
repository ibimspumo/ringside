// Where everything sits at a poker table drawn by Ringside: the seats round an oval, you at
// the bottom in the middle and the others clockwise in the order they act (as PokerStars
// does with a preferred seat), and per seat the action badge, the chips and the dealer
// button on the line from the seat to the middle. Every table of the app uses this; the HUD
// at the real client follows the client's own window instead.
//
// All lengths are pixels of the stage. Element sizes are passed in, so the same distances
// hold for every seat at every stage size.

export interface Point {
  x: number;
  y: number;
}

export interface Box {
  w: number;
  h: number;
}

export interface SeatPlace {
  /** Middle of the seat block (cards and name plate). */
  seat: Point;
  /** Unit vector from the seat to the middle of the table. */
  dir: Point;
  badge: Point;
  bet: Point;
  dealer: Point;
  box: Box;
}

export interface TableLayout {
  w: number;
  h: number;
  /** Middle of the felt: board and pot. */
  center: Point;
  /** The felt as an ellipse. */
  felt: { cx: number; cy: number; rx: number; ry: number };
  /** By place round the table: 0 is you at the bottom, then clockwise. */
  places: SeatPlace[];
}

/** How far a badge on the edge reaches over the seat block. */
const EDGE = 3;

export interface Sizes {
  /** Seat block of the other players. */
  seat: Box;
  /** Your seat block (usually larger: your cards face up). */
  me?: Box;
  /** Seat block per place where they differ (e.g. empty seats are smaller). */
  boxOf?: (place: number) => Box;
  /** Largest action badge. */
  badge: Box;
  /** Largest chip stack with its amount. */
  bet: Box;
  /** Diameter of the dealer button. */
  dealer?: number;
  /** Space between seat, badge and chips. */
  gap?: number;
  /** Space kept free round the stage. */
  margin?: number;
  /**
   * The badges of the others sit on the edge of their seat block, a few pixels over the name
   * plate's border, instead of beside it (yours stays beside: your cards are on that edge).
   */
  onEdge?: boolean;
}

/** Place round the table of `seat` when `me` sits at the bottom; both 0 … n−1 in action order. */
export function placeOf(seat: number, me: number, n: number): number {
  return (((seat - me) % n) + n) % n;
}

/** How far a box reaches from its middle in direction `u`. */
export function reach(box: Box, u: Point): number {
  const x = Math.abs(u.x) < 1e-9 ? Infinity : box.w / 2 / Math.abs(u.x);
  const y = Math.abs(u.y) < 1e-9 ? Infinity : box.h / 2 / Math.abs(u.y);
  return Math.min(x, y);
}

/**
 * How far box `b` must move from box `a` in direction `u` to keep `gap` between them: the
 * edge of the boxes' Minkowski sum, so they stay apart at every angle, not only on the line.
 */
export function apart(a: Box, b: Box, gap: number, u: Point): number {
  return reach({ w: a.w + b.w + 2 * gap, h: a.h + b.h + 2 * gap }, u);
}

/**
 * `n` points evenly spaced along an ellipse by arc length, from the bottom clockwise on
 * screen (y points down), so seats keep the same distance on long and short sides.
 */
function aroundEllipse(n: number, cx: number, cy: number, rx: number, ry: number): Point[] {
  const steps = 720;
  const at = (t: number) => ({ x: cx + rx * Math.cos(t), y: cy + ry * Math.sin(t) });
  const lengths = [0];
  let prev = at(Math.PI / 2);
  for (let i = 1; i <= steps; i++) {
    const p = at(Math.PI / 2 + (i / steps) * 2 * Math.PI);
    lengths.push(lengths[i - 1] + Math.hypot(p.x - prev.x, p.y - prev.y));
    prev = p;
  }
  const total = lengths[steps];
  const out: Point[] = [];
  let i = 0;
  for (let k = 0; k < n; k++) {
    const want = (k * total) / n;
    while (i < steps && lengths[i + 1] < want) i++;
    const f = lengths[i + 1] === lengths[i] ? 0 : (want - lengths[i]) / (lengths[i + 1] - lengths[i]);
    out.push(at(Math.PI / 2 + ((i + f) / steps) * 2 * Math.PI));
  }
  // Mirror pairs exactly, so the table is symmetric to its vertical axis.
  for (let k = 1; k < n - k; k++) {
    out[n - k] = { x: 2 * cx - out[k].x, y: out[k].y };
  }
  if (n % 2 === 0) out[n / 2].x = cx;
  out[0].x = cx;
  return out;
}

export function layout(n: number, w: number, h: number, sizes: Sizes): TableLayout {
  const seat = sizes.seat;
  const me = sizes.me ?? seat;
  const gap = sizes.gap ?? 6;
  const m = sizes.margin ?? 8;
  const dealer = sizes.dealer ?? 20;
  // Seats sit on an ellipse whose top and bottom blocks just fit the stage.
  const ry = Math.max(40, (h - 2 * m - seat.h / 2 - me.h / 2) / 2);
  const cy = m + seat.h / 2 + ry;
  const rx = Math.max(60, w / 2 - m - Math.max(seat.w, me.w) / 2);
  const cx = w / 2;
  const points = aroundEllipse(n, cx, cy, rx, ry);
  // The middle sits halfway between the top seat and your (taller) seat, so both lines to
  // it have the same room for badge and chips.
  const center = { x: cx, y: (m + seat.h + (h - m - me.h)) / 2 };
  const places = points.map((p, k): SeatPlace => {
    const box = sizes.boxOf?.(k) ?? (k === 0 ? me : seat);
    const dx = center.x - p.x;
    const dy = center.y - p.y;
    const len = Math.hypot(dx, dy) || 1;
    const u = { x: dx / len, y: dy / len };
    const d1 = apart(box, sizes.badge, sizes.onEdge && k > 0 ? -EDGE : gap, u);
    const badge = { x: p.x + u.x * d1, y: p.y + u.y * d1 };
    // Clear of the badge and of the seat block itself (a small badge does not cover it).
    const d2 = Math.max(d1 + apart(sizes.badge, sizes.bet, gap, u), apart(box, sizes.bet, gap, u));
    const bet = { x: p.x + u.x * d2, y: p.y + u.y * d2 };
    // The button sits beside the badge, clear of the seat block and of its own badge, on the
    // side where it needs the shorter push away from the seat.
    const button = { w: dealer, h: dealer };
    const seatRect = rect(p, { w: box.w + 2 * gap, h: box.h + 2 * gap });
    const badgeRect = rect(badge, { w: sizes.badge.w + 2 * gap, h: sizes.badge.h + 2 * gap });
    const beside = (sign: number) => {
      const side = { x: -u.y * sign, y: u.x * sign };
      const off = Math.max(apart(sizes.badge, button, gap, side), apart(sizes.bet, button, gap, side));
      const at = { x: badge.x + side.x * off, y: badge.y + side.y * off };
      let pushes = 0;
      while (pushes < 60 && (overlaps(rect(at, button), seatRect) || overlaps(rect(at, button), badgeRect))) {
        at.x += u.x * 2;
        at.y += u.y * 2;
        pushes++;
      }
      return { at, pushes };
    };
    const [a, b] = [beside(1), beside(-1)];
    const dealerAt = (b.pushes < a.pushes ? b : a).at;
    return { seat: p, dir: u, badge, bet, dealer: dealerAt, box };
  });
  // The seats sit on the edge of the felt, half on the rail, as at the client.
  return { w, h, center, felt: { cx, cy, rx, ry }, places };
}

/**
 * The least stage height where a seat at the top and you at the bottom keep their badges and
 * chips clear of the middle (`middle`: board and pot) on the vertical line.
 */
export function minHeight(sizes: Sizes, middle: Middle): number {
  const gap = sizes.gap ?? 6;
  const m = sizes.margin ?? 8;
  const line = gap + sizes.bet.h + 2 * gap;
  const top = (sizes.onEdge ? sizes.badge.h - 2 * EDGE : gap + sizes.badge.h) + line;
  const bottom = gap + sizes.badge.h + line;
  const high = Math.min(...middle.map((x) => x.dy - x.box.h / 2));
  const low = Math.max(...middle.map((x) => x.dy + x.box.h / 2));
  return Math.ceil(2 * m + sizes.seat.h + (sizes.me ?? sizes.seat).h + 2 * Math.max(top, bottom) + (low - high));
}

/** What sits in the middle (board, pot), each box with its offset below the centre. */
export type Middle = { box: Box; dy: number }[];

/** Rectangle of a box centred on a point, for overlap checks. */
export function rect(p: Point, b: Box) {
  return { left: p.x - b.w / 2, top: p.y - b.h / 2, right: p.x + b.w / 2, bottom: p.y + b.h / 2 };
}

function overlaps(a: ReturnType<typeof rect>, b: ReturnType<typeof rect>): boolean {
  return a.left < b.right && b.left < a.right && a.top < b.bottom && b.top < a.bottom;
}

type Rect = ReturnType<typeof rect>;
/** Overlap by more than a pixel (pieces placed at exactly `gap` apart only touch). */
const hits = (a: Rect, b: Rect) => a.left < b.right - 1 && b.left < a.right - 1 && a.top < b.bottom - 1 && b.top < a.bottom - 1;

/** Whether badges, chips and dealer button keep clear of each other, the seats and the middle. */
export function clear(geo: TableLayout, sizes: Sizes, middle: Middle, dealerAt: number | null = null): boolean {
  const gap = (sizes.gap ?? 6) / 2;
  const grow = (r: Rect): Rect => ({ left: r.left - gap, top: r.top - gap, right: r.right + gap, bottom: r.bottom + gap });
  // A badge on the edge of its own seat block may cover it; nothing else may touch.
  const all: { r: Rect; seat: number; badge: boolean }[] = [];
  geo.places.forEach((p, k) => {
    all.push({ r: grow(rect(p.seat, p.box)), seat: k, badge: false });
    all.push({ r: grow(rect(p.badge, sizes.badge)), seat: k, badge: true });
    all.push({ r: grow(rect(p.bet, sizes.bet)), seat: k, badge: false });
    if (k === dealerAt) all.push({ r: grow(rect(p.dealer, { w: sizes.dealer ?? 20, h: sizes.dealer ?? 20 })), seat: k, badge: false });
  });
  for (const x of middle) all.push({ r: grow(rect({ x: geo.center.x, y: geo.center.y + x.dy }, x.box)), seat: -1, badge: false });
  for (let i = 0; i < all.length; i++)
    for (let j = i + 1; j < all.length; j++) {
      const a = all[i];
      const b = all[j];
      if (sizes.onEdge && a.seat > 0 && a.seat === b.seat && (a.badge || b.badge) && j === i + 1) continue;
      if (hits(a.r, b.r)) return false;
    }
  return true;
}

/**
 * The layout for a stage `w` wide: if everything does not fit, it is laid out on the least
 * wider stage where it does and drawn scaled down by `scale`, so nothing ever overlaps.
 * `height(w)` gives the stage height for a width.
 */
export function fit(
  n: number,
  w: number,
  height: (w: number) => number,
  sizes: Sizes,
  middle: Middle,
  dealerAt: number | null = null,
): { geo: TableLayout; scale: number } {
  for (let vw = Math.max(w, 200); vw <= 2400; vw += 20) {
    const geo = layout(n, vw, height(vw), sizes);
    if (clear(geo, sizes, middle, dealerAt)) return { geo, scale: w / vw };
  }
  return { geo: layout(n, 2400, height(2400), sizes), scale: w / 2400 };
}

/**
 * The table every view draws: a fixed stage, scaled as a whole to the room it gets, so it keeps
 * the shape of a real table (the felt about twice as wide as high) at every size. Lengths are
 * pixels of the stage. Action, win and dealer button sit on the name plate, so only the chips
 * go on the line to the middle; `scripts/table-layout.test.mjs` checks that nothing overlaps at
 * 2 to 10 seats.
 */
export const STAGE: Box = { w: 760, h: 426 };
export const STAGE_SIZES: Sizes = {
  seat: { w: 112, h: 96 },
  me: { w: 118, h: 103 },
  badge: { w: 0, h: 0 },
  bet: { w: 76, h: 20 },
  gap: 4,
  margin: 4,
};
/** Pot, board and street name, stacked in the middle. */
export const STAGE_MIDDLE: Middle = [{ box: { w: 190, h: 94 }, dy: 0 }];
/** Card widths on the stage. */
export const STAGE_CARDS = { board: 34, hero: 33, shown: 28, hidden: 22 };
