<script lang="ts">
  import type { Snippet } from "svelte";

  /** Progress ring; `value` is 0–1. The centre shows `children`. */
  let {
    value,
    size = 64,
    stroke = 6,
    tone = "var(--text)",
    label,
    children,
  }: { value: number; size?: number; stroke?: number; tone?: string; label?: string; children?: Snippet } = $props();

  const r = $derived((size - stroke) / 2);
  const c = $derived(2 * Math.PI * r);
  const v = $derived(Math.max(0, Math.min(1, value)));
</script>

<span class="ring" style="--s:{size}px" role={label ? "img" : undefined} aria-label={label}>
  <svg width={size} height={size} viewBox="0 0 {size} {size}" aria-hidden="true">
    <circle cx={size / 2} cy={size / 2} {r} stroke-width={stroke} class="track" />
    <circle
      cx={size / 2}
      cy={size / 2}
      {r}
      stroke-width={stroke}
      class="bar"
      style="--c:{c}; stroke:{tone}; stroke-dasharray:{c}; stroke-dashoffset:{c * (1 - v)}"
    />
  </svg>
  {#if children}<span class="mid">{@render children()}</span>{/if}
</span>

<style>
  .ring {
    position: relative;
    display: inline-grid;
    place-items: center;
    width: var(--s);
    height: var(--s);
    flex-shrink: 0;
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
    stroke-linecap: round;
    transition: stroke-dashoffset 0.7s cubic-bezier(0.2, 0.8, 0.2, 1);
    animation: fill 0.8s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  @keyframes fill {
    from {
      stroke-dashoffset: var(--c, 999);
    }
  }
  .mid {
    position: relative;
    display: grid;
    place-items: center;
    text-align: center;
    line-height: 1.1;
  }
</style>
