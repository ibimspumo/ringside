<script lang="ts">
  /**
   * A figure with its meaning: label, value, and one line that says whether it is good and why.
   * `tone` colours only the value; the meaning line carries the same message in words.
   */
  let {
    label,
    value,
    unit,
    meaning,
    tone = "neutral",
    size = "md",
    help,
  }: {
    label: string;
    value: string;
    unit?: string;
    meaning?: string;
    tone?: "neutral" | "good" | "warn" | "bad";
    size?: "sm" | "md" | "lg";
    /** Explains the term on hover. */
    help?: string;
  } = $props();
</script>

<div class="ui-stat {size} {tone}">
  <span class="label" title={help} class:help={!!help}>{label}</span>
  <span class="value num">{value}{#if unit}<span class="u">{unit}</span>{/if}</span>
  {#if meaning}<span class="meaning">{meaning}</span>{/if}
</div>

<style>
  .ui-stat {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .label {
    color: var(--text-2);
    font-size: var(--fs-sm);
  }
  .label.help {
    text-decoration: underline dotted color-mix(in srgb, currentColor 50%, transparent);
    text-underline-offset: 3px;
    cursor: help;
    align-self: flex-start;
  }
  .value {
    color: var(--text);
    font-size: var(--fs-2xl);
    font-weight: var(--fw-semibold);
    line-height: var(--lh-tight);
  }
  .sm .value {
    font-size: var(--fs-lg);
  }
  .lg .value {
    font-size: var(--fs-4xl);
  }
  .u {
    margin-left: 3px;
    color: var(--text-3);
    font-size: 0.6em;
    font-weight: var(--fw-medium);
  }
  .good .value {
    color: var(--pos);
  }
  .warn .value {
    color: var(--amber);
  }
  .bad .value {
    color: var(--neg);
  }
  .meaning {
    color: var(--text-2);
    font-size: var(--fs-sm);
    line-height: 1.35;
  }
</style>
