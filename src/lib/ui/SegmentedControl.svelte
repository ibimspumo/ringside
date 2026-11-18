<script lang="ts" generics="T extends string | number">
  /** A small set of exclusive choices side by side ("Heute · 7 Tage · Alles"). Arrow keys move the choice. */
  let {
    options,
    value = $bindable(),
    label,
    size = "md",
    disabled = false,
    onchange,
  }: {
    options: { value: T; label: string; hint?: string; disabled?: boolean }[];
    value: T;
    /** Names the group for screen readers. */
    label: string;
    size?: "sm" | "md";
    disabled?: boolean;
    onchange?: (value: T) => void;
  } = $props();

  let group = $state<HTMLElement>();

  function pick(v: T) {
    if (v === value) return;
    value = v;
    onchange?.(v);
  }

  function key(e: KeyboardEvent) {
    const step = e.key === "ArrowRight" || e.key === "ArrowDown" ? 1 : e.key === "ArrowLeft" || e.key === "ArrowUp" ? -1 : 0;
    if (!step) return;
    e.preventDefault();
    const usable = options.filter((o) => !o.disabled);
    const at = usable.findIndex((o) => o.value === value);
    const next = usable[(at + step + usable.length) % usable.length];
    pick(next.value);
    group?.querySelectorAll<HTMLElement>("[role=radio]")[options.indexOf(next)]?.focus();
  }
</script>

<div class="ui-seg {size}" role="radiogroup" aria-label={label} bind:this={group} onkeydown={key} tabindex="-1">
  {#each options as o (o.value)}
    <button
      type="button"
      role="radio"
      aria-checked={o.value === value}
      tabindex={o.value === value ? 0 : -1}
      disabled={disabled || o.disabled}
      title={o.hint}
      onclick={() => pick(o.value)}>{o.label}</button
    >
  {/each}
</div>

<style>
  .ui-seg {
    display: inline-flex;
    padding: 2px;
    gap: 2px;
    border-radius: var(--r-md);
    background: var(--surface-sunken);
    border: 1px solid var(--line);
  }
  .ui-seg:focus {
    outline: none;
  }
  button {
    height: 26px;
    padding: 0 var(--sp-3);
    white-space: nowrap;
    border: 0;
    border-radius: var(--r-sm);
    background: transparent;
    color: var(--text-2);
    font: inherit;
    font-size: var(--fs-md);
    cursor: pointer;
    transition:
      background var(--dur-fast) var(--ease-standard),
      color var(--dur-fast) var(--ease-standard);
  }
  .sm button {
    height: 22px;
    padding: 0 var(--sp-2);
    font-size: var(--fs-sm);
  }
  button:hover {
    color: var(--text);
  }
  button[aria-checked="true"] {
    background: var(--surface-3);
    color: var(--text);
    font-weight: var(--fw-medium);
    box-shadow: var(--shadow-1);
  }
  button:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: 1px;
  }
  button:disabled {
    opacity: 0.45;
    cursor: default;
  }
</style>
