<script lang="ts">
  import RangeGrid, { type RangeCell } from "../../../ui/RangeGrid.svelte";
  import { int, pct } from "../math";
  import { combosOf, matrixHand } from "../poker";

  let { range }: { range: string[] } = $props();

  const inRange = $derived(new Set(range));
  const combos = $derived(range.reduce((s, h) => s + combosOf(h).length, 0));
  const hand = (i: number) => matrixHand(Math.floor(i / 13), i % 13);
  const cells = $derived(
    Array.from({ length: 169 }, (_, i): RangeCell => (inRange.has(hand(i)) ? { color: "var(--act-bet-1)", note: `${combosOf(hand(i)).length} Kombos` } : null)),
  );
</script>

<div class="wrap">
  <div class="grid">
    <RangeGrid {cells} label="Handmatrix, {combos} Kombos markiert">
      {#snippet tip(i)}
        <b class="num">{hand(i)}</b>
        <p class="tip-line">{combosOf(hand(i)).length} Kombos · {inRange.has(hand(i)) ? "in der Range" : "nicht dabei"}</p>
      {/snippet}
    </RangeGrid>
  </div>
  <div class="facts">
    <div><b class="num">{int(combos)}</b><span>Kombos</span></div>
    <div><b class="num">{pct(combos / 1326)}</b><span>aller Starthände</span></div>
  </div>
</div>

<style>
  .wrap {
    display: grid;
    gap: var(--sp-4);
    justify-items: center;
  }
  .grid {
    width: min(100%, 440px);
  }
  .facts {
    display: flex;
    gap: var(--sp-6);
    width: min(100%, 440px);
  }
  .facts div {
    display: grid;
  }
  .facts b {
    font-size: var(--fs-3xl);
    font-weight: var(--fw-semibold);
    line-height: 1.1;
  }
  .facts span {
    color: var(--text-2);
    font-size: var(--fs-md);
  }
  .tip-line {
    margin: 2px 0 0;
    color: var(--text-2);
    font-size: var(--fs-sm);
  }
</style>
