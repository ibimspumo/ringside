<script lang="ts">
  import type { Snippet } from "svelte";
  import Tooltip from "./Tooltip.svelte";

  /**
   * A poker word with its explanation where it stands: dotted underline, reachable by keyboard,
   * the explanation opens on hover and focus through Tooltip and stays inside the window.
   */
  let {
    tip,
    side = "top",
    children,
  }: { tip: string; side?: "top" | "bottom" | "right" | "left"; children: Snippet } = $props();
</script>

<Tooltip text={tip} {side}><button type="button" class="ui-term">{@render children()}</button></Tooltip>

<style>
  .ui-term {
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
  .ui-term:hover {
    text-decoration-color: currentColor;
  }
</style>
