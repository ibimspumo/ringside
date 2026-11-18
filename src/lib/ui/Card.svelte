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
    disabled = false,
    onclick,
    children,
    ...rest
  }: {
    /** `xs` for small tiles in a dense grid, e.g. one flop of three cards. */
    pad?: "xs" | "sm" | "md" | "lg";
    tone?: "default" | "raised" | "sunken";
    selected?: boolean;
    /** A clickable card that cannot be chosen right now; say why in a title. */
    disabled?: boolean;
    onclick?: (e: MouseEvent) => void;
    children: Snippet;
  } & HTMLAttributes<HTMLElement> = $props();
</script>

{#if onclick}
  <button type="button" class="ui-card {pad} {tone} action" class:selected aria-pressed={selected} {disabled} {onclick} {...rest}>
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
  .xs {
    padding: var(--sp-2);
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
  /* A button centres its content vertically; a card reads from the top like the plain one. */
  .action {
    display: flex;
    flex-direction: column;
    justify-content: flex-start;
    align-items: stretch;
    width: 100%;
    cursor: pointer;
    transition:
      border-color var(--dur-fast) var(--ease-standard),
      background var(--dur-fast) var(--ease-standard);
  }
  .action:disabled {
    cursor: default;
    opacity: 0.45;
  }
  .action:is(:hover, [data-force="hover"]):not(:disabled) {
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
