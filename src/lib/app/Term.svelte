<script lang="ts">
  import type { Snippet } from "svelte";
  import { floating, type Anchor } from "./format";

  /** A word with its explanation; the card opens on hover and on keyboard focus and stays inside the window. */
  let {
    tip,
    children,
    side = "top",
  }: { tip: string; children: Snippet; side?: "top" | "bottom" | "right" } = $props();

  const id = $props.id();
  let anchor = $state<Anchor | null>(null);

  function show(e: Event) {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    anchor = { left: r.left, right: r.right, top: r.top, bottom: r.bottom };
  }
  const hide = () => (anchor = null);
</script>

<button
  type="button"
  class="term"
  aria-describedby={id}
  onpointerenter={show}
  onpointerleave={hide}
  onfocus={show}
  onblur={hide}
  onkeydown={(e) => e.key === "Escape" && hide()}>{@render children()}</button
>{#if anchor}<span class="tip explain" {id} role="tooltip" use:floating={{ anchor, side }}>{tip}</span>{:else}<span
    class="sr"
    {id}>{tip}</span
  >{/if}

<style>
  .term {
    display: inline;
    padding: 0;
    border: 0;
    background: none;
    font: inherit;
    color: inherit;
    text-align: inherit;
    text-decoration: underline dotted color-mix(in srgb, currentColor 55%, transparent);
    text-decoration-thickness: 1px;
    text-underline-offset: 3px;
    cursor: help;
  }
  .term:hover {
    text-decoration-color: currentColor;
  }
  .explain {
    animation: fade-up var(--dur) var(--ease-out);
    display: block;
    max-width: 320px;
    padding: 9px 12px;
    color: var(--text);
    font-size: 12.5px;
    font-weight: 400;
    line-height: 1.5;
    white-space: normal;
    letter-spacing: normal;
    text-transform: none;
  }
  .sr {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
</style>
