<script lang="ts">
  /**
   * Progress as the edge of a poker chip: sixteen segments that light up one by one. With
   * no known share a pair of segments circles instead.
   */
  let {
    share = null,
    size = 32,
    count = null,
    dim = false,
  }: { share?: number | null; size?: number; count?: number | null; dim?: boolean } = $props();

  const N = 16;
  const R = 13;
  const GAP = 5.5;

  function arc(i: number): string {
    const a0 = ((i * 360) / N + GAP / 2 - 90) * (Math.PI / 180);
    const a1 = (((i + 1) * 360) / N - GAP / 2 - 90) * (Math.PI / 180);
    const p = (a: number) => `${(16 + R * Math.cos(a)).toFixed(2)} ${(16 + R * Math.sin(a)).toFixed(2)}`;
    return `M${p(a0)}A${R} ${R} 0 0 1 ${p(a1)}`;
  }
  const SEGMENTS = Array.from({ length: N }, (_, i) => arc(i));

  /** 0–1 per segment: full, partly lit, off. */
  const lit = $derived(SEGMENTS.map((_, i) => (share === null ? 0 : Math.max(0, Math.min(1, share * N - i)))));
</script>

<span class="ring" class:dim class:spin={share === null && count !== 0} style="--size:{size}px">
  <svg viewBox="0 0 32 32" width={size} height={size} aria-hidden="true">
    {#each SEGMENTS as d, i (i)}
      <path class="track" {d} />
      {#if lit[i] > 0}<path class="on" {d} style="opacity:{0.35 + 0.65 * lit[i]}" />{/if}
    {/each}
    {#if share === null && count !== 0}
      <g class="orbit">
        <path class="on" d={SEGMENTS[0]} />
        <path class="on" d={SEGMENTS[1]} style="opacity:.5" />
      </g>
    {/if}
  </svg>
  {#if count !== null}<b class="num">{count}</b>{/if}
</span>

<style>
  .ring {
    position: relative;
    display: inline-grid;
    place-items: center;
    width: var(--size);
    height: var(--size);
    flex-shrink: 0;
  }
  svg {
    position: absolute;
    inset: 0;
    fill: none;
    stroke-width: 3.2;
    stroke-linecap: butt;
  }
  .track {
    stroke: var(--surface-3);
  }
  .on {
    stroke: var(--text);
    transition: opacity 0.3s;
  }
  .dim .on {
    stroke: var(--text-3);
  }
  b {
    font-size: calc(var(--size) * 0.4);
    font-weight: 600;
    line-height: 1;
  }
  .orbit {
    transform-origin: 16px 16px;
  }
  .spin .orbit {
    animation: orbit 1.6s steps(16) infinite;
  }
  @keyframes orbit {
    to {
      transform: rotate(360deg);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .spin .orbit {
      animation: none;
    }
  }
</style>
