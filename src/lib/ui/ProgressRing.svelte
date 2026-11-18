<script lang="ts">
  import type { Snippet } from "svelte";

  /** Progress or a share as a ring; `value` null spins while the end is unknown. */
  let {
    value,
    size = 40,
    stroke = 4,
    tone = "neutral",
    label,
    children,
  }: {
    value: number | null;
    size?: number;
    stroke?: number;
    tone?: "neutral" | "good" | "warn" | "bad" | "info";
    /** Screen-reader name, e.g. "Analyse". */
    label: string;
    children?: Snippet;
  } = $props();

  const r = $derived((size - stroke) / 2);
  const c = $derived(2 * Math.PI * r);
  const v = $derived(value === null ? 0.25 : Math.max(0, Math.min(1, value)));
</script>

<span
  class="ui-ring {tone}"
  class:spin={value === null}
  style:width="{size}px"
  style:height="{size}px"
  role="progressbar"
  aria-label={label}
  aria-valuemin={0}
  aria-valuemax={100}
  aria-valuenow={value === null ? undefined : Math.round(v * 100)}
>
  <svg viewBox="0 0 {size} {size}" aria-hidden="true">
    <circle cx={size / 2} cy={size / 2} {r} stroke-width={stroke} class="track" />
    <circle cx={size / 2} cy={size / 2} {r} stroke-width={stroke} class="bar" stroke-dasharray="{c * v} {c}" />
  </svg>
  {#if children}<span class="center">{@render children()}</span>{/if}
</span>

<style>
  .ui-ring {
    --c: var(--text);
    position: relative;
    display: inline-grid;
    place-items: center;
    flex-shrink: 0;
  }
  .good {
    --c: var(--pos);
  }
  .warn {
    --c: var(--amber);
  }
  .bad {
    --c: var(--neg);
  }
  .info {
    --c: var(--info);
  }
  svg {
    position: absolute;
    inset: 0;
    transform: rotate(-90deg);
  }
  circle {
    fill: none;
  }
  .track {
    stroke: var(--surface-3);
  }
  .bar {
    stroke: var(--c);
    stroke-linecap: round;
    transition: stroke-dasharray var(--dur) var(--ease-standard);
  }
  .spin svg {
    animation: spin var(--dur-loop) var(--ease-spin) infinite;
  }
  .center {
    position: relative;
    font-family: var(--font-num);
    font-size: var(--fs-sm);
    font-weight: var(--fw-semibold);
  }
  @keyframes spin {
    from {
      transform: rotate(-90deg);
    }
    to {
      transform: rotate(270deg);
    }
  }
</style>
