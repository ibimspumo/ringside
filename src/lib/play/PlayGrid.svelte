<script lang="ts">
  import { CLASSES, classLabel } from "../solver/range";

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

  function fill(s: number[] | null): string {
    if (!s) return "transparent";
    let at = 0;
    const stops: string[] = [];
    s.forEach((f, k) => {
      if (f <= 0) return;
      const from = at * 100;
      at += f;
      stops.push(`${colors[k]} ${from}% ${at * 100}%`);
    });
    return stops.length ? `linear-gradient(90deg, ${stops.join(", ")})` : "transparent";
  }
</script>

<div class="grid" role="img" aria-label="Range als 13 mal 13 Raster">
  {#each Array.from({ length: CLASSES }, (_, i) => i) as i (i)}
    {@const r = Math.max(0, Math.min(1, reach[i] ?? 0))}
    <div class="cell" class:hero={i === hero} class:out={r < 0.02} title="{classLabel(i)}{r > 0 ? ` · ${Math.round(r * 100)} % in der Range` : ''}">
      <i style="height:{r * 100}%; background:{fill(strategy[i])}"></i>
      <span>{classLabel(i).replace(/[so]$/, "")}</span>
    </div>
  {/each}
</div>

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(13, 1fr);
    gap: 1px;
    background: var(--line);
    border-radius: var(--r-sm);
    overflow: hidden;
    aspect-ratio: 1;
  }
  .cell {
    position: relative;
    background: var(--surface-2);
    min-width: 0;
  }
  .cell i {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
  }
  .cell span {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    font-size: clamp(7px, 0.62vw, 10px);
    font-family: var(--font-num);
    color: rgba(18, 20, 24, 0.82);
    font-weight: 600;
    letter-spacing: -0.02em;
  }
  .cell.out span {
    color: var(--text-3);
    font-weight: 400;
  }
  .cell.hero {
    outline: 2px solid var(--text);
    outline-offset: -2px;
    z-index: 1;
  }
</style>
