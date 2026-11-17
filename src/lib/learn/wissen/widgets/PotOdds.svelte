<script lang="ts">
  import { alpha, hitChance, int, mdf, num, pct, potOdds } from "../math";

  let { mode = "call" }: { mode?: "call" | "mdf" | "alpha" } = $props();

  const POT = 100;
  let size = $state(50);
  const PRESETS: [string, number][] = [
    ["⅓", 33],
    ["½", 50],
    ["⅔", 67],
    ["Pot", 100],
    ["2×", 200],
  ];

  const need = $derived(potOdds(POT, size));
  const a = $derived(alpha(POT, size));
  const d = $derived(mdf(POT, size));
  const main = $derived(mode === "call" ? need : mode === "mdf" ? d : a);
  const total = $derived(POT + size * 2);

  // Exact draw equities to compare against the price (unknown villain cards counted as unseen).
  const DRAWS = [
    { label: "Flush-Draw, nur Turn", eq: hitChance(9, 1, 47) },
    { label: "Flush-Draw, bis River", eq: hitChance(9, 2, 47) },
  ];
</script>

<div class="po">
  <div class="ctrl">
    <label for="po-size">Bet des Gegners</label>
    <div class="seg" role="group" aria-label="Schnellauswahl">
      {#each PRESETS as [l, v] (l)}
        <button class:on={size === v} onclick={() => (size = v)}>{l}</button>
      {/each}
    </div>
  </div>
  <input id="po-size" type="range" min="10" max="200" step="1" bind:value={size} />

  <div class="pot" aria-hidden="true">
    <span class="seg-p" style="flex:{POT}">Pot <b class="num">{POT}</b></span>
    <span class="seg-b" style="flex:{size}">Bet <b class="num">{size}</b></span>
    {#if mode === "call"}<span class="seg-c" style="flex:{size}">Call <b class="num">{size}</b></span>{/if}
  </div>

  <div class="result">
    <div>
      <span class="lbl">
        {mode === "call" ? "Du brauchst mindestens diese Gewinnchance (Equity)" : mode === "mdf" ? "Verteidige mindestens" : "Der Bluff muss klappen in"}
      </span>
      <b class="big num">{pct(main)}</b>
    </div>
    <div class="side">
      {#if mode === "call"}
        <span title="Was im Pot liegt, gegen das, was du zahlst">Pot Odds <b class="num">{num((POT + size) / size, 1)} : 1</b></span>
        <span title="So groß ist der Pot, nachdem du gecallt hast">Pot nach deinem Call <b class="num">{int(total)}</b></span>
      {:else}
        <span>{mode === "mdf" ? "Alpha" : "MDF"} <b class="num">{pct(mode === "mdf" ? a : d)}</b></span>
        <span>Bet <b class="num">{pct(size / POT, 0)}</b> des Pots</span>
      {/if}
    </div>
  </div>

  <div class="meter">
    <span class="fill" style="width:{main * 100}%"></span>
    {#if mode === "call"}
      {#each DRAWS as dr (dr.label)}
        <span class="mark" class:ok={dr.eq >= need} style="left:{dr.eq * 100}%"></span>
      {/each}
    {/if}
  </div>
  {#if mode === "call"}
    <ul class="draws">
      {#each DRAWS as dr (dr.label)}
        <li>
          <i class:ok={dr.eq >= need}></i>
          <span>{dr.label}</span>
          <b class="num">{pct(dr.eq)}</b>
          <span class:pos={dr.eq >= need} class:neg={dr.eq < need}>{dr.eq >= need ? "reicht ohne weitere Bet" : "reicht nicht"}</span>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .po {
    display: grid;
    gap: 14px;
  }
  .ctrl {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    flex-wrap: wrap;
  }
  label,
  .lbl {
    color: var(--text-2);
    font-size: 13px;
  }
  input[type="range"] {
    width: 100%;
    margin: 0;
  }
  .pot {
    display: flex;
    gap: 3px;
    height: 34px;
    font-size: 12px;
  }
  .pot span {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 0 9px;
    border-radius: 5px;
    overflow: hidden;
    white-space: nowrap;
    min-width: 0;
    transition: flex 0.12s;
  }
  .pot b {
    font-size: 14px;
  }
  .seg-p {
    background: var(--surface-3);
    color: var(--text-2);
  }
  .seg-b {
    background: color-mix(in srgb, var(--text) 18%, var(--surface-2));
  }
  .seg-c {
    background: color-mix(in srgb, var(--text) 82%, transparent);
    color: var(--bg);
    font-weight: 600;
  }
  .result {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 16px;
    margin-top: 6px;
  }
  .result > div:first-child {
    display: grid;
  }
  .big {
    font-size: 44px;
    font-weight: 600;
    line-height: 1.05;
  }
  .side {
    display: grid;
    gap: 2px;
    text-align: right;
    color: var(--text-2);
    font-size: 13px;
  }
  .side b {
    color: var(--text);
    font-size: 16px;
    font-weight: 600;
  }
  .meter {
    position: relative;
    height: 14px;
    border-radius: 99px;
    background: var(--bg);
  }
  .fill {
    display: block;
    height: 100%;
    border-radius: 99px;
    background: var(--text);
    transition: width 0.12s;
  }
  .mark {
    position: absolute;
    top: -4px;
    bottom: -4px;
    width: 3px;
    margin-left: -1.5px;
    border-radius: 2px;
    background: var(--neg);
  }
  .mark.ok {
    background: var(--pos);
  }
  .draws {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 4px;
    font-size: 13px;
  }
  .draws li {
    display: grid;
    grid-template-columns: 10px minmax(0, 1fr) 64px 92px;
    align-items: center;
    gap: 10px;
    color: var(--text-2);
  }
  .draws i {
    width: 3px;
    height: 14px;
    border-radius: 2px;
    background: var(--neg);
  }
  .draws i.ok {
    background: var(--pos);
  }
  .draws b {
    color: var(--text);
    text-align: right;
  }
</style>
