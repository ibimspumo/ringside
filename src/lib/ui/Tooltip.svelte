<script lang="ts">
  import type { Snippet } from "svelte";
  import { floating, rectOf, type Anchor } from "./floating";

  /**
   * Explains the wrapped element on hover and keyboard focus, after a short delay. Placed by
   * `floating`, so it never clips at the window edge. Text only; use Popover for rich content.
   * Plain words get a tab stop of their own; wrapped buttons and links keep theirs. Esc hides it.
   */
  let {
    text,
    side = "top",
    open: forced = false,
    children,
  }: { text: string; side?: "top" | "bottom" | "right"; open?: boolean; children: Snippet } = $props();

  const id = `tt-${Math.random().toString(36).slice(2, 9)}`;
  let wrap = $state<HTMLElement>();
  let anchor = $state<Anchor | null>(null);
  let timer = 0;
  /** No focusable element inside: the anchor itself takes the focus, so keys reach the text. */
  let ownStop = $state(true);
  const FOCUSABLE = "a[href], button, input, select, textarea, [tabindex]";
  $effect(() => {
    ownStop = !wrap?.querySelector(FOCUSABLE);
  });

  function show() {
    clearTimeout(timer);
    timer = window.setTimeout(() => wrap && (anchor = rectOf(wrap)), 250);
  }
  function hide() {
    clearTimeout(timer);
    anchor = null;
  }
  $effect(() => {
    if (forced && wrap) anchor = rectOf(wrap);
  });
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<span
  class="ui-tooltip-anchor"
  bind:this={wrap}
  tabindex={ownStop ? 0 : undefined}
  aria-describedby={anchor ? id : undefined}
  onpointerenter={show}
  onpointerleave={hide}
  onfocusin={show}
  onfocusout={hide}
  onkeydown={(e) => e.key === "Escape" && anchor && (e.stopPropagation(), hide())}
  role={ownStop ? undefined : "presentation"}>{@render children()}</span
>{#if anchor}<div class="ui-tooltip" {id} role="tooltip" use:floating={{ anchor, side }}>{text}</div>{/if}

<style>
  .ui-tooltip-anchor {
    display: inline-flex;
    border-radius: var(--r-xs);
  }
  .ui-tooltip-anchor:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: 2px;
  }
  .ui-tooltip {
    max-width: 260px;
    padding: 6px var(--sp-2);
    border-radius: var(--r-sm);
    border: 1px solid var(--line-2);
    background: var(--surface-3);
    box-shadow: var(--shadow-2);
    color: var(--text);
    font-size: var(--fs-sm);
    line-height: 1.4;
    pointer-events: none;
    animation: in var(--dur-fast) var(--ease-enter);
  }
  @keyframes in {
    from {
      opacity: 0;
    }
  }
</style>
