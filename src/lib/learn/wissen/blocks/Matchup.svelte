<script lang="ts">
  import { pct } from "../math";
  import { equity } from "../poker";
  import CardRow from "./CardRow.svelte";

  let {
    hero,
    villain,
    board,
  }: { hero: { label: string; cards: string[] }; villain: { label: string; cards: string[] }; board: string[] } = $props();

  const eq = $derived(equity(hero.cards, villain.cards, board));
</script>

<div class="mu">
  <div class="side">
    <span class="lbl">{hero.label}</span>
    <CardRow cards={hero.cards} size={42} />
    <b class="num">{pct(eq.win + eq.tie / 2)}</b>
  </div>
  <div class="board">
    <CardRow cards={board} size={40} />
  </div>
  <div class="side r">
    <span class="lbl">{villain.label}</span>
    <CardRow cards={villain.cards} size={42} />
    <b class="num dim">{pct(eq.lose + eq.tie / 2)}</b>
  </div>
  <div class="bar" role="img" aria-label="Equity {pct(eq.win)} gegen {pct(eq.lose)}">
    <span class="h" style="width:{eq.win * 100}%"></span>
    <span class="t" style="width:{eq.tie * 100}%"></span>
  </div>
  <p class="foot faint">Exakt über alle {eq.runouts.toLocaleString("de-DE")} möglichen Turn- und River-Karten</p>
</div>

<style>
  .mu {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: end;
    gap: 14px 18px;
  }
  .side {
    display: grid;
    justify-items: start;
    gap: 6px;
  }
  .side.r {
    justify-items: end;
  }
  .side b {
    font-size: 30px;
    font-weight: 600;
    line-height: 1;
  }
  .board {
    display: flex;
    justify-content: center;
    padding: 14px 10px;
    border-radius: 999px;
    background: radial-gradient(ellipse at 50% 40%, #23312d 0%, #1b2623 62%, #16201d 100%);
    box-shadow: 0 0 0 5px #1c1f24;
    align-self: center;
  }
  .lbl {
    color: var(--text-3);
    font-size: 12px;
    font-weight: 600;
  }
  .bar {
    grid-column: 1 / -1;
    display: flex;
    height: 12px;
    border-radius: 99px;
    overflow: hidden;
    background: var(--surface-3);
  }
  .bar .h {
    background: var(--text);
  }
  .bar .t {
    background: var(--text-3);
  }
  .foot {
    grid-column: 1 / -1;
    margin: -6px 0 0;
    font-size: 12px;
  }
</style>
