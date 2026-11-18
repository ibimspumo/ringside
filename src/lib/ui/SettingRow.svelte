<script lang="ts">
  import type { Snippet } from "svelte";
  import SourceNote from "./SourceNote.svelte";

  /**
   * One setting: name and one sentence on the left, its control on the right. `more` folds the
   * long explanation away under "Mehr dazu"; `children` runs below across the row (lists,
   * results, a picker). Rows stack inside a Panel with `flush`, divided by lines.
   */
  let {
    title,
    sub,
    locked = false,
    more,
    control,
    children,
  }: {
    title: string;
    sub?: string;
    /** Locked while PokerStars runs: says so next to the name. */
    locked?: boolean;
    more?: Snippet;
    control?: Snippet;
    children?: Snippet;
  } = $props();

  const id = `set-${Math.random().toString(36).slice(2, 9)}`;
  let open = $state(false);
</script>

<div class="ui-setting">
  <div class="text">
    <div class="name">
      <h3 class="title">{title}</h3>
      {#if locked}<SourceNote kind="locked" />{/if}
    </div>
    {#if sub}<p class="sub">{sub}</p>{/if}
    {#if more}
      <button type="button" class="more-toggle" aria-expanded={open} aria-controls={id} onclick={() => (open = !open)}>
        {open ? "Weniger" : "Mehr dazu"}
        <svg viewBox="0 0 12 12" width="10" height="10" aria-hidden="true"><path d="M3 4.5l3 3 3-3" /></svg>
      </button>
      <div class="more" class:open {id} inert={!open}>
        <div class="more-inner">{@render more()}</div>
      </div>
    {/if}
  </div>
  {#if control}<div class="control">{@render control()}</div>{/if}
  {#if children}<div class="detail">{@render children()}</div>{/if}
</div>

<style>
  .ui-setting {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    column-gap: var(--sp-8);
    padding: var(--sp-4) var(--sp-5);
  }
  /* Siblings, not neighbours: a dialog a row brings along may stand between two rows. */
  .ui-setting ~ :global(.ui-setting) {
    border-top: 1px solid var(--line);
  }
  .text {
    min-width: 0;
    max-width: 68ch;
  }
  .name {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-1) var(--sp-2);
    min-height: 28px;
  }
  .title {
    margin: 0;
    color: var(--text);
    font-size: var(--fs-base);
    font-weight: var(--fw-semibold);
    line-height: var(--lh-tight);
  }
  .sub {
    margin: 2px 0 0;
    color: var(--text-2);
    font-size: var(--fs-base);
    line-height: 1.5;
  }
  .more-toggle {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-1);
    margin-top: var(--sp-1);
    padding: 0;
    border: 0;
    background: none;
    color: var(--text-2);
    font: inherit;
    font-size: var(--fs-md);
    text-decoration: underline;
    text-decoration-color: var(--line-2);
    text-underline-offset: 3px;
    cursor: pointer;
    transition: color var(--dur-fast) var(--ease-standard);
  }
  .more-toggle:hover {
    color: var(--text);
  }
  .more-toggle:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: 2px;
    border-radius: var(--r-xs);
  }
  .more-toggle svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 1.5;
    stroke-linecap: round;
    stroke-linejoin: round;
    transition: transform var(--dur-fast) var(--ease-standard);
  }
  .more-toggle[aria-expanded="true"] svg {
    transform: rotate(180deg);
  }
  .more {
    display: grid;
    grid-template-rows: 0fr;
    opacity: 0;
    transition:
      grid-template-rows var(--dur-slow) var(--ease-standard),
      opacity var(--dur-slow) var(--ease-standard);
  }
  .more.open {
    grid-template-rows: 1fr;
    opacity: 1;
  }
  .more-inner {
    min-height: 0;
    overflow: hidden;
    color: var(--text-2);
    font-size: var(--fs-base);
    line-height: 1.55;
  }
  .more-inner :global(ul) {
    margin: 0 0 var(--sp-2);
    padding-left: var(--sp-5);
  }
  .more-inner :global(li + li) {
    margin-top: 2px;
  }
  /* A second fold inside "Mehr dazu" for what only the curious need. */
  .more-inner :global(details) {
    margin-top: var(--sp-2);
  }
  .more-inner :global(summary) {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-1);
    list-style: none;
    width: fit-content;
    color: var(--text-2);
    font-size: var(--fs-md);
    text-decoration: underline;
    text-decoration-color: var(--line-2);
    text-underline-offset: 3px;
    cursor: pointer;
  }
  .more-inner :global(summary::-webkit-details-marker) {
    display: none;
  }
  /* The same small chevron as "Mehr dazu". */
  .more-inner :global(summary::after) {
    content: "";
    width: 5px;
    height: 5px;
    margin: 0 2px 3px 3px;
    border-right: 1.5px solid currentColor;
    border-bottom: 1.5px solid currentColor;
    transform: rotate(45deg);
    transition: transform var(--dur-fast) var(--ease-standard);
  }
  .more-inner :global(details[open] > summary::after) {
    margin-bottom: -2px;
    transform: rotate(225deg);
  }
  .more-inner :global(summary:hover) {
    color: var(--text);
  }
  .more-inner :global(summary:focus-visible) {
    outline: 2px solid var(--focus);
    outline-offset: 2px;
    border-radius: var(--r-xs);
  }
  .more-inner :global(details[open] > summary) {
    margin-bottom: var(--sp-2);
  }
  .more-inner > :global(:first-child) {
    margin-top: var(--sp-2);
  }
  .more-inner :global(h4) {
    margin: var(--sp-2) 0 var(--sp-1);
    color: var(--text);
    font-size: var(--fs-md);
    font-weight: var(--fw-semibold);
  }
  .more-inner :global(p) {
    margin: 0 0 var(--sp-2);
  }
  .more-inner :global(p:last-child) {
    margin-bottom: 0;
  }
  /* At the name, not the middle: an opened "Mehr dazu" must not pull the control away from it. */
  .control {
    align-self: start;
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: var(--sp-2);
    min-height: 28px;
  }
  /* A quiet button keeps its padding inside: pulled out by it, its text ends where framed buttons end. */
  .control > :global(.ui-btn.quiet.sm:last-child) {
    margin-right: -10px;
  }
  .detail {
    grid-column: 1 / -1;
    min-width: 0;
    margin-top: var(--sp-3);
  }
  /* Content that is only there in some states (a result, an error) leaves no gap while it is away. */
  .detail:empty {
    display: none;
  }
  @container (max-width: 640px) {
    .ui-setting {
      grid-template-columns: minmax(0, 1fr);
    }
    .control {
      justify-content: flex-start;
      margin-top: var(--sp-3);
    }
  }
</style>
