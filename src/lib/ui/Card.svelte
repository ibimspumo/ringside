<script lang="ts">
  import type { Snippet } from "svelte";
  import type { HTMLAttributes } from "svelte/elements";

  /**
   * A surface for one thing (a table, a spot, a player). With `onclick` the whole card is one
   * button; `selected` marks the chosen one in a list of cards.
   */
  let {
    pad = "md",
    tone = "default",
    selected = false,
    onclick,
    children,
    ...rest
  }: {
    pad?: "sm" | "md" | "lg";
    tone?: "default" | "raised" | "sunken";
    selected?: boolean;
    onclick?: (e: MouseEvent) => void;
    children: Snippet;
  } & HTMLAttributes<HTMLElement> = $props();
</script>

{#if onclick}
  <button type="button" class="ui-card {pad} {tone} action" class:selected aria-pressed={selected} {onclick} {...rest}>
    {@render children()}
  </button>
{:else}
  <div class="ui-card {pad} {tone}" class:selected {...rest}>{@render children()}</div>
{/if}

<style>
  .ui-card {
    display: block;
    min-width: 0;
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    background: var(--surface);
    color: inherit;
    font: inherit;
    text-align: left;
  }
  .sm {
    padding: var(--sp-3);
  }
  .md {
    padding: var(--sp-4);
  }
  .lg {
    padding: var(--sp-6);
  }
  .raised {
    background: var(--surface-2);
    border-color: var(--line-2);
  }
  .sunken {
    background: var(--surface-sunken);
  }
  .action {
    width: 100%;
    cursor: pointer;
    transition:
      border-color var(--dur-fast) var(--ease-standard),
      background var(--dur-fast) var(--ease-standard);
  }
  .action:is(:hover, [data-force="hover"]) {
    border-color: var(--line-2);
    background: var(--surface-2);
  }
  .action:is(:focus-visible, [data-force="focus"]) {
    outline: 2px solid var(--focus);
    outline-offset: 2px;
  }
  .selected {
    border-color: var(--text);
    box-shadow: inset 0 0 0 1px var(--text);
  }
</style>
