<script lang="ts" generics="T extends string">
  /** Sections of one page, underlined. The caller renders the panel for `value`. */
  let {
    tabs,
    value = $bindable(),
    label,
    onchange,
  }: {
    /** `note`: a quiet word on the tab's state, e.g. "nach dem Lösen" for a section still waiting. */
    tabs: { id: T; label: string; count?: number; note?: string; disabled?: boolean; hint?: string }[];
    value: T;
    label: string;
    onchange?: (id: T) => void;
  } = $props();

  let list = $state<HTMLElement>();

  function pick(id: T) {
    if (id === value) return;
    value = id;
    onchange?.(id);
  }

  function key(e: KeyboardEvent) {
    const usable = tabs.filter((t) => !t.disabled);
    const i = usable.findIndex((t) => t.id === value);
    const n = usable.length;
    const at = ({ ArrowRight: i + 1, ArrowLeft: i - 1 + n, Home: 0, End: n - 1 } as Record<string, number>)[e.key];
    if (at === undefined || !n) return;
    e.preventDefault();
    const t = usable[at % n];
    pick(t.id);
    list?.querySelectorAll<HTMLElement>("[role=tab]")[tabs.indexOf(t)]?.focus();
  }
</script>

<div class="ui-tabs" role="tablist" aria-label={label} bind:this={list} onkeydown={key} tabindex="-1">
  {#each tabs as t (t.id)}
    <button
      type="button"
      role="tab"
      aria-selected={t.id === value}
      tabindex={t.id === value ? 0 : -1}
      disabled={t.disabled}
      title={t.hint}
      onclick={() => pick(t.id)}
    >
      {t.label}
      {#if t.count !== undefined}<span class="count num">{t.count}</span>{/if}
      {#if t.note}<span class="note">{t.note}</span>{/if}
    </button>
  {/each}
</div>

<style>
  .ui-tabs {
    display: flex;
    gap: var(--sp-5);
    border-bottom: 1px solid var(--line);
  }
  .ui-tabs:focus {
    outline: none;
  }
  button {
    position: relative;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 40px;
    padding: 0;
    border: 0;
    background: none;
    color: var(--text-2);
    font: inherit;
    font-size: var(--fs-base);
    cursor: pointer;
  }
  button::after {
    content: "";
    position: absolute;
    left: 0;
    right: 0;
    bottom: -1px;
    height: 2px;
    border-radius: 2px 2px 0 0;
    background: transparent;
    transform: scaleX(0.4);
    transition:
      background var(--dur) var(--ease-standard),
      transform var(--dur) var(--ease-spring);
  }
  button:hover:not(:disabled) {
    color: var(--text);
  }
  button[aria-selected="true"] {
    color: var(--text);
    font-weight: var(--fw-semibold);
  }
  button[aria-selected="true"]::after {
    background: var(--text);
    transform: none;
  }
  button:disabled {
    color: var(--text-3);
    cursor: default;
  }
  button:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: 2px;
    border-radius: var(--r-xs);
  }
  .note {
    color: var(--text-3);
    font-size: var(--fs-xs);
    font-weight: var(--fw-regular);
  }
  .count {
    padding: 1px 6px;
    border-radius: var(--r-pill);
    background: var(--surface-2);
    color: var(--text-2);
    font-size: var(--fs-xs);
    font-weight: var(--fw-medium);
  }
</style>
