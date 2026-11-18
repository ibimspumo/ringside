<script lang="ts">
  import IconButton from "./IconButton.svelte";

  /**
   * Short message after an action ("Gespeichert", "Analyse fertig"). The caller stacks toasts in
   * a fixed corner; errors say what failed and what to do.
   */
  let {
    tone = "neutral",
    title,
    text,
    action,
    onclose,
  }: {
    tone?: "neutral" | "good" | "bad";
    title: string;
    text?: string;
    action?: { label: string; onclick: () => void };
    onclose?: () => void;
  } = $props();
</script>

<div class="ui-toast {tone}" role={tone === "bad" ? "alert" : "status"}>
  <i aria-hidden="true"></i>
  <div class="text">
    <strong>{title}</strong>
    {#if text}<span>{text}</span>{/if}
  </div>
  {#if action}<button type="button" class="act" onclick={action.onclick}>{action.label}</button>{/if}
  {#if onclose}
    <IconButton label="Schließen" size="sm" onclick={onclose}><svg viewBox="0 0 16 16"><path d="M4 4l8 8M12 4l-8 8" /></svg></IconButton>
  {/if}
</div>

<style>
  .ui-toast {
    --c: var(--text-3);
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    width: 360px;
    max-width: calc(100vw - 32px);
    padding: 10px var(--sp-2) 10px var(--sp-4);
    border-radius: var(--r);
    border: 1px solid var(--line-2);
    background: var(--surface-3);
    box-shadow: var(--shadow-3);
    animation: in var(--dur) var(--ease-enter);
  }
  .good {
    --c: var(--pos);
  }
  .bad {
    --c: var(--neg);
  }
  i {
    align-self: stretch;
    width: 3px;
    border-radius: 2px;
    background: var(--c);
  }
  .text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex: 1;
    min-width: 0;
  }
  strong {
    font-size: var(--fs-md);
    font-weight: var(--fw-semibold);
  }
  span {
    color: var(--text-2);
    font-size: var(--fs-sm);
  }
  .act {
    height: 28px;
    padding: 0 10px;
    border: 0;
    border-radius: var(--r-sm);
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: var(--fs-md);
    font-weight: var(--fw-semibold);
    text-decoration: underline;
    text-underline-offset: 3px;
    cursor: pointer;
  }
  .act:hover {
    background: var(--surface-2);
  }
  .act:focus-visible {
    outline: 2px solid var(--focus);
  }
  @keyframes in {
    from {
      opacity: 0;
      transform: translateY(8px);
    }
  }
</style>
