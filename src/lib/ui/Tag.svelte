<script lang="ts">
  import type { Snippet } from "svelte";

  /**
   * A choice or an active filter. With `onclick` it toggles (`selected`), with `onremove` it
   * carries a remove cross; without both it only labels.
   */
  let {
    selected = false,
    onclick,
    onremove,
    removeLabel = "Entfernen",
    children,
  }: {
    selected?: boolean;
    onclick?: () => void;
    onremove?: () => void;
    removeLabel?: string;
    children: Snippet;
  } = $props();
</script>

{#if onclick}
  <button type="button" class="ui-tag toggle" class:selected aria-pressed={selected} {onclick}>{@render children()}</button>
{:else}
  <span class="ui-tag" class:removable={!!onremove}>
    {@render children()}
    {#if onremove}
      <button type="button" class="x" aria-label={removeLabel} title={removeLabel} onclick={onremove}>
        <svg viewBox="0 0 12 12" width="10" height="10" aria-hidden="true"><path d="M2 2l8 8M10 2l-8 8" /></svg>
      </button>
    {/if}
  </span>
{/if}

<style>
  .ui-tag {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 28px;
    padding: 0 var(--sp-3);
    border-radius: var(--r-pill);
    border: 1px solid var(--line-2);
    background: var(--surface-2);
    color: var(--text);
    font: inherit;
    font-size: var(--fs-md);
    white-space: nowrap;
  }
  .removable {
    padding-right: var(--sp-1);
  }
  .toggle {
    color: var(--text-2);
    cursor: pointer;
    transition:
      background var(--dur-fast) var(--ease-standard),
      color var(--dur-fast) var(--ease-standard),
      border-color var(--dur-fast) var(--ease-standard);
  }
  .toggle:hover {
    color: var(--text);
    border-color: color-mix(in srgb, var(--text) 30%, transparent);
  }
  .toggle.selected {
    background: var(--text);
    border-color: var(--text);
    color: var(--bg);
    font-weight: var(--fw-medium);
  }
  .toggle:focus-visible,
  .x:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: 2px;
  }
  .x {
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: transparent;
    color: var(--text-2);
    cursor: pointer;
  }
  .x:hover {
    background: var(--surface-3);
    color: var(--text);
  }
  .x svg {
    stroke: currentColor;
    stroke-width: 1.6;
    stroke-linecap: round;
  }
</style>
