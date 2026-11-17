<script lang="ts">
  import type { Snippet } from "svelte";
  import { GLOSSARY, type Term } from "./explain";

  /** A poker word with its explanation on hover and keyboard focus. */
  let { term, children }: { term: Term; children: Snippet } = $props();
  const id = `term-${Math.random().toString(36).slice(2, 9)}`;
</script>

<button type="button" class="term" aria-describedby={id}
  >{@render children()}<span class="tip-box" role="tooltip" {id}>{GLOSSARY[term]}</span></button
>

<style>
  .term {
    position: relative;
    display: inline;
    padding: 0;
    border: 0;
    background: none;
    color: inherit;
    font: inherit;
    text-decoration: underline dotted color-mix(in srgb, currentColor 55%, transparent);
    text-underline-offset: 3px;
    cursor: help;
  }
  .tip-box {
    position: absolute;
    left: 50%;
    bottom: calc(100% + 6px);
    transform: translateX(-50%);
    z-index: 30;
    width: 260px;
    padding: 8px 10px;
    border-radius: var(--r-sm);
    background: var(--surface-3);
    border: 1px solid var(--line-2);
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.45);
    color: var(--text);
    font-family: var(--font-ui);
    font-size: 12px;
    font-weight: 400;
    line-height: 1.45;
    text-align: left;
    white-space: normal;
    visibility: hidden;
    opacity: 0;
    transition: opacity 0.12s;
    pointer-events: none;
  }
  .term:hover .tip-box,
  .term:focus .tip-box {
    visibility: visible;
    opacity: 1;
  }
  @media (prefers-reduced-motion: reduce) {
    .tip-box {
      transition: none;
    }
  }
</style>
