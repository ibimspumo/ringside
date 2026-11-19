<script lang="ts">
  import { classCombos, classLabel, comboClass } from "../solver/range";
  import { actionColor, type ActionKind } from "../ui/poker";
  import RangeGrid, { type RangeCell } from "../ui/RangeGrid.svelte";
  import type { SpotGrid } from "./spots";

  /** How every hand plays a learning spot, the hero's hand outlined. */
  let { grid, cards }: { grid: SpotGrid; cards: string[] } = $props();

  const KIND: Record<string, ActionKind> = { fold: "fold", check: "check", call: "call", limp: "limp", bet: "bet", raise: "raise", all_in: "allin" };
  const color = (kind: string) => actionColor(KIND[kind] ?? "call");
  /** Fold last, as in every range on the app's pages. */
  const order = $derived([...grid.actions.keys()].sort((a, b) => Number(grid.actions[a].kind === "fold") - Number(grid.actions[b].kind === "fold")));
  const hero = $derived(cards.length === 2 ? comboClass(cards[0], cards[1]) : -1);

  /** Fill height against the hand that gets here most often, so a narrow river range stays readable. */
  const top = $derived(grid.weights ? Math.max(1, ...grid.weights) : 100);
  const cells = $derived(
    grid.cells.map((c, i): RangeCell => {
      if (!c) return null;
      const weight = grid.weights ? grid.weights[i] / top : 1;
      return {
        mix: order.map((a) => ({ color: color(grid.actions[a].kind), share: c[a] / 100 })),
        weight,
        note: `${classLabel(i)}: ${order
          .filter((a) => c[a] > 0)
          .map((a) => `${grid.actions[a].label} ${c[a]} %`)
          .join(", ")}`,
      };
    }),
  );

  /** Share of the hands here that take each action, weighted by combos and reach. */
  const shares = $derived.by(() => {
    const total = new Array(grid.actions.length).fill(0);
    let all = 0;
    grid.cells.forEach((c, i) => {
      if (!c) return;
      const combos = classCombos(i) * (grid.weights ? grid.weights[i] / 100 : 1);
      all += combos;
      c.forEach((f, a) => (total[a] += (f / 100) * combos));
    });
    return total.map((t) => (all > 0 ? Math.round((t / all) * 100) : 0));
  });
</script>

<div class="spot-range">
  <ul class="legend" aria-label="Aktionen">
    {#each order as a (a)}
      <li><i style:background={color(grid.actions[a].kind)}></i>{grid.actions[a].label}<span class="share">{shares[a]} %</span></li>
    {/each}
  </ul>
  <RangeGrid {cells} {hero} size="sm" minFill={grid.weights ? 0.3 : 0} label="Wie jede Hand diesen Spot spielt" />
  <p class="foot">
    Jedes Feld ist eine Hand, deine ist umrandet.{#if grid.weights}{" "}Niedrig gefüllte Felder kommen auf diesem Weg seltener hier an.{/if}
  </p>
</div>

<style>
  .spot-range {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }
  .legend {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-2) var(--sp-4);
    margin: 0;
    padding: 0;
    list-style: none;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .legend li {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-1);
  }
  .legend i {
    width: 10px;
    height: 10px;
    border-radius: var(--r-xs);
  }
  .share {
    color: var(--text-3);
    font-family: var(--font-num);
  }
  .foot {
    margin: 0;
    font-size: var(--fs-sm);
    color: var(--text-3);
  }
</style>
