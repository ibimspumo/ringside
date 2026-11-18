<script lang="ts" module>
  export interface ColumnBar {
    key: string | number;
    /** Height, 0–100. */
    value: number;
    /** Above the bar, e.g. "46". */
    valueText: string;
    /** Below the bar, e.g. "9". */
    label: string;
    /** Whole sentence for the tooltip and screen readers. */
    title: string;
  }
</script>

<script lang="ts">
  /**
   * A row of small upright bars to compare one number across a few cases; each bar is a
   * button that picks its case. `selected` marks the case on view, `tone` colours the bars.
   */
  let {
    bars,
    selected = null,
    tone = "var(--act-raise)",
    label,
    onpick,
  }: { bars: ColumnBar[]; selected?: string | number | null; tone?: string; label: string; onpick: (key: string | number) => void } = $props();
</script>

<div class="ui-colbars" role="group" aria-label={label} style:--tone={tone}>
  {#each bars as b (b.key)}
    {@const on = b.key === selected}
    <button type="button" class="col" class:on aria-pressed={on} title={b.title} aria-label={b.title} onclick={() => onpick(b.key)}>
      <span class="val num">{b.valueText}</span>
      <span class="track"><span class="fill" style:height="{Math.max(2, Math.min(100, b.value))}%"></span></span>
      <span class="lbl num">{b.label}</span>
    </button>
  {/each}
</div>

<style>
  .ui-colbars {
    display: flex;
    gap: var(--sp-1);
  }
  .col {
    flex: 1 1 0;
    min-width: 0;
    display: grid;
    grid-template-rows: auto 64px auto;
    gap: 4px;
    justify-items: center;
    padding: 4px 0;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: var(--text-3);
    font: inherit;
    cursor: pointer;
    transition:
      color var(--dur-fast) var(--ease-standard),
      background var(--dur-fast) var(--ease-standard);
  }
  .col:hover {
    color: var(--text);
    background: var(--surface-2);
  }
  .col.on {
    color: var(--text);
  }
  .val {
    font-size: var(--fs-sm);
  }
  .lbl {
    font-size: var(--fs-xs);
  }
  .col.on .lbl,
  .col.on .val {
    font-weight: var(--fw-medium);
  }
  .track {
    position: relative;
    width: 100%;
    max-width: 30px;
    border-radius: 4px;
    background: var(--surface-sunken);
    overflow: hidden;
  }
  .col.on .track {
    outline: 1px solid var(--line-2);
    outline-offset: 2px;
  }
  .fill {
    position: absolute;
    inset: auto 0 0 0;
    background: var(--tone);
    opacity: 0.45;
    transition:
      height var(--dur) var(--ease-standard),
      opacity var(--dur-fast) var(--ease-standard);
  }
  .col:hover .fill,
  .col.on .fill {
    opacity: 1;
  }
</style>
