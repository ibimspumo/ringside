<script lang="ts">
  import { floating, signed, type Anchor } from "../app/format";
  import { CLASSES, classCombos, classLabel } from "./range";
  import type { SolverClass, SolverNode } from "./types";

  /**
   * 13×13 strategy of the acting player: each cell splits into the actions' colours by
   * frequency, and fills from the bottom by how many of its combos reach the node.
   */
  let {
    node,
    labels,
    colors,
    hero = null,
  }: { node: SolverNode; labels: string[]; colors: string[]; hero?: string[] | null } = $props();

  const byClass = $derived(new Map(node.classes.map((c) => [c.class, c])));
  const heroClass = $derived.by(() => {
    if (!hero || hero.length !== 2) return -1;
    const combo = node.hands.find((h) => sameCombo(h.combo, hero));
    return combo ? combo.class : -1;
  });

  function sameCombo(combo: string, cards: string[]): boolean {
    const a = combo.slice(0, 2);
    const b = combo.slice(2, 4);
    return (a === cards[0] && b === cards[1]) || (a === cards[1] && b === cards[0]);
  }

  let hover = $state<{ i: number; anchor: Anchor } | null>(null);

  function show(e: MouseEvent, i: number) {
    if (!byClass.has(i)) {
      hover = null;
      return;
    }
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    hover = { i, anchor: { left: r.left, top: r.top, right: r.right, bottom: r.bottom } };
  }

  function bands(c: SolverClass): string {
    let at = 0;
    const stops: string[] = [];
    c.strategy.forEach((s, k) => {
      const from = at * 100;
      at += s;
      stops.push(`${colors[k]} ${from.toFixed(2)}% ${(at * 100).toFixed(2)}%`);
    });
    return `linear-gradient(to right, ${stops.join(", ")})`;
  }

  const pct = (x: number) => `${Math.round(x * 100)} %`;
  const num = (x: number, d = 1) => x.toLocaleString("de-DE", { minimumFractionDigits: d, maximumFractionDigits: d });
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="matrix" onmouseleave={() => (hover = null)}>
  {#each Array.from({ length: CLASSES }, (_, i) => i) as i (i)}
    {@const c = byClass.get(i)}
    <div
      class="cell"
      class:in={!!c}
      class:hero={i === heroClass}
      onmouseenter={(e) => show(e, i)}
      aria-label={c ? `${c.label}: ${c.strategy.map((s, k) => `${labels[k]} ${pct(s)}`).join(", ")}` : classLabel(i)}
      role="img"
    >
      {#if c}
        <i class="bar" style="height:{Math.max(8, Math.min(1, c.combos / classCombos(i)) * 100)}%; background:{bands(c)}"></i>
      {/if}
      <span class="k">{classLabel(i)}</span>
    </div>
  {/each}
  {#if hover}
    {@const c = byClass.get(hover.i)}
    {#if c}
      {@const combos = node.hands.filter((h) => h.class === c.class).sort((a, b) => b.ev - a.ev)}
      <div class="tip wide" use:floating={{ anchor: hover.anchor, side: "right" }}>
        <header><b>{c.label}</b> · {num(c.combos, 2)} Kombos</header>
        {#each c.strategy as s, k (k)}
          <div class="row"><span><i class="dot" style="background:{colors[k]}"></i>{labels[k]}</span><b>{pct(s)}</b></div>
        {/each}
        <div class="row sep"><span>Equity</span><b>{pct(c.equity)}</b></div>
        <div class="row"><span>EV</span><b>{signed(c.ev, 2)} BB</b></div>
        <table class="combos">
          <tbody>
            {#each combos as h (h.combo)}
              <tr class:me={hero && sameCombo(h.combo, hero)}>
                <td class="num">{h.combo}</td>
                <td class="mix">
                  <span class="mini">
                    {#each h.strategy as s, k (k)}<i style="width:{s * 100}%; background:{colors[k]}"></i>{/each}
                  </span>
                </td>
                <td class="num">{pct(h.equity)}</td>
                <td class="num">{signed(h.ev, 2)}</td>
              </tr>
            {/each}
          </tbody>
        </table>
        <p class="faint legend">Kombo · Strategie · Equity · EV (BB)</p>
      </div>
    {/if}
  {/if}
</div>

<style>
  .matrix {
    position: relative;
    display: grid;
    grid-template-columns: repeat(13, minmax(0, 1fr));
    gap: 2px;
  }
  .cell {
    position: relative;
    aspect-ratio: 1;
    min-width: 0;
    border-radius: 3px;
    overflow: hidden;
    background: var(--surface-2);
    color: var(--text-3);
    cursor: default;
  }
  .cell.in {
    background: color-mix(in srgb, var(--surface-3) 70%, var(--surface-2));
    color: #f4f3ef;
  }
  .cell.hero {
    outline: 2px solid var(--text);
    outline-offset: -1px;
    z-index: 1;
  }
  .cell:hover {
    outline: 1px solid var(--text);
    z-index: 2;
  }
  .bar {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
  }
  .k {
    position: relative;
    display: block;
    padding: 3px 4px;
    font-family: var(--font-num);
    font-size: 11px;
    font-weight: 600;
    line-height: 1;
    text-shadow:
      0 0 2px rgba(0, 0, 0, 0.9),
      0 1px 3px rgba(0, 0, 0, 0.6);
  }
  .cell:not(.in) .k {
    text-shadow: none;
  }
  .tip.wide {
    min-width: 230px;
    max-width: 300px;
  }
  .dot {
    display: inline-block;
    width: 8px;
    height: 8px;
    margin-right: 6px;
    border-radius: 2px;
  }
  .sep {
    margin-top: 4px;
    padding-top: 4px;
    border-top: 1px solid var(--line-2);
  }
  .combos {
    width: 100%;
    margin-top: 6px;
    border-collapse: collapse;
    font-size: 11.5px;
  }
  .combos td {
    padding: 1px 0;
  }
  .combos td + td {
    padding-left: 8px;
    text-align: right;
  }
  .combos tr.me td {
    color: var(--text);
    font-weight: 600;
  }
  .mix {
    width: 70px;
  }
  .mini {
    display: flex;
    width: 70px;
    height: 7px;
    border-radius: 2px;
    overflow: hidden;
    background: var(--surface-2);
  }
  .legend {
    margin: 4px 0 0;
    font-size: 11px;
  }
  @container (max-width: 560px) {
    .k {
      padding: 2px;
      font-size: 8.5px;
    }
  }
</style>
