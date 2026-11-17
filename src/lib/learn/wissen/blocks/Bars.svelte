<script lang="ts">
  import type { BarRow } from "../articles";

  let { rows, max, schematic = false }: { rows: BarRow[]; max?: number; schematic?: boolean } = $props();

  const top = $derived(max ?? Math.max(...rows.map((r) => r.value)));
  const TONES: Record<string, string> = { pos: "var(--pos)", neg: "var(--neg)", amber: "var(--amber)", base: "var(--text)" };
</script>

<div class="bars">
  {#if schematic}<span class="schema">Schema, keine echten Werte</span>{/if}
  {#each rows as r (r.label)}
    <div class="row">
      <span class="lbl">{r.label}</span>
      <span class="track"><span class="fill" style="width:{(r.value / top) * 100}%; --c:{TONES[r.tone ?? 'base']}"></span></span>
      {#if !schematic}<b class="num" style="color:{r.tone ? TONES[r.tone] : 'var(--text)'}">{r.display}</b>{/if}
    </div>
  {/each}
</div>

<style>
  .bars {
    display: grid;
    gap: 14px;
  }
  .schema {
    justify-self: start;
    padding: 1px 8px;
    border-radius: 99px;
    border: 1px dashed var(--line-2);
    color: var(--text-2);
    font-size: 12px;
  }
  .row {
    display: grid;
    grid-template-columns: minmax(110px, 30%) minmax(0, 1fr) auto;
    align-items: center;
    gap: 14px;
  }
  .lbl {
    color: var(--text-2);
    font-size: 13.5px;
  }
  .track {
    height: 22px;
    border-radius: 5px;
    background: var(--bg);
    overflow: hidden;
  }
  .fill {
    display: block;
    height: 100%;
    border-radius: 5px;
    background: color-mix(in srgb, var(--c) 82%, transparent);
  }
  b {
    min-width: 64px;
    text-align: right;
    font-size: 20px;
    font-weight: 600;
  }
</style>
