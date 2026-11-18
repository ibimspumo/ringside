<script lang="ts">
  import type { Snippet } from "svelte";

  /**
   * The main action of a table row, filling its first cell: reachable by keyboard, the row's
   * click target stays the whole row. `current` marks the row whose detail is open.
   */
  let {
    current = false,
    onclick,
    onkeydown,
    children,
  }: { current?: boolean; onclick: (e: MouseEvent) => void; onkeydown?: (e: KeyboardEvent) => void; children: Snippet } = $props();
</script>

<button type="button" class="ui-cell-button" aria-current={current ? "true" : undefined} {onclick} {onkeydown}>{@render children()}</button>

<style>
  .ui-cell-button {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    width: 100%;
    min-width: 150px;
    max-width: 260px;
    height: 40px;
    padding: 0 var(--sp-3) 0 var(--sp-4);
    border: 0;
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: var(--fs-base);
    text-align: left;
    cursor: pointer;
  }
  .ui-cell-button:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: -2px;
  }
</style>
