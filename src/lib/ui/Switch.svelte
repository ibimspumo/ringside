<script lang="ts">
  /** On or off for a setting that applies at once, without a save button. `label` names it for screen readers. */
  let {
    checked = $bindable(false),
    label,
    disabled = false,
    title,
    onchange,
  }: { checked?: boolean; label: string; disabled?: boolean; title?: string; onchange?: (on: boolean) => void } = $props();

  function toggle() {
    checked = !checked;
    onchange?.(checked);
  }
</script>

<button type="button" role="switch" class="ui-switch" aria-checked={checked} aria-label={label} {disabled} {title} onclick={toggle}>
  <span class="knob" aria-hidden="true"></span>
</button>

<style>
  .ui-switch {
    position: relative;
    flex-shrink: 0;
    width: 38px;
    height: 22px;
    padding: 0;
    border: 1px solid var(--line-2);
    border-radius: var(--r-pill);
    background: var(--surface-3);
    cursor: pointer;
    transition:
      background var(--dur-fast) var(--ease-standard),
      border-color var(--dur-fast) var(--ease-standard);
  }
  .knob {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: var(--text-2);
    box-shadow: var(--shadow-1);
    transition:
      transform var(--dur-fast) var(--ease-standard),
      background var(--dur-fast) var(--ease-standard);
  }
  .ui-switch:hover .knob {
    background: var(--text);
  }
  .ui-switch[aria-checked="true"] {
    background: var(--pos);
    border-color: var(--pos);
  }
  .ui-switch[aria-checked="true"] .knob {
    transform: translateX(16px);
    background: var(--text);
  }
  .ui-switch:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: 2px;
  }
  .ui-switch:disabled {
    opacity: 0.45;
    cursor: default;
  }
</style>
