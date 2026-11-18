<script lang="ts">
  import type { Grade } from "./poker";

  /**
   * Grade mark that reads without colour: shape and fill differ as well as the glyph.
   * Best and mix are filled circles, good and inaccurate outlined circles, mistake a filled
   * square and blunder a filled diamond-cut square with two crosses.
   */
  let { grade, size = 16 }: { grade: Grade; size?: number } = $props();
</script>

<svg class="ui-grade-icon {grade}" viewBox="0 0 20 20" width={size} height={size} aria-hidden="true">
  {#if grade === "best"}
    <circle cx="10" cy="10" r="9" class="fill" />
    <path d="M5.6 10.4l3 3 5.8-6.6" class="ink" />
  {:else if grade === "mix"}
    <circle cx="10" cy="10" r="9" class="fill" />
    <path d="M5.5 8.4c1.5-1.4 3-1.4 4.5 0s3 1.4 4.5 0M5.5 12.6c1.5-1.4 3-1.4 4.5 0s3 1.4 4.5 0" class="ink" />
  {:else if grade === "good"}
    <circle cx="10" cy="10" r="8.1" class="ring" />
    <path d="M6.2 10.4l2.6 2.6 5-5.8" class="line" />
  {:else if grade === "inaccurate"}
    <circle cx="10" cy="10" r="8.1" class="ring" />
    <path d="M10 5.4v5.6" class="line" />
    <circle cx="10" cy="14.2" r="1.2" class="dot" />
  {:else if grade === "mistake"}
    <rect x="1.5" y="1.5" width="17" height="17" rx="4" class="fill" />
    <path d="M6.6 6.6l6.8 6.8M13.4 6.6l-6.8 6.8" class="ink" />
  {:else}
    <path d="M10 .8l9.2 9.2-9.2 9.2L.8 10z" class="fill" />
    <path d="M5.6 7.8l3 3M8.6 7.8l-3 3M11.4 9.2l3 3M14.4 9.2l-3 3" class="ink thin" />
  {/if}
</svg>

<style>
  .ui-grade-icon {
    --c: var(--grade-best);
    flex-shrink: 0;
    display: inline-block;
    vertical-align: middle;
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
  }
  .fill {
    fill: var(--c);
  }
  .ring {
    fill: color-mix(in srgb, var(--c) 14%, transparent);
    stroke: var(--c);
    stroke-width: 1.8;
  }
  .ink {
    fill: none;
    stroke: var(--bg);
    stroke-width: 2.2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .ink.thin {
    stroke-width: 1.7;
  }
  .line {
    fill: none;
    stroke: var(--c);
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .dot {
    fill: var(--c);
  }
</style>
