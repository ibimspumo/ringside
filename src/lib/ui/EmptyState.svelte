<script lang="ts">
  import type { Snippet } from "svelte";

  /**
   * Nothing to show yet. Says why in one sentence and offers the next step; `tone` "error" says
   * what went wrong and how to fix it, "loading" what is being prepared. `inline` lays it out
   * as one left-aligned line with the action on the right, for a place inside a list or row;
   * `page` frames it like a panel, for a state that stands in for the whole page.
   */
  let {
    title,
    text,
    tone = "empty",
    compact = false,
    inline = false,
    page = false,
    icon,
    action,
  }: {
    title: string;
    text?: string;
    tone?: "empty" | "loading" | "error";
    compact?: boolean;
    inline?: boolean;
    page?: boolean;
    /** A symbol for what is missing (24×24 strokes); the generic box otherwise. */
    icon?: Snippet;
    action?: Snippet;
  } = $props();
</script>

<div class="ui-state {tone}" class:compact class:inline class:page role={tone === "error" ? "alert" : tone === "loading" ? "status" : undefined} aria-busy={tone === "loading" || undefined}>
  <span class="mark" aria-hidden="true">
    {#if tone === "loading"}
      <span class="spin"></span>
    {:else if tone === "error"}
      <svg viewBox="0 0 24 24"><path d="M12 3l10 18H2z" /><path d="M12 10v5" /><circle cx="12" cy="18" r=".6" /></svg>
    {:else if icon}
      <svg viewBox="0 0 24 24">{@render icon()}</svg>
    {:else}
      <svg viewBox="0 0 24 24"><rect x="3" y="5" width="18" height="14" rx="3" /><path d="M3 10h18" /></svg>
    {/if}
  </span>
  <div class="words">
    <strong>{title}</strong>
    {#if text}<p>{text}</p>{/if}
  </div>
  {#if action}<div class="action">{@render action()}</div>{/if}
</div>

<style>
  .ui-state {
    --c: var(--text-3);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--sp-2);
    padding: var(--sp-10) var(--sp-6);
    text-align: center;
  }
  .words {
    display: flex;
    flex-direction: column;
    align-items: inherit;
    gap: var(--sp-2);
  }
  .compact {
    padding: var(--sp-5) var(--sp-4);
  }
  .page {
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    background: var(--surface);
  }
  .inline {
    flex-direction: row;
    align-items: center;
    gap: var(--sp-3);
    padding: var(--sp-3);
    border: 1px solid var(--line);
    border-radius: var(--r-sm);
    background: var(--surface-sunken);
    text-align: left;
  }
  .inline .words {
    flex: 1;
    min-width: 0;
    align-items: flex-start;
    gap: 2px;
  }
  .inline .mark {
    width: 32px;
    height: 32px;
    margin: 0;
    flex-shrink: 0;
  }
  .inline svg {
    width: 16px;
    height: 16px;
  }
  .inline p {
    max-width: 68ch;
  }
  .inline .action {
    margin: 0;
    flex-shrink: 0;
  }
  .error {
    --c: var(--danger-text);
  }
  .mark {
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    margin-bottom: var(--sp-1);
    border-radius: 50%;
    background: var(--surface-2);
    color: var(--c);
  }
  .error .mark {
    background: color-mix(in srgb, var(--danger) 14%, var(--surface-2));
  }
  svg {
    width: 20px;
    height: 20px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.7;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  strong {
    color: var(--text);
    font-size: var(--fs-base);
    font-weight: var(--fw-semibold);
  }
  p {
    max-width: 44ch;
    margin: 0;
    color: var(--text-2);
    font-size: var(--fs-md);
  }
  .action {
    margin-top: var(--sp-2);
  }
  .spin {
    width: 18px;
    height: 18px;
    border: 2px solid var(--text-3);
    border-right-color: transparent;
    border-radius: 50%;
    animation: spin var(--dur-loop) var(--ease-spin) infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
