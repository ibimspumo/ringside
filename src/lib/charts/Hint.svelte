<script lang="ts">
  import type { Snippet } from "svelte";
  import { floating, type Anchor } from "../app/format";

  /** A word with its explanation on hover and keyboard focus; the card stays inside the window. */
  let { text, children }: { text: string; children: Snippet } = $props();
  const id = `hint-${Math.random().toString(36).slice(2, 9)}`;
  let anchor = $state<Anchor | null>(null);

  function open(e: Event) {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    anchor = { left: r.left, top: r.top, right: r.right, bottom: r.bottom };
  }
</script>

<button
  type="button"
  class="hint"
  aria-describedby={id}
  onmouseenter={open}
  onmouseleave={() => (anchor = null)}
  onfocus={open}
  onblur={() => (anchor = null)}>{@render children()}</button
>
{#if anchor}
  <span class="tip hint-tip" role="tooltip" {id} use:floating={{ anchor }}>{text}</span>
{/if}

<style>
  .hint {
    display: inline;
    padding: 0;
    border: 0;
    background: none;
    color: inherit;
    font: inherit;
    text-decoration: underline dotted color-mix(in srgb, currentColor 50%, transparent);
    text-underline-offset: 3px;
    cursor: help;
  }
  .hint:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: 2px;
    border-radius: 2px;
  }
  .hint-tip {
    display: block;
    width: max-content;
    max-width: 280px;
    color: var(--text);
    font-size: 12.5px;
    font-weight: 400;
    line-height: 1.45;
    text-align: left;
    white-space: normal;
  }
</style>
