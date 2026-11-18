<script lang="ts">
  /** Text search with its magnifier; Escape empties it. */
  let {
    value = $bindable(""),
    label,
    placeholder = label,
    full = false,
  }: { value?: string; label: string; placeholder?: string; full?: boolean } = $props();

  function key(e: KeyboardEvent) {
    if (e.key === "Escape" && value) {
      value = "";
      e.stopPropagation();
    }
  }
</script>

<label class="ui-search" class:full>
  <svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true"><path d="M10.5 17a6.5 6.5 0 1 0 0-13 6.5 6.5 0 0 0 0 13zM15.5 15.5 20 20" /></svg>
  <input type="search" aria-label={label} {placeholder} bind:value onkeydown={key} />
</label>

<style>
  .ui-search {
    position: relative;
    display: inline-flex;
    align-items: center;
    width: 280px;
    max-width: 100%;
  }
  .full {
    width: 100%;
  }
  svg {
    position: absolute;
    left: var(--sp-3);
    fill: none;
    stroke: var(--text-3);
    stroke-width: 1.7;
    stroke-linecap: round;
    pointer-events: none;
  }
  input {
    width: 100%;
    height: 32px;
    padding: 0 var(--sp-3) 0 34px;
    border: 1px solid var(--line-2);
    border-radius: var(--r-sm);
    background: var(--surface-sunken);
    color: var(--text);
    font: inherit;
    font-size: var(--fs-md);
    transition: border-color var(--dur-fast) var(--ease-standard);
  }
  input::placeholder {
    color: var(--text-3);
  }
  input:focus {
    outline: none;
    border-color: var(--focus);
  }
  input:focus-visible {
    box-shadow: 0 0 0 1px var(--focus);
  }
</style>
