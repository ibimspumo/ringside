<script lang="ts">
  import type { Snippet } from "svelte";
  import SectionHead from "./SectionHead.svelte";

  /** A titled block of a page. Content runs edge to edge with `flush` (tables, lists). */
  let {
    title,
    sub,
    flush = false,
    actions,
    footer,
    children,
  }: { title?: string; sub?: string; flush?: boolean; actions?: Snippet; footer?: Snippet; children: Snippet } = $props();
</script>

<section class="ui-panel">
  {#if title}
    <div class="head"><SectionHead {title} {sub} level={3} {actions} /></div>
  {/if}
  <div class="body" class:flush>{@render children()}</div>
  {#if footer}<footer>{@render footer()}</footer>{/if}
</section>

<style>
  .ui-panel {
    min-width: 0;
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    background: var(--surface);
  }
  .head {
    padding: var(--sp-4) var(--sp-4) 0;
  }
  .body {
    padding: var(--sp-3) var(--sp-4) var(--sp-4);
  }
  .body.flush {
    padding: var(--sp-3) 0 0;
  }
  footer {
    padding: var(--sp-3) var(--sp-4);
    border-top: 1px solid var(--line);
    color: var(--text-2);
    font-size: var(--fs-sm);
  }
</style>
