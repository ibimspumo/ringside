<script lang="ts">
  import { pct } from "../math";

  let { legend, rows }: { legend: [string, string]; rows: { label: string; a: number; b: number }[] } = $props();

  const top = $derived(Math.max(...rows.flatMap((r) => [r.a, r.b])));
</script>

<div class="cmp">
  <div class="legend">
    <span><i class="a"></i>{legend[0]}</span>
    <span><i class="b"></i>{legend[1]}</span>
  </div>
  <div class="cols">
    {#each rows as r (r.label)}
      <div class="col">
        <div class="pair">
          <div class="bar a" style="height:{(r.a / top) * 100}%"><b class="num">{pct(r.a)}</b></div>
          <div class="bar b" style="height:{(r.b / top) * 100}%"><b class="num">{pct(r.b)}</b></div>
        </div>
        <span class="lbl">{r.label}</span>
      </div>
    {/each}
  </div>
</div>

<style>
  .cmp {
    display: grid;
    gap: 14px;
  }
  .legend {
    display: flex;
    gap: 18px;
    font-size: 13px;
    color: var(--text-2);
  }
  .legend i {
    display: inline-block;
    width: 10px;
    height: 10px;
    margin-right: 7px;
    border-radius: 3px;
  }
  .cols {
    display: grid;
    grid-auto-flow: column;
    grid-auto-columns: minmax(0, 1fr);
    gap: 20px;
  }
  .col {
    display: grid;
    gap: 8px;
    justify-items: center;
  }
  .pair {
    display: flex;
    align-items: flex-end;
    gap: 6px;
    height: 180px;
    padding-top: 24px;
    width: 100%;
    justify-content: center;
  }
  .bar {
    position: relative;
    width: min(56px, 42%);
    border-radius: 5px 5px 2px 2px;
  }
  .bar b {
    position: absolute;
    bottom: calc(100% + 4px);
    left: 50%;
    transform: translateX(-50%);
    font-size: 14px;
    font-weight: 600;
    white-space: nowrap;
  }
  .a,
  .legend .a {
    background: var(--surface-3);
  }
  .b,
  .legend .b {
    background: var(--text);
  }
  .lbl {
    color: var(--text-2);
    font-size: 13px;
  }
</style>
