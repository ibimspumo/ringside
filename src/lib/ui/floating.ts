export type Anchor = { left: number; top: number; right: number; bottom: number };
export type FloatOptions = { anchor: Anchor; side?: "top" | "right" | "bottom" | "left" };

const EDGE = 8;
const GAP = 6;

/** Viewport rectangle of an element as an anchor. */
export function rectOf(el: Element): Anchor {
  const r = el.getBoundingClientRect();
  return { left: r.left, top: r.top, right: r.right, bottom: r.bottom };
}

/**
 * Anchor beside the pointer that keeps the card clear of the cursor arrow, which hangs down and
 * right of the pointer; the card never covers what is being pointed at.
 */
export function pointerAnchor(e: PointerEvent | MouseEvent, target?: Element): Anchor {
  const r = target ? rectOf(target) : { left: e.clientX, right: e.clientX, top: e.clientY, bottom: e.clientY };
  const y = e.clientY - 12;
  return { left: Math.min(r.left, e.clientX - 10), right: Math.max(r.right, e.clientX + 18), top: y, bottom: y };
}

/**
 * Svelte action for hover cards and popovers: moves the node to <body> so no scrolling or
 * contained ancestor can clip it, places it above, below, right or left of the anchor in viewport
 * coordinates, flips to the other side when there is no room and clamps it 8 px inside the window.
 */
export function floating(node: HTMLElement, options: FloatOptions) {
  document.body.appendChild(node);
  node.classList.add("float");
  // Viewport coordinates need fixed positioning whatever class the card has.
  node.style.position = "fixed";
  node.style.zIndex = "var(--z-popover)";

  function place({ anchor: a, side = "top" }: FloatOptions) {
    const w = node.offsetWidth;
    const h = node.offsetHeight;
    const vw = document.documentElement.clientWidth;
    const vh = document.documentElement.clientHeight;
    let x: number;
    let y: number;
    if (side === "top") {
      x = (a.left + a.right) / 2 - w / 2;
      y = a.top - GAP - h;
      if (y < EDGE && vh - a.bottom > a.top) y = a.bottom + GAP;
    } else if (side === "bottom") {
      x = (a.left + a.right) / 2 - w / 2;
      y = a.bottom + GAP;
      if (y + h > vh - EDGE && a.top > vh - a.bottom) y = a.top - GAP - h;
    } else if (side === "left") {
      x = a.left - GAP - w;
      if (x < EDGE && vw - a.right > a.left) x = a.right + GAP;
      y = a.top;
    } else {
      x = a.right + GAP;
      if (x + w > vw - EDGE && a.left > vw - a.right) x = a.left - GAP - w;
      y = a.top;
    }
    node.style.left = `${Math.max(EDGE, Math.min(x, vw - EDGE - w))}px`;
    node.style.top = `${Math.max(EDGE, Math.min(y, vh - EDGE - h))}px`;
    node.style.visibility = "";
  }

  // The anchor moves with the page, the card does not; hide it until the next hover. Scrolling
  // inside the node itself (a long list) moves nothing.
  const hide = (e: Event) => {
    if (!(e.target instanceof Node && node.contains(e.target))) node.style.visibility = "hidden";
  };
  window.addEventListener("scroll", hide, { capture: true, passive: true });
  place(options);
  return {
    update: place,
    destroy() {
      window.removeEventListener("scroll", hide, { capture: true });
      node.remove();
    },
  };
}
