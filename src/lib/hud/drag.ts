import type { Point } from "../types";

export interface DragOptions {
  /** Current position (px) of the thing being dragged, read when the press starts. */
  origin: () => Point;
  onMove: (p: Point, done: boolean) => void;
  /** A press without movement. */
  onTap?: () => void;
  onDouble?: () => void;
}

/** Pointer dragging with a small dead zone, so a click still counts as a tap. */
export function draggable(node: HTMLElement, options: DragOptions) {
  let opts = options;
  let start: { px: number; py: number; origin: Point; moved: boolean } | null = null;

  const move = (e: PointerEvent, done: boolean) => {
    if (!start) return;
    const dx = e.clientX - start.px;
    const dy = e.clientY - start.py;
    if (!start.moved && Math.hypot(dx, dy) < 4) return;
    start.moved = true;
    opts.onMove({ x: start.origin.x + dx, y: start.origin.y + dy }, done);
  };
  const down = (e: PointerEvent) => {
    if (e.button !== 0) return;
    node.setPointerCapture(e.pointerId);
    start = { px: e.clientX, py: e.clientY, origin: opts.origin(), moved: false };
  };
  const up = (e: PointerEvent) => {
    if (!start) return;
    const moved = start.moved;
    move(e, true);
    start = null;
    if (!moved) opts.onTap?.();
  };
  const pmove = (e: PointerEvent) => move(e, false);
  const dbl = () => opts.onDouble?.();

  node.addEventListener("pointerdown", down);
  node.addEventListener("pointermove", pmove);
  node.addEventListener("pointerup", up);
  node.addEventListener("dblclick", dbl);
  node.style.touchAction = "none";
  return {
    update(next: DragOptions) {
      opts = next;
    },
    destroy() {
      node.removeEventListener("pointerdown", down);
      node.removeEventListener("pointermove", pmove);
      node.removeEventListener("pointerup", up);
      node.removeEventListener("dblclick", dbl);
    },
  };
}
