<script lang="ts">
  import IconButton from "./IconButton.svelte";
  import type { Snippet } from "svelte";
  import { floating, type Anchor } from "./floating";

  /**
   * Rich card beside an anchor. Hover cards follow the pointer (anchor from `pointerAnchor`) and
   * ignore the mouse; a pinned card is a dialog that closes with Escape or its close button.
   */
  let {
    anchor,
    side = "right",
    pinned = false,
    label,
    width = 280,
    onclose,
    children,
  }: {
    anchor: Anchor;
    side?: "top" | "bottom" | "right";
    pinned?: boolean;
    /** Names a pinned card for screen readers. */
    label?: string;
    width?: number;
    onclose?: () => void;
    children: Snippet;
  } = $props();

  function key(e: KeyboardEvent) {
    if (e.key !== "Escape" || !pinned) return;
    e.stopPropagation();
    onclose?.();
  }
</script>

{#if pinned}
  <div class="ui-popover pinned" class:closable={!!onclose} role="dialog" aria-label={label} tabindex="-1" style:width="{width}px" onkeydown={key} use:floating={{ anchor, side }}>
    {#if onclose}
      <span class="close">
        <IconButton size="sm" label="Schließen (Esc)" onclick={() => onclose?.()}>
          <svg viewBox="0 0 24 24"><path d="M6 6l12 12M18 6 6 18" /></svg>
        </IconButton>
      </span>
    {/if}
    {@render children()}
  </div>
{:else}
  <div class="ui-popover" role="tooltip" style:width="{width}px" use:floating={{ anchor, side }}>
    {@render children()}
  </div>
{/if}

<style>
  .ui-popover {
    max-height: calc(100vh - 16px);
    overflow-y: auto;
    padding: 10px var(--sp-3);
    border-radius: var(--r);
    border: 1px solid var(--line-2);
    background: var(--surface);
    box-shadow: var(--shadow-3);
    font-size: var(--fs-md);
    pointer-events: none;
    animation: in var(--dur-fast) var(--ease-enter);
  }
  .close {
    position: absolute;
    top: var(--sp-2);
    right: var(--sp-2);
  }
  .closable {
    padding-right: 40px;
  }
  .pinned {
    padding: var(--sp-3) 14px;
    pointer-events: auto;
  }
  .pinned:focus {
    outline: none;
  }
  @keyframes in {
    from {
      opacity: 0;
    }
  }
</style>
