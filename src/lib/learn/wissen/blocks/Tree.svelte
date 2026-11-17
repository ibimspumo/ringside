<script lang="ts">
  import { ev, num, pct } from "../math";

  let { root, branches, unit }: { root: string; branches: { label: string; p: number; result: number }[]; unit: string } = $props();

  const total = $derived(ev(branches));
  const sign = (x: number) => (x > 0 ? "+" : x < 0 ? "−" : "±") + num(Math.abs(x), 1);
  const tone = (x: number) => (x > 0 ? "pos" : x < 0 ? "neg" : "");
</script>

<div class="tree">
  <div class="root">{root}</div>
  <div class="branches">
    {#each branches as b (b.label)}
      <div class="br">
        <span class="lbl">{b.label}</span>
        <span class="p">
          <span class="track"><span class="fill" style="width:{b.p * 100}%"></span></span>
          <span class="num">{pct(b.p, 0)}</span>
        </span>
        <span class="num res {tone(b.result)}">{sign(b.result)}</span>
        <span class="num prod faint">= {sign(b.p * b.result)}</span>
      </div>
    {/each}
  </div>
  <div class="sum">
    <span>Erwartungswert</span>
    <b class="num {tone(total)}">{sign(total)} {unit}</b>
  </div>
</div>

<style>
  .tree {
    display: grid;
    gap: 0;
  }
  .root {
    justify-self: start;
    padding: 7px 12px;
    border-radius: var(--r-sm);
    background: var(--surface-3);
    font-size: 13.5px;
  }
  .branches {
    display: grid;
    gap: 10px;
    margin-left: 18px;
    padding: 14px 0 14px 20px;
    border-left: 1.5px solid var(--line-2);
  }
  .br {
    position: relative;
    display: grid;
    grid-template-columns: minmax(110px, 1.2fr) minmax(90px, 1fr) 56px 64px;
    align-items: center;
    gap: 12px;
  }
  .br::before {
    content: "";
    position: absolute;
    left: -20px;
    top: 50%;
    width: 14px;
    border-top: 1.5px solid var(--line-2);
  }
  .lbl {
    font-size: 13.5px;
  }
  .p {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    color: var(--text-2);
  }
  .track {
    flex: 1;
    height: 6px;
    border-radius: 99px;
    background: var(--bg);
    overflow: hidden;
  }
  .fill {
    display: block;
    height: 100%;
    background: var(--text-2);
  }
  .res {
    text-align: right;
    font-size: 16px;
    font-weight: 600;
  }
  .prod {
    text-align: right;
    font-size: 13px;
  }
  .sum {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    padding: 10px 12px;
    border-radius: var(--r-sm);
    background: var(--bg);
    color: var(--text-2);
    font-size: 13px;
  }
  .sum b {
    font-size: 24px;
    font-weight: 600;
  }
</style>
