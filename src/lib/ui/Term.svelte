<script lang="ts">
  import type { Snippet } from "svelte";
  import Tooltip from "./Tooltip.svelte";
  import { WORDS, explain, type WordId } from "./words";

  /**
   * A poker word with its explanation where it stands: dotted underline, reachable by keyboard,
   * the explanation opens on hover and focus through Tooltip and stays inside the window.
   * `id` takes word and explanation from the glossary (words.ts); `tip` is for explanations that
   * belong to one place only. Without children the glossary's word is shown.
   */
  let {
    id,
    tip,
    side = "top",
    children,
  }: { id?: WordId; tip?: string; side?: "top" | "bottom" | "right" | "left"; children?: Snippet } = $props();

  const text = $derived(tip ?? (id ? explain(id) : ""));
</script>

<Tooltip {text} {side} inline
  ><button type="button" class="ui-term">{#if children}{@render children()}{:else if id}{WORDS[id].word}{/if}</button></Tooltip
>

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
