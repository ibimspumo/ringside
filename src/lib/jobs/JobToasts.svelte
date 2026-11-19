<script lang="ts">
  import type { Snippet } from "svelte";
  import ChipRing from "./ChipRing.svelte";
  import { PAGE_LABEL } from "./kinds";
  import { dismiss, jobs } from "./store.svelte";
  import type { JobTarget } from "./types";

  /** `children`: further notes in the same stack, above the tasks'. */
  let { onView, children }: { onView: (target: JobTarget) => void; children?: Snippet } = $props();
</script>

<div class="toasts" aria-live="polite">
  {#if children}<div class="extra">{@render children()}</div>{/if}
  {#each jobs.toasts as t (t.id)}
    <div class="toast" class:failed={t.status === "failed"}>
      <ChipRing share={t.status === "done" ? 1 : 0} size={24} />
      <div class="body">
        <span class="label">{t.status === "done" ? "Fertig" : "Fehlgeschlagen"}</span>
        <b>{t.title}</b>
        {#if t.error}<small>{t.error}</small>{/if}
      </div>
      {#if t.status === "done"}
        <button
          class="btn sm"
          title="Öffnet {PAGE_LABEL[t.target.page] ?? 'die Seite'}"
          onclick={() => {
            dismiss(t.id);
            onView(t.target);
          }}>Ansehen</button
        >
      {/if}
      <button class="x" aria-label="Meldung schließen" onclick={() => dismiss(t.id)}>
        <svg viewBox="0 0 16 16" width="12" height="12" aria-hidden="true"><path d="M4 4l8 8M12 4l-8 8" /></svg>
      </button>
    </div>
  {/each}
</div>

<style>
  /* Top right, below the page head: the main actions of a page sit at the bottom. */
  /* Bottom right: the notes under the top bar and the page header stay readable. */
  .toasts {
    position: fixed;
    right: 20px;
    bottom: 20px;
    z-index: 70;
    display: flex;
    flex-direction: column;
    gap: 8px;
    pointer-events: none;
  }
  .extra {
    display: contents;
    pointer-events: auto;
  }
  .toast {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 380px;
    max-width: calc(100vw - 40px);
    padding: 10px 10px 10px 12px;
    border: 1px solid var(--line-2);
    border-radius: var(--r);
    background: var(--surface-2);
    box-shadow: 0 12px 34px rgba(0, 0, 0, 0.5);
    pointer-events: auto;
    animation: in 0.2s ease-out;
  }
  @keyframes in {
    from {
      opacity: 0;
      transform: translateY(-8px);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .toast {
      animation: none;
    }
  }
  .body {
    display: grid;
    flex: 1;
    min-width: 0;
    line-height: 1.3;
  }
  .label {
    font-size: 11.5px;
    font-weight: 600;
    letter-spacing: 0.02em;
    color: var(--text-3);
  }
  b {
    font-size: 13px;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  small {
    font-size: 12px;
    color: var(--text-2);
  }
  .btn.sm {
    height: 28px;
    padding: 0 11px;
    font-size: 12.5px;
  }
  .x {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border: 0;
    border-radius: var(--r-sm);
    background: transparent;
    color: var(--text-3);
    cursor: pointer;
  }
  .x:hover {
    color: var(--text);
    background: var(--surface-3);
  }
  svg path {
    fill: none;
    stroke: currentColor;
    stroke-width: 1.7;
    stroke-linecap: round;
  }
</style>
