/**
 * Arrow keys move between the entries of a menu (`[data-nav]`), Home and End jump to the ends.
 * Tab still leaves the menu as usual.
 */
export function arrowNav(node: HTMLElement) {
  function key(e: KeyboardEvent) {
    const next = e.key === "ArrowDown" || e.key === "ArrowRight";
    const prev = e.key === "ArrowUp" || e.key === "ArrowLeft";
    if (!next && !prev && e.key !== "Home" && e.key !== "End") return;
    const items = [...node.querySelectorAll<HTMLElement>("[data-nav]")].filter((el) => el.offsetParent !== null && !el.closest("[inert]"));
    if (!items.length) return;
    const at = items.indexOf(document.activeElement as HTMLElement);
    if (at < 0 && (next || prev)) return;
    e.preventDefault();
    const to = e.key === "Home" ? 0 : e.key === "End" ? items.length - 1 : (at + (next ? 1 : -1) + items.length) % items.length;
    items[to].focus();
  }
  node.addEventListener("keydown", key);
  return { destroy: () => node.removeEventListener("keydown", key) };
}
