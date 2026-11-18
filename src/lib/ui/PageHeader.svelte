<script lang="ts">
  import type { Snippet } from "svelte";
  import { pageFrame, type PageFrame, type PageSections } from "./page.svelte";
  import SourceNote from "./SourceNote.svelte";
  import Tabs from "./Tabs.svelte";

  /**
   * The one page heading of the app: title (the only h1), a line about the page, page-wide
   * controls on the right, and the page's sections as a full-size tab row below. It sits in the
   * same place with the same height on every page; pages add their line and sections through
   * `usePageHead` instead of drawing their own heading.
   */
  let {
    title,
    sub,
    sections = null,
    demo = false,
    locked = false,
    frame,
    actions,
  }: {
    title: string;
    sub?: string;
    sections?: PageSections | null;
    /** Catalogue preview: the title is not a heading, so the page keeps its one h1. */
    demo?: boolean;
    /** The page is locked while PokerStars runs; the note stays whatever line the page shows. */
    locked?: boolean;
    /** Lines the heading up with the page below; by default the page on screen (`followPage`). */
    frame?: PageFrame | null;
    actions?: Snippet;
  } = $props();

  const box = $derived(frame === undefined ? pageFrame.current : frame);

  const line = $derived(sections?.sub ?? sub);
  let tab = $state("");
  $effect(() => {
    tab = sections?.value ?? "";
  });
</script>

<header
  class="ui-page-header"
  class:tabbed={!!sections?.tabs?.length}
  style:--frame-left={box ? `${box.left}px` : "var(--sp-6)"}
  style:--frame-right={box ? `${box.right}px` : "var(--sp-6)"}
>
  <div class="bar">
    <div class="title">
      {#if demo}<p class="h1">{title}</p>{:else}<h1 class="h1">{title}</h1>{/if}
      {#if locked}<SourceNote kind="locked" />{/if}
      {#if line}<p title={line}>{line}</p>{/if}
    </div>
    {#if actions}<div class="actions">{@render actions()}</div>{/if}
  </div>
  {#if sections?.tabs?.length}
    <div class="sections">
      <Tabs label={sections.label ?? `Bereiche von ${title}`} tabs={sections.tabs} bind:value={tab} onchange={(id) => sections?.onchange?.(id)} />
    </div>
  {/if}
</header>

<style>
  .ui-page-header {
    position: relative;
    z-index: var(--z-sticky);
    border-bottom: 1px solid var(--line);
    background: var(--bg);
  }
  /* Title and tabs share the left and right edge of the page content below. */
  .bar,
  .sections {
    padding-left: var(--frame-left);
    padding-right: var(--frame-right);
  }
  .bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: var(--sp-2) var(--sp-5);
    min-height: 60px;
    padding-block: var(--sp-2);
  }
  .title {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    min-width: 0;
    flex: 1 1 260px;
  }
  .h1 {
    margin: 0;
    color: var(--text);
    font-family: var(--font-num);
    font-size: var(--fs-2xl);
    font-weight: var(--fw-semibold);
    line-height: var(--lh-tight);
    white-space: nowrap;
  }
  p:not(.h1) {
    margin: 0;
    min-width: 0;
    overflow: hidden;
    color: var(--text-2);
    font-size: var(--fs-md);
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2);
  }
  .sections {
    margin-top: calc(-1 * var(--sp-1));
  }
  .sections :global(.ui-tabs) {
    border-bottom: 0;
    gap: var(--sp-6);
  }
</style>
