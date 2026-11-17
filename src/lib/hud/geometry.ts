// Sizes are in "table pixels": px at a 960 px wide table scene, multiplied by `stageScale`.

/** Width / height of the client area the default layouts were measured in (ringside-core). */
export const REFERENCE_ASPECT = 953 / 657;

/**
 * The client draws the table scene at a fixed shape, scaled to fit the window, so HUD sizes
 * follow the scene rather than the window: a wide window does not make panels wider.
 */
export function stageScale(width: number, height: number): number {
  return Math.min(width, height * REFERENCE_ASPECT) / 960;
}

/** Avatar ring: diameter, and stroke in 1/100 of the diameter. Sits on the avatar's edge. */
export const RING = { size: 74, stroke: 10 };

/** Radius of the client's avatar picture, kept visible through the plate. */
export const AVATAR_RADIUS = 33;

/**
 * Graphic plate, sized to cover the client's name plate (195 × 60 at 953 px) and the bounty
 * badge under the avatar. `compact` is the stats-only strip used when not covering.
 */
export const PLATE = {
  width: 212,
  // As tall as the ring and ending in a half circle around it: a fixed unit like the original.
  height: RING.size,
  offsetY: 0,
  behindAvatar: RING.size / 2,
  compact: { width: 150, height: 34 },
};

export type Side = "left" | "right";

/**
 * Side of the avatar where the client draws its name plate: the outer side for seats along
 * the rails, and opposite sides for the two seats near the vertical centre line (hero at the
 * bottom has it on the right, the top seat on the left).
 */
export function plateSide(x: number, y: number, width: number, height: number): Side {
  const rx = x / width;
  if (Math.abs(rx - 0.5) < 0.1) return y / height > 0.5 ? "right" : "left";
  return rx < 0.5 ? "left" : "right";
}

export const other = (side: Side): Side => (side === "left" ? "right" : "left");

/**
 * Default anchor of a classic box: under the client's name plate, flush with its outer edge,
 * which keeps it off the bets (towards the table centre), the cards (above the plate) and the
 * neighbouring seats. The bottom-centre seat gets it centred under its plate; the top-centre
 * seat beside its avatar, since under its plate are the dealer button and the timer.
 */
export function classicAnchor(cx: number, cy: number, s: number, width: number, height: number) {
  const side = plateSide(cx, cy, width, height);
  const centred = Math.abs(cx / width - 0.5) < 0.1;
  const below = cy + (PLATE.height / 2 + PLATE.offsetY) * s;
  if (centred && cy / height < 0.5) {
    const gap = (RING.size / 2 + 3) * s;
    return side === "left"
      ? { x: cx + gap, y: cy - AVATAR_RADIUS * s, transform: "translate(0, 0)" }
      : { x: cx - gap, y: cy - AVATAR_RADIUS * s, transform: "translate(-100%, 0)" };
  }
  if (centred) {
    const plateCentre = cx + (side === "left" ? -1 : 1) * (PLATE.width / 2 - PLATE.behindAvatar) * s;
    return { x: plateCentre, y: below, transform: "translate(-50%, 0)" };
  }
  const outer = (PLATE.width - PLATE.behindAvatar) * s;
  return side === "left"
    ? { x: cx - outer, y: below, transform: "translate(0, 0)" }
    : { x: cx + outer, y: below, transform: "translate(-100%, 0)" };
}

/** Default top-left of the graphic plate for an avatar at (cx, cy). */
export function defaultPlate(cx: number, cy: number, s: number, side: Side, cover: boolean, widthFactor = 1) {
  if (cover) {
    const w = PLATE.width * widthFactor;
    const left = side === "left" ? cx - (w - PLATE.behindAvatar) * s : cx - PLATE.behindAvatar * s;
    return { x: left, y: cy - (PLATE.height / 2 - PLATE.offsetY) * s, width: w * s, height: PLATE.height * s };
  }
  // Not covering: a stats strip right under the client's name plate.
  const w = PLATE.compact.width * s;
  const centre = cx + (side === "left" ? -1 : 1) * (PLATE.width / 2 - PLATE.behindAvatar) * s;
  return { x: centre - w / 2, y: cy + 33 * s, width: w, height: PLATE.compact.height * s };
}
