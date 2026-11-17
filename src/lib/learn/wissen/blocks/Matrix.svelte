<script lang="ts">
  import { int, pct } from "../math";
  import { combosOf, matrixHand } from "../poker";

  let { range }: { range: string[] } = $props();

  const inRange = $derived(new Set(range));
  const combos = $derived(range.reduce((s, h) => s + combosOf(h).length, 0));
  let hover = $state<string | null>(null);
  const IDX = [...Array(13).keys()];
</script>

<div class="wrap">
  <div class="grid" role="img" aria-label="Handmatrix, {combos} Kombos markiert" onpointerover={(e) => (hover = (e.target as HTMLElement).dataset.h ?? hover)}
    onpointerleave={() => (hover = null)}
  >
    {#each IDX as i (i)}
      {#each IDX as j (j)}
        {@const h = matrixHand(i, j)}
        <span class="cell" class:on={inRange.has(h)} class:pair={i === j} data-h={h}>{h}</span>
      {/each}
    {/each}
  </div>
  <div class="facts">
    <div><b class="num">{int(combos)}</b><span>Kombos</span></div>
    <div><b class="num">{pct(combos / 1326)}</b><span>aller Starthände</span></div>
    <div class="hov">
      {#if hover}
        <b class="num">{hover}</b><span>{combosOf(hover).length} Kombos · {inRange.has(hover) ? "in der Range" : "nicht dabei"}</span>
      {:else}
        <span class="faint">Zeig auf ein Feld</span>
      {/if}
    </div>
  </div>
</div>

<style>
  .wrap {
    display: grid;
    gap: 16px;
    justify-items: center;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(13, 1fr);
    gap: 2px;
    width: min(100%, 440px);
    aspect-ratio: 1;
  }
  .cell {
    display: grid;
    place-items: center;
    border-radius: 3px;
    background: var(--bg);
    color: var(--text-3);
    font-family: var(--font-num);
    font-size: 10px;
    letter-spacing: -0.02em;
    user-select: none;
    overflow: hidden;
  }
  .cell.pair {
    background: var(--surface-2);
  }
  .cell.on {
    background: color-mix(in srgb, var(--text) 82%, transparent);
    color: var(--bg);
    font-weight: 600;
  }
  .cell:hover {
    outline: 1.5px solid var(--focus);
  }
  .facts {
    display: grid;
    grid-template-columns: auto auto minmax(0, 1fr);
    gap: 24px;
    width: min(100%, 440px);
  }
  .facts div {
    display: grid;
  }
  .facts b {
    font-size: 26px;
    font-weight: 600;
    line-height: 1.1;
  }
  .facts span {
    color: var(--text-2);
    font-size: 13px;
  }
  .hov {
    min-height: 48px;
    justify-items: end;
    text-align: right;
  }
  .hov b {
    font-size: 18px;
  }
</style>
