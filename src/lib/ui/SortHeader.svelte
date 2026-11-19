<script lang="ts">
  import type { Snippet } from "svelte";

  /**
   * Column title of a table. With `onclick` it sorts: the arrow shows the active column and its
   * direction; without, it is the same title, just not clickable.
   */
  let {
    active = false,
    asc = false,
    term = false,
    sub,
    title,
    onclick,
    children,
  }: {
    active?: boolean;
    asc?: boolean;
    /** Has an explanation on hover: dotted underline like a Term. */
    term?: boolean;
    /** Quieter second line under the plain title, e.g. the abbreviation pros know ("VPIP %"). */
    sub?: string;
    title?: string;
    onclick?: () => void;
    children: Snippet;
  } = $props();
</script>

{#snippet label()}
  {#if sub}<span class="two"><span class="main">{@render children()}</span><small>{sub}</small></span>{:else}<span class="main">{@render children()}</span>{/if}
{/snippet}

{#if onclick}
  <button type="button" class="ui-sort" class:on={active} class:asc class:term {title} {onclick}>{@render label()}</button>
{:else}
  <span class="ui-sort" class:term {title}>{@render label()}</span>
{/if}

<style>
  .ui-sort {
    display: inline-flex;
    align-items: center;
    padding: 0;
    border: 0;
    border-radius: var(--r-xs);
    background: none;
    color: var(--text-2);
    font: inherit;
    font-size: var(--fs-sm);
    font-weight: var(--fw-semibold);
    /* Titles break only where they say so ("Fold\n3-Bet"). */
    white-space: pre;
    text-align: inherit;
    line-height: var(--lh-tight);
    transition: color var(--dur-fast) var(--ease-standard);
  }
  button {
    cursor: pointer;
  }
  .two {
    display: inline-block;
  }
  small {
    display: block;
    margin-top: 2px;
    color: var(--text-3);
    font-size: var(--fs-xs);
    font-weight: var(--fw-medium);
  }
  .term .main {
    text-decoration: underline dotted color-mix(in srgb, currentColor 55%, transparent);
    text-decoration-thickness: 1px;
    text-underline-offset: 3px;
  }
  button:hover,
  .on {
    color: var(--text);
  }
  button:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: 2px;
  }
  .on::after {
    content: "";
    margin-left: 5px;
    border: 4px solid transparent;
    border-top-color: currentColor;
    border-bottom: 0;
  }
  .on.asc::after {
    border-top: 0;
    border-bottom: 4px solid currentColor;
  }
</style>
