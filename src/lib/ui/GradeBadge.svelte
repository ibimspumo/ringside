<script lang="ts">
  import { bb } from "./format";
  import GradeIcon from "./GradeIcon.svelte";
  import { GRADE, type Grade } from "./poker";

  /**
   * Grade of a decision as icon, word and (for losses) the price in BB, on a tinted plate.
   * `icon` shows only the mark (dense lists); the word stays as tooltip and for screen readers.
   */
  let {
    grade,
    cost,
    size = "md",
    variant = "pill",
  }: {
    grade: Grade;
    /** Lost big blinds, shown for every grade below "good". */
    cost?: number;
    size?: "sm" | "md" | "lg";
    variant?: "pill" | "icon";
  } = $props();

  const g = $derived(GRADE[grade]);
  const iconSize = $derived(size === "sm" ? 14 : size === "lg" ? 20 : 16);
  const showCost = $derived(cost !== undefined && cost > 0.005 && grade !== "best" && grade !== "mix");
</script>

{#if variant === "icon"}
  <span class="ui-grade icon-only" role="img" aria-label={g.label} title={g.label}><GradeIcon {grade} size={iconSize} /></span>
{:else}
  <span class="ui-grade {grade} {size}">
    <GradeIcon {grade} size={iconSize} />
    <span class="word">{g.label}</span>
    {#if showCost}<span class="cost num">−{bb(cost ?? 0, { digits: 2 })}</span>{/if}
  </span>
{/if}

<style>
  .ui-grade {
    --c: var(--grade-best);
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 24px;
    padding: 0 var(--sp-2) 0 4px;
    border-radius: var(--r-pill);
    border: 1px solid color-mix(in srgb, var(--c) 45%, transparent);
    background: color-mix(in srgb, var(--c) 16%, var(--surface));
    color: var(--text);
    font-size: var(--fs-md);
    font-weight: var(--fw-semibold);
    line-height: 1;
    white-space: nowrap;
    vertical-align: middle;
  }
  .icon-only {
    height: auto;
    padding: 0;
    border: 0;
    background: none;
  }
  .sm {
    height: 20px;
    padding-left: 3px;
    gap: 4px;
    font-size: var(--fs-sm);
  }
  .lg {
    height: 30px;
    padding: 0 var(--sp-3) 0 5px;
    font-size: var(--fs-base);
  }
  .mix {
    --c: var(--grade-mix);
  }
  .good {
    --c: var(--grade-good);
  }
  .inaccurate {
    --c: var(--grade-inaccurate);
  }
  .mistake {
    --c: var(--grade-mistake);
  }
  .blunder {
    --c: var(--grade-blunder);
    background: color-mix(in srgb, var(--c) 26%, var(--surface));
  }
  .cost {
    padding-left: 6px;
    border-left: 1px solid color-mix(in srgb, var(--c) 45%, transparent);
    color: color-mix(in srgb, var(--c) 45%, var(--text));
    font-weight: var(--fw-semibold);
  }
</style>
