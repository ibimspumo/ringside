<script lang="ts">
  import type { Snippet } from "svelte";

  /**
   * Nothing to show yet. Says why in one sentence and offers the next step; `tone` "error" says
   * what went wrong and how to fix it, "loading" what is being prepared.
   */
  let {
    title,
    text,
    tone = "empty",
    compact = false,
    action,
  }: {
    title: string;
    text?: string;
    tone?: "empty" | "loading" | "error";
    compact?: boolean;
    action?: Snippet;
  } = $props();
</script>

<div class="ui-state {tone}" class:compact role={tone === "error" ? "alert" : tone === "loading" ? "status" : undefined} aria-busy={tone === "loading" || undefined}>
  <span class="mark" aria-hidden="true">
    {#if tone === "loading"}
      <span class="spin"></span>
    {:else if tone === "error"}
      <svg viewBox="0 0 24 24"><path d="M12 3l10 18H2z" /><path d="M12 10v5" /><circle cx="12" cy="18" r=".6" /></svg>
    {:else}
      <svg viewBox="0 0 24 24"><rect x="3" y="5" width="18" height="14" rx="3" /><path d="M3 10h18" /></svg>
    {/if}
  </span>
  <strong>{title}</strong>
  {#if text}<p>{text}</p>{/if}
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
  .compact {
    padding: var(--sp-5) var(--sp-4);
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
