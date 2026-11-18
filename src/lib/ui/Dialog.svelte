<script lang="ts">
  import type { Snippet } from "svelte";
  import IconButton from "./IconButton.svelte";

  /**
   * Modal on the native <dialog>: focus stays inside, Escape and the backdrop close it, and the
   * focus returns to where it was. `actions` sit bottom right, the primary one last.
   */
  let {
    open = $bindable(false),
    title,
    sub,
    size = "md",
    onclose,
    children,
    actions,
  }: {
    open: boolean;
    title: string;
    sub?: string;
    size?: "sm" | "md" | "lg";
    onclose?: () => void;
    children: Snippet;
    actions?: Snippet;
  } = $props();

  let dialog = $state<HTMLDialogElement>();

  $effect(() => {
    if (!dialog) return;
    if (open && !dialog.open) dialog.showModal();
    if (!open && dialog.open) dialog.close();
  });

  function closed() {
    open = false;
    onclose?.();
  }
</script>

<dialog
  bind:this={dialog}
  class="ui-dialog {size}"
  aria-labelledby="{title}-title"
  onclose={closed}
  onclick={(e) => e.target === dialog && dialog?.close()}
>
  <header>
    <div>
      <h2 id="{title}-title">{title}</h2>
      {#if sub}<p>{sub}</p>{/if}
    </div>
    <IconButton label="Schließen" size="sm" onclick={() => dialog?.close()}>
      <svg viewBox="0 0 16 16"><path d="M4 4l8 8M12 4l-8 8" /></svg>
    </IconButton>
  </header>
  <div class="body">{@render children()}</div>
  {#if actions}<footer>{@render actions()}</footer>{/if}
</dialog>

<style>
  .ui-dialog {
    width: min(480px, calc(100vw - 32px));
    max-height: calc(100vh - 48px);
    padding: 0;
    border: 1px solid var(--line-2);
    border-radius: var(--r-xl);
    background: var(--surface);
    box-shadow: var(--shadow-3);
    color: var(--text);
  }
  .sm {
    width: min(380px, calc(100vw - 32px));
  }
  .lg {
    width: min(720px, calc(100vw - 32px));
  }
  .ui-dialog[open] {
    animation: in var(--dur) var(--ease-enter);
  }
  .ui-dialog::backdrop {
    background: var(--overlay);
  }
  header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: var(--sp-3);
    padding: var(--sp-5) var(--sp-5) 0 var(--sp-6);
  }
  h2 {
    margin: 0;
    font-size: var(--fs-xl);
    font-weight: var(--fw-semibold);
  }
  header p {
    margin: var(--sp-1) 0 0;
    color: var(--text-2);
    font-size: var(--fs-md);
  }
  .body {
    padding: var(--sp-4) var(--sp-6) var(--sp-5);
    font-size: var(--fs-base);
    line-height: var(--lh-base);
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: var(--sp-2);
    padding: var(--sp-3) var(--sp-6) var(--sp-5);
  }
  @keyframes in {
    from {
      opacity: 0;
      transform: translateY(6px) scale(0.98);
    }
  }
</style>
