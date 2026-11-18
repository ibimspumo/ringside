<script lang="ts">
  import type { Snippet } from "svelte";

  /** Small read-only status ("Live", "3 neu", "Turnier"). Not clickable; use Tag for choices. */
  let {
    tone = "neutral",
    size = "md",
    dot = false,
    dashed = false,
    title,
    children,
  }: {
    tone?: "neutral" | "info" | "good" | "warn" | "bad";
    size?: "sm" | "md";
    /** A leading dot, e.g. for "live". */
    dot?: boolean;
    /** Dashed edge: a judgement that is not settled yet. */
    dashed?: boolean;
    title?: string;
    children: Snippet;
  } = $props();
</script>

<span class="ui-badge {tone} {size}" class:dashed {title}>
  {#if dot}<i aria-hidden="true"></i>{/if}{@render children()}
</span>

<style>
  .ui-badge {
    --c: var(--text-2);
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 22px;
    padding: 0 var(--sp-2);
    border-radius: var(--r-pill);
    border: 1px solid color-mix(in srgb, var(--c) 30%, transparent);
    background: color-mix(in srgb, var(--c) 12%, transparent);
    color: color-mix(in srgb, var(--c) 70%, var(--text));
    font-size: var(--fs-sm);
    font-weight: var(--fw-medium);
    line-height: 1;
    white-space: nowrap;
    vertical-align: middle;
  }
  .sm {
    height: 18px;
    padding: 0 6px;
    font-size: var(--fs-xs);
  }
  .dashed {
    border-style: dashed;
    border-color: color-mix(in srgb, var(--c) 60%, transparent);
    background: transparent;
  }
  .info {
    --c: var(--info);
  }
  .good {
    --c: var(--pos);
  }
  .warn {
    --c: var(--amber);
  }
  .bad {
    --c: var(--neg);
  }
  i {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--c);
  }
</style>
