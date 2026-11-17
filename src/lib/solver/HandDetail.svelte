<script lang="ts">
  import { signed } from "../app/format";
  import PlayingCard from "../PlayingCard.svelte";
  import { bb } from "./actions";
  import { cap, handAdvice } from "./explain";
  import Term from "./Term.svelte";
  import { sameCombo } from "./practice";
  import { classLabel } from "./range";
  import type { SolverNode } from "./types";

  /** One hand class of the acting player, split into its suit combinations. */
  let {
    node,
    index,
    colors,
    labels,
    hero = null,
    pinned = false,
    onUnpin,
  }: {
    node: SolverNode;
    index: number;
    colors: string[];
    labels: string[];
    hero?: string[] | null;
    pinned?: boolean;
    onUnpin?: () => void;
  } = $props();

  const SUITS = "shdc";
  const cls = $derived(node.classes.find((c) => c.class === index));
  const label = $derived(classLabel(index));

  /** Every combo of the class in a fixed suit order, also those the board blocks. */
  const combos = $derived.by(() => {
    const [a, b] = [label[0], label[1]];
    const pair = a === b;
    const suited = label.endsWith("s");
    const out: string[][] = [];
    for (let i = 0; i < 4; i++) {
      for (let j = 0; j < 4; j++) {
        if (pair ? j <= i : suited ? i !== j : i === j) continue;
        out.push([a + SUITS[i], b + SUITS[j]]);
      }
    }
    return out;
  });
  const board = $derived(new Set(node.board));
  const advice = $derived(cls ? handAdvice(node, cls.strategy, node.hands.filter((h) => h.class === index)) : null);
  const pct = (x: number) => `${Math.round(x * 100)} %`;
</script>

<section class="detail" aria-live="polite">
  <header>
    <b class="num">{label}</b>
    {#if cls}<span class="dim num">{bb(cls.combos, 2)} Kombos</span>{/if}
    {#if pinned}<button class="link" onclick={onUnpin}>lösen</button>{/if}
  </header>
  {#if !cls}
    <p class="faint">Nicht in der Range an dieser Stelle.</p>
  {:else}
    {#if advice}
      <p class="advice">
        <b>{cap(advice.line)}.</b>
        {#if advice.close}<span>{advice.close}</span>{/if}
      </p>
    {/if}
    <div class="mix" aria-hidden="true">
      {#each cls.strategy as s, k (k)}<i style="width:{s * 100}%; background:{colors[k]}"></i>{/each}
    </div>
    <div class="rows">
      {#each cls.strategy as s, k (k)}
        <div class="kv"><span><i class="dot" style="background:{colors[k]}"></i>{labels[k]}</span><b class="num">{pct(s)}</b></div>
      {/each}
      <div class="kv sep"><span><Term term="equity">Equity</Term></span><b class="num">{pct(cls.equity)}</b></div>
      <div class="kv"><span><Term term="ev">EV</Term></span><b class="num">{signed(cls.ev, 2)} BB</b></div>
    </div>
    {#if node.hands.length}
      <table class="combos">
        <thead><tr><th>Kombo</th><th>Strategie</th><th>Eq.</th><th>EV</th></tr></thead>
        <tbody>
          {#each combos as cards (cards.join(""))}
            {@const h = node.hands.find((x) => sameCombo(x.combo, cards))}
            {@const blocked = cards.some((c) => board.has(c))}
            <tr class:off={!h} class:me={hero && sameCombo(cards.join(""), hero)}>
              <td class="cards"><PlayingCard card={cards[0]} size={15} /><PlayingCard card={cards[1]} size={15} /></td>
              {#if h}
                <td>
                  <span class="mini" title={h.strategy.map((s, k) => `${labels[k]} ${pct(s)}`).join(", ")}>
                    {#each h.strategy as s, k (k)}<i style="width:{s * 100}%; background:{colors[k]}"></i>{/each}
                  </span>
                </td>
                <td class="num">{pct(h.equity)}</td>
                <td class="num">{signed(h.ev, 2)}</td>
              {:else}
                <td colspan="3" class="faint why">{blocked ? "liegt auf dem Board" : "nicht in der Range"}</td>
              {/if}
            </tr>
          {/each}
        </tbody>
      </table>
    {:else}
      <p class="faint small">Für diesen Knoten sind keine einzelnen Kombos gespeichert.</p>
    {/if}
  {/if}
</section>

<style>
  .detail {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
  }
  header {
    display: flex;
    align-items: baseline;
    gap: 10px;
  }
  header b {
    font-size: 18px;
    font-weight: 700;
  }
  header .dim {
    font-size: 12px;
  }
  .link {
    margin-left: auto;
    border: 0;
    background: none;
    color: var(--text-2);
    font-size: 12px;
    text-decoration: underline;
    cursor: pointer;
  }
  .advice {
    margin: 0;
    font-size: 13px;
    line-height: 1.45;
  }
  .advice span {
    color: var(--text-2);
  }
  .mix {
    display: flex;
    height: 8px;
    border-radius: 3px;
    overflow: hidden;
    background: var(--surface-2);
  }
  .kv {
    display: flex;
    justify-content: space-between;
    font-size: 12.5px;
    padding: 1px 0;
  }
  .kv b {
    font-weight: 600;
  }
  .sep {
    margin-top: 4px;
    padding-top: 4px;
    border-top: 1px solid var(--line);
  }
  .dot {
    display: inline-block;
    width: 8px;
    height: 8px;
    margin-right: 6px;
    border-radius: 2px;
  }
  .combos {
    width: 100%;
    border-collapse: collapse;
    font-size: 12px;
  }
  .combos th {
    font-size: 10.5px;
    font-weight: 500;
    color: var(--text-3);
    text-align: right;
    padding-bottom: 3px;
  }
  .combos th:first-child,
  .combos th:nth-child(2) {
    text-align: left;
  }
  .combos td {
    padding: 2px 0;
    text-align: right;
  }
  .combos td + td {
    padding-left: 8px;
  }
  .cards {
    display: flex;
    gap: 2px;
    text-align: left;
  }
  .combos td:nth-child(2) {
    text-align: left;
    width: 45%;
  }
  .why {
    text-align: left !important;
    font-size: 11.5px;
  }
  tr.off .cards {
    opacity: 0.35;
  }
  tr.me td {
    font-weight: 700;
  }
  .mini {
    display: flex;
    width: 100%;
    height: 8px;
    border-radius: 2px;
    overflow: hidden;
    background: var(--surface-2);
  }
  .small {
    margin: 0;
    font-size: 12px;
  }
  p {
    margin: 0;
  }
</style>
