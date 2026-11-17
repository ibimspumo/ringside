<script lang="ts">
  import type { PushFoldNode } from "../types";
  import { floating, signed, type Anchor } from "../app/format";

  /** 13×13 starting-hand grid of one push/fold node, filled by shove or call frequency. */
  let { node, action }: { node: PushFoldNode; action: "shove" | "call" } = $props();

  const RANKS = "AKQJT98765432";
  type Cell = { i: number; key: string; combos: number; freq: number; ev: number; fold: number };

  let hover = $state<{ cell: Cell; anchor: Anchor } | null>(null);

  // HandClass::index is row * 13 + col with the ace first, suited above the diagonal.
  const cells = $derived(
    Array.from({ length: 169 }, (_, i): Cell => {
      const r = Math.floor(i / 13);
      const c = i % 13;
      const hi = RANKS[Math.min(r, c)];
      const lo = RANKS[Math.max(r, c)];
      const key = r === c ? hi + lo : r < c ? `${hi}${lo}s` : `${hi}${lo}o`;
      return {
        i,
        key,
        combos: r === c ? 6 : r < c ? 4 : 12,
        freq: node.freq[i] / 255,
        ev: node.ev_act[i] / 100,
        fold: node.ev_fold / 100,
      };
    }),
  );

  const verb = $derived(action === "shove" ? "Shove" : "Call");
  const tone = $derived(action === "shove" ? "var(--shove)" : "var(--call)");

  function pct(f: number): string {
    return `${Math.round(f * 100)} %`;
  }

  /** Whole percent; 0 and 100 count as pure. */
  const share = (c: Cell) => Math.round(c.freq * 100);

  /** Fills from the bottom by frequency, so mixed hands read as partly played. */
  function fill(c: Cell): string {
    if (share(c) <= 0) return "var(--surface-2)";
    if (share(c) >= 100) return tone;
    const p = (c.freq * 100).toFixed(1);
    return `linear-gradient(to top, ${tone} ${p}%, var(--surface-2) ${p}%)`;
  }

  function show(e: MouseEvent, cell: Cell) {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    hover = { cell, anchor: { left: r.left, top: r.top, right: r.right, bottom: r.bottom } };
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="matrix" onmouseleave={() => (hover = null)}>
  {#each cells as c (c.i)}
    {@const mixed = share(c) > 0 && share(c) < 100}
    <div
      class="cell"
      class:on={c.freq >= 0.8}
      class:low-on={c.freq >= 0.25}
      style="background:{fill(c)}"
      aria-label="{c.key}: {verb} {pct(c.freq)}"
      onmouseenter={(e) => show(e, c)}
    >
      <span class="k">{c.key}</span>
      {#if mixed}<span class="v num">{share(c)}</span>{/if}
    </div>
  {/each}
  {#if hover}
    {@const c = hover.cell}
    <div class="tip" use:floating={{ anchor: hover.anchor }}>
      <header><b>{c.key}</b> · {c.combos} Kombos</header>
      <div class="row"><span>{verb}</span><b>{pct(c.freq)}</b></div>
      <div class="row"><span>EV {verb}</span><b>{signed(c.ev, 2)} BB</b></div>
      <div class="row"><span>EV Fold</span><b>{signed(c.fold, 2)} BB</b></div>
      <div class="row">
        <span>Differenz</span><b class={c.ev - c.fold > 0.005 ? "pos" : c.ev - c.fold < -0.005 ? "neg" : ""}
          >{signed(c.ev - c.fold, 2)} BB</b
        >
      </div>
    </div>
  {/if}
</div>

<style>
  .matrix {
    --shove: color-mix(in srgb, var(--neg) 78%, var(--surface-2));
    --call: color-mix(in srgb, var(--pos) 70%, var(--surface-2));
    position: relative;
    display: grid;
    grid-template-columns: repeat(13, minmax(0, 1fr));
    gap: 2px;
  }
  .cell {
    position: relative;
    aspect-ratio: 1;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    padding: 3px 4px;
    border-radius: 3px;
    overflow: hidden;
    cursor: default;
    min-width: 0;
    color: var(--text-3);
  }
  /* Ink follows the fill: the name sits at the top, the share at the bottom. */
  .cell.on .k,
  .cell.low-on .v {
    color: #121418;
  }
  .cell:hover {
    outline: 1px solid var(--text);
    z-index: 1;
  }
  .k {
    font-family: var(--font-num);
    font-size: 11px;
    font-weight: 600;
    line-height: 1;
  }
  .v {
    align-self: flex-end;
    font-size: 10.5px;
    line-height: 1;
  }
  @container (max-width: 520px) {
    .cell {
      padding: 2px;
    }
    .k {
      font-size: 9px;
    }
    .v {
      display: none;
    }
  }
</style>
