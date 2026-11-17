<script lang="ts">
  import { pct, wilson } from "../math";

  let { rows }: { rows: { k: number; n: number }[] } = $props();

  const items = $derived(rows.map((r) => ({ ...r, ci: wilson(r.k, r.n) })));
  const TICKS = [0, 25, 50, 75, 100];
</script>

<div class="sample">
  <div class="axis">
    {#each TICKS as t (t)}<span class="num" style="left:{t}%">{t} %</span>{/each}
  </div>
  {#each items as r (r.n)}
    <div class="row">
      <span class="frac num">{r.k.toLocaleString("de-DE")} / {r.n.toLocaleString("de-DE")}</span>
      <span class="track">
        {#each TICKS as t (t)}<i style="left:{t}%"></i>{/each}
        <span class="band" style="left:{r.ci[0] * 100}%; width:{(r.ci[1] - r.ci[0]) * 100}%"></span>
        <span class="dot" style="left:{(r.k / r.n) * 100}%"></span>
      </span>
      <span class="ci num">{pct(r.ci[0], 0)} bis {pct(r.ci[1], 0)}</span>
    </div>
  {/each}
</div>

<style>
  .sample {
    display: grid;
    gap: 16px;
  }
  .axis,
  .track {
    position: relative;
  }
  .axis {
    height: 16px;
    margin: 0 118px 0 92px;
    font-size: 11.5px;
    color: var(--text-3);
  }
  .axis span {
    position: absolute;
    transform: translateX(-50%);
    white-space: nowrap;
  }
  .row {
    display: grid;
    grid-template-columns: 80px minmax(0, 1fr) 106px;
    align-items: center;
    gap: 12px;
  }
  .frac {
    font-size: 17px;
    font-weight: 600;
  }
  .track {
    height: 26px;
    border-radius: 5px;
    background: var(--bg);
  }
  .track i {
    position: absolute;
    top: 0;
    bottom: 0;
    border-left: 1px solid var(--line);
  }
  .band {
    position: absolute;
    top: 5px;
    bottom: 5px;
    border-radius: 99px;
    background: color-mix(in srgb, var(--text) 30%, transparent);
    border: 1px solid color-mix(in srgb, var(--text) 50%, transparent);
  }
  .dot {
    position: absolute;
    top: 50%;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--text);
    transform: translate(-50%, -50%);
  }
  .ci {
    color: var(--text-2);
    font-size: 13.5px;
  }
</style>
