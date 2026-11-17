<script lang="ts">
  let { size = 28 }: { size?: number } = $props();

  const R = 25;
  const GAP = 16;
  const COLORS = ["#e5483b", "#f2b12e", "#78c142", "#a6d07c"];

  function arc(i: number): string {
    const a0 = ((-90 + i * 90 + GAP / 2) * Math.PI) / 180;
    const a1 = ((-90 + (i + 1) * 90 - GAP / 2) * Math.PI) / 180;
    const p = (a: number) => `${(32 + R * Math.cos(a)).toFixed(2)} ${(32 + R * Math.sin(a)).toFixed(2)}`;
    return `M${p(a0)} A${R} ${R} 0 0 1 ${p(a1)}`;
  }
</script>

<svg width={size} height={size} viewBox="0 0 64 64" aria-label="Ringside" role="img">
  {#each COLORS as color, i (color)}
    <path d={arc(i)} stroke={color} stroke-width="8" fill="none" />
  {/each}
  <path
    d="M32 21.5c4.2 4.4 8.5 7.4 8.5 11.2a4.3 4.3 0 0 1-7.1 3.3l1.5 5.5h-5.8l1.5-5.5a4.3 4.3 0 0 1-7.1-3.3c0-3.8 4.3-6.8 8.5-11.2z"
    fill="#ecebe7"
  />
</svg>
