// What the open page adds to the one PageHeader of the app: a line about the page and the
// sections it is divided into. Pages register it while they are shown.

export type PageTab = { id: string; label: string; count?: number; disabled?: boolean; hint?: string };

export type PageSections = {
  /** Replaces the default line under the title. */
  sub?: string;
  tabs?: PageTab[];
  value?: string;
  /** Names the tab row for screen readers. */
  label?: string;
  onchange?: (id: string) => void;
};

export const pageHead = $state<{ current: PageSections | null }>({ current: null });

/** Registers a page's line and sections for the PageHeader while the calling component is mounted. */
export function usePageHead(get: () => PageSections) {
  $effect(() => {
    pageHead.current = get();
  });
  // Cleared only when the page goes, not on every update: a null in between would rebuild the
  // tab row under the pointer.
  $effect(() => () => {
    pageHead.current = null;
  });
}

/** Width and side padding of the page on screen; the PageHeader lines up with it. */
export type PageFrame = { max: string; pad: string };

export const pageFrame = $state<{ current: PageFrame | null }>({ current: null });

/**
 * Action for the scrolling content area: follows the `.page` inside it (pages differ in width:
 * 1360 px, wide editors, full-bleed lists) and publishes its frame for the PageHeader.
 */
export function followPage(node: HTMLElement) {
  let queued = false;
  const sync = () => {
    queued = false;
    const page = node.querySelector<HTMLElement>(".page");
    if (!page) {
      pageFrame.current = null;
      return;
    }
    const cs = getComputedStyle(page);
    const next = { max: cs.maxWidth, pad: cs.paddingLeft };
    const now = pageFrame.current;
    if (!now || now.max !== next.max || now.pad !== next.pad) pageFrame.current = next;
  };
  const later = () => {
    if (queued) return;
    queued = true;
    requestAnimationFrame(sync);
  };
  const watch = new MutationObserver(later);
  watch.observe(node, { childList: true, subtree: true });
  sync();
  return {
    destroy() {
      watch.disconnect();
      pageFrame.current = null;
    },
  };
}
