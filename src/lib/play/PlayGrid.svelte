<script lang="ts">
  import { CLASSES, classLabel } from "../solver/range";
  import RangeGrid, { type RangeCell } from "../ui/RangeGrid.svelte";

  /**
   * 13×13 range of the player to act: each cell splits into the choices' colours by frequency
   * and fills from the bottom by how much of the class reaches the decision.
   */
  let {
    strategy,
    reach,
    colors,
    hero = -1,
  }: { strategy: (number[] | null)[]; reach: number[]; colors: string[]; hero?: number } = $props();

  const cells = $derived(
    Array.from({ length: CLASSES }, (_, i): RangeCell => {
      const r = Math.max(0, Math.min(1, reach[i] ?? 0));
      const s = strategy[i];
      if (!s || r < 0.02) return null;
      return {
        mix: s.map((f, k) => ({ color: colors[k], share: f })).filter((m) => m.share > 0),
        weight: r,
        note: `${classLabel(i)} · ${Math.round(r * 100)} % in der Range`,
      };
    }),
  );
</script>

<RangeGrid {cells} {hero} size="sm" label="Range als 13 mal 13 Raster" />
