<script lang="ts">
  import type { Snippet } from "svelte";

  /**
   * Column title of a table. With `onclick` it sorts: the arrow shows the active column and its
   * direction; without, it is the same title, just not clickable.
   */
  let {
    active = false,
    asc = false,
    title,
    onclick,
    children,
  }: { active?: boolean; asc?: boolean; title?: string; onclick?: () => void; children: Snippet } = $props();
</script>

{#if onclick}
  <button type="button" class="ui-sort" class:on={active} class:asc {title} {onclick}>{@render children()}</button>
{:else}
  <span class="ui-sort" {title}>{@render children()}</span>
{/if}

<style>
  .ui-sort {
    display: inline-flex;
    align-items: center;
    padding: 0;
    border: 0;
    border-radius: var(--r-xs);
    background: none;
    color: var(--text-3);
    font: inherit;
    font-size: var(--fs-xs);
    font-weight: var(--fw-semibold);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    white-space: nowrap;
    transition: color var(--dur-fast) var(--ease-standard);
  }
  button {
    cursor: pointer;
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
