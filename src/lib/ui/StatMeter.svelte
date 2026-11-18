<script lang="ts">
  /**
   * One value on its scale: the usual range as a band, the value as a mark in its HUD colour.
   * `thin` (too few chances) draws the mark hollow, so an unsure value never looks settled.
   */
  let {
    value,
    low,
    high,
    max = 100,
    color = "var(--text-2)",
    thin = false,
    size = "md",
    label,
  }: {
    value: number | null;
    low: number;
    high: number;
    max?: number;
    color?: string;
    thin?: boolean;
    size?: "sm" | "md";
    /** Read out instead of the picture, e.g. "VPIP 33 %, normal 20–35 %". */
    label: string;
  } = $props();

  const at = (v: number) => `${(Math.max(0, Math.min(max, v)) / max) * 100}%`;
</script>

<span class="ui-meter {size}" class:thin class:empty={value === null} role="img" aria-label={label} style="--c:{color}">
  <i class="band" style="left:{at(low)};right:calc(100% - {at(high)})"></i>
  {#if value !== null}<i class="mark" style="left:{at(value)}"></i>{/if}
</span>

<style>
  .ui-meter {
    position: relative;
    display: block;
    width: 100%;
    height: 14px;
  }
  .ui-meter::before {
    content: "";
    position: absolute;
    inset: 6px 0;
    border-radius: var(--r-pill);
    background: var(--surface-3);
  }
  .band {
    position: absolute;
    top: 4px;
    bottom: 4px;
    border-radius: var(--r-pill);
    background: color-mix(in srgb, var(--text) 24%, transparent);
  }
  .mark {
    position: absolute;
    top: 50%;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--c);
    box-shadow: 0 0 0 2px var(--surface);
    transform: translate(-50%, -50%);
    transition: left var(--dur) var(--ease-standard);
  }
  .thin .mark {
    background: var(--surface);
    box-shadow:
      inset 0 0 0 2px var(--c),
      0 0 0 2px var(--surface);
  }
  .sm {
    height: 10px;
  }
  .sm::before {
    inset: 4px 0;
  }
  .sm .band {
    top: 2px;
    bottom: 2px;
  }
  .sm .mark {
    width: 8px;
    height: 8px;
  }
  @media (prefers-reduced-motion: reduce) {
    .mark {
      transition: none;
    }
  }
</style>
