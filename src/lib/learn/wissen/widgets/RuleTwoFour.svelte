<script lang="ts">
  import { hitChance, num, pct } from "../math";

  let cards = $state<1 | 2>(2);
  let outs = $state(9);

  const W = 520;
  const H = 230;
  const PAD = { l: 40, r: 12, t: 12, b: 28 };
  const MAX_OUTS = 20;
  const MAX_Y = 0.85;

  // From the flop: 47 unseen cards. One card = the turn only.
  const exact = (n: number) => hitChance(n, cards, 47);
  const rule = (n: number) => (n * (cards === 2 ? 4 : 2)) / 100;

  const x = (n: number) => PAD.l + ((n - 1) / (MAX_OUTS - 1)) * (W - PAD.l - PAD.r);
  const y = (p: number) => PAD.t + (1 - Math.min(p, MAX_Y) / MAX_Y) * (H - PAD.t - PAD.b);
  const N = [...Array(MAX_OUTS).keys()].map((i) => i + 1);

  const exactPath = $derived(N.map((n, i) => `${i ? "L" : "M"}${x(n)},${y(exact(n))}`).join(""));
  const rulePath = $derived(N.map((n, i) => `${i ? "L" : "M"}${x(n)},${y(rule(n))}`).join(""));
  const e = $derived(exact(outs));
  const r = $derived(rule(outs));
  const diff = $derived((r - e) * 100);
</script>

<div class="r24">
  <div class="ctrl">
    <div class="seg" role="group" aria-label="Karten, die noch kommen">
      <button class:on={cards === 2} onclick={() => (cards = 2)}>Flop bis River · ×4</button>
      <button class:on={cards === 1} onclick={() => (cards = 1)}>Nur der Turn · ×2</button>
    </div>
  </div>

  <svg viewBox="0 0 {W} {H}" role="img" aria-label="Trefferchance nach Zahl der Outs">
    {#each [0, 0.2, 0.4, 0.6, 0.8] as g (g)}
      <line class="grid" x1={PAD.l} x2={W - PAD.r} y1={y(g)} y2={y(g)} />
      <text class="ax" x={PAD.l - 8} y={y(g) + 4} text-anchor="end">{g * 100}</text>
    {/each}
    {#each [1, 4, 8, 12, 16, 20] as n (n)}
      <text class="ax" x={x(n)} y={H - 8} text-anchor="middle">{n}</text>
    {/each}
    <line class="cursor" x1={x(outs)} x2={x(outs)} y1={PAD.t} y2={H - PAD.b} />
    <path class="rule" d={rulePath} />
    <path class="exact" d={exactPath} />
    <circle class="rule-dot" cx={x(outs)} cy={y(r)} r="5" />
    <circle class="exact-dot" cx={x(outs)} cy={y(e)} r="5" />
  </svg>

  <div class="slider">
    <label for="r24-outs">Outs</label>
    <input id="r24-outs" type="range" min="1" max={MAX_OUTS} bind:value={outs} />
    <b class="num">{outs}</b>
  </div>

  <div class="read">
    <div><span><i class="l-rule"></i>Faustregel</span><b class="num amber">{pct(r, 0)}</b></div>
    <div><span><i class="l-exact"></i>Exakt</span><b class="num">{pct(e)}</b></div>
    <div><span>Abweichung</span><b class="num dim">{diff >= 0 ? "+" : "−"}{num(Math.abs(diff))} Pp</b></div>
  </div>
</div>

<style>
  .r24 {
    display: grid;
    gap: 10px;
  }
  svg {
    width: 100%;
    height: auto;
    max-height: 250px;
    font-family: var(--font-num);
  }
  .grid {
    stroke: var(--line);
  }
  .ax {
    fill: var(--text-3);
    font-size: 11px;
  }
  .cursor {
    stroke: var(--line-2);
    stroke-dasharray: 3 3;
  }
  path {
    fill: none;
    stroke-width: 2.5;
    stroke-linejoin: round;
  }
  .exact {
    stroke: var(--text);
  }
  .rule {
    stroke: var(--amber);
    stroke-dasharray: 6 5;
  }
  .exact-dot {
    fill: var(--text);
  }
  .rule-dot {
    fill: var(--amber);
  }
  .slider {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) 28px;
    align-items: center;
    gap: 12px;
    font-size: 13px;
    color: var(--text-2);
  }
  .slider b {
    color: var(--text);
    font-size: 18px;
    text-align: right;
  }
  .read {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 10px;
  }
  .read div {
    display: grid;
    padding: 8px 12px;
    border-radius: var(--r-sm);
    background: var(--bg);
  }
  .read span {
    color: var(--text-2);
    font-size: 12px;
  }
  .read b {
    font-size: 22px;
    font-weight: 600;
  }
  .amber {
    color: var(--amber);
  }
  i {
    display: inline-block;
    width: 14px;
    height: 0;
    margin: 0 6px 3px 0;
    border-top: 2.5px solid var(--text);
  }
  i.l-rule {
    border-top: 2.5px dashed var(--amber);
  }
</style>
