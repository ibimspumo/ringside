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
