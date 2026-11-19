import type { Page } from "./pages";

let open: ((page: Page) => void) | null = null;

/** Main registers how a page opens, so a component deep in a page can link to another one. */
export function handleGo(handler: (page: Page) => void): void {
  open = handler;
}

export function go(page: Page): void {
  open?.(page);
}
