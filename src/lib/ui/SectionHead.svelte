<script lang="ts">
  import type { Snippet } from "svelte";

  /** Title of a page section or panel, with an optional explaining line and actions on the right. */
  let {
    title,
    sub,
    level = 2,
    id,
    actions,
  }: { title: string; sub?: string; level?: 2 | 3; id?: string; actions?: Snippet } = $props();
</script>

<header class="ui-section-head">
  <div>
    <svelte:element this={`h${level}`} {id} class="title l{level}">{title}</svelte:element>
    {#if sub}<p class="sub">{sub}</p>{/if}
  </div>
  {#if actions}<div class="actions">{@render actions()}</div>{/if}
</header>

<style>
  .ui-section-head {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: var(--sp-4);
  }
  .title {
    margin: 0;
    color: var(--text);
    font-weight: var(--fw-semibold);
    line-height: var(--lh-tight);
  }
  .l2 {
    font-family: var(--font-num);
    font-size: var(--fs-xl);
    letter-spacing: 0.005em;
  }
  .l3 {
    font-size: var(--fs-base);
  }
  .sub {
    margin: var(--sp-1) 0 0;
    color: var(--text-2);
    font-size: var(--fs-md);
  }
  /* A section's line is never smaller than the lines of the rows inside it. */
  .l2 + .sub {
    font-size: var(--fs-base);
  }
  .actions {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    flex-shrink: 0;
  }
</style>
