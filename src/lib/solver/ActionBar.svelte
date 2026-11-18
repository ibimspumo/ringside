<script lang="ts">
  import { bb } from "./actions";
  import { actionParts } from "./play";
  import type { SolverNode } from "./types";

  /**
   * Every action of the player to act as one strip: each segment is as wide as the action is
   * frequent, so the strategy itself is what you click. Keys 1–9 play the actions in order.
   * With `blind` the frequencies stay hidden and all segments are equal (practice).
   */
  let {
    node,
    colors,
    onAct,
    blind = false,
    disabled = false,
  }: {
    node: SolverNode;
    colors: string[];
    onAct: (k: number) => void;
    blind?: boolean;
    disabled?: boolean;
  } = $props();

  const parts = $derived(node.actions.map((a) => actionParts(a, node)));
  const shown = $derived(node.actions.map((a) => a.frequency));
  // Rare actions keep a clickable width; the rest shares the strip by frequency.
  const grow = $derived(shown.map((f) => (blind ? 1 : 0.25 + f)));
  const pct = (x: number) => `${Math.round(x * 100)} %`;
</script>

<div class="strip" role="group" aria-label="Aktionen">
  {#each node.actions as a, k (k)}
    <button
      class="seg"
      class:blind
      style="flex-grow:{grow[k]}; --c:{colors[k]}"
      {disabled}
      onclick={() => onAct(k)}
      title={blind ? undefined : `${bb(a.combos, 1)} Kombos der Range`}
      aria-keyshortcuts={k < 9 ? String(k + 1) : undefined}
      aria-label="{parts[k].label}{parts[k].chips ? `, ${parts[k].chips}` : ''}{blind ? '' : `, ${pct(shown[k])}`}"
    >
      <span class="key num" aria-hidden="true">{k + 1}</span>
      <span class="what">{parts[k].label}</span>
      <span class="chips num">{parts[k].chips || " "}</span>
      {#if !blind}
        <b class="freq num">{pct(shown[k])}</b>
        <i class="fill" style="width:{Math.max(0, Math.min(1, shown[k])) * 100}%"></i>
      {/if}
    </button>
  {/each}
</div>

<style>
  .strip {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    min-width: 0;
  }
  .seg {
    position: relative;
    flex: 1 1 0;
    min-width: 124px;
    display: grid;
    grid-template-columns: 1fr auto;
    grid-template-areas:
      "what what"
      "chips freq";
    align-items: baseline;
    gap: 0 8px;
    padding: 9px 10px 11px;
    border: 1px solid var(--line-2);
    border-radius: var(--r-sm);
    background: color-mix(in srgb, var(--c) 14%, var(--surface-2));
    text-align: left;
    cursor: pointer;
    overflow: hidden;
    transition:
      background 0.12s,
      border-color 0.12s;
  }
  .seg::before {
    content: "";
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    width: 3px;
    background: var(--c);
  }
  .seg:hover:not(:disabled) {
    background: color-mix(in srgb, var(--c) 26%, var(--surface-2));
    border-color: color-mix(in srgb, var(--c) 60%, var(--line-2));
  }
  .seg:disabled {
    cursor: default;
    opacity: 0.55;
  }
  .seg.blind {
    background: var(--surface-2);
  }
  .key {
    position: absolute;
    right: 5px;
    bottom: 3px;
    font-size: 9.5px;
    color: var(--text-3);
  }
  .what {
    grid-area: what;
    font-size: var(--fs-md);
    font-weight: var(--fw-semibold);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .chips {
    grid-area: chips;
    font-size: var(--fs-xs);
    color: var(--text-2);
    white-space: nowrap;
  }
  .freq {
    grid-area: freq;
    font-size: var(--fs-lg);
    font-weight: var(--fw-bold);
    line-height: 1;
    justify-self: end;
  }
  .fill {
    position: absolute;
    left: 0;
    bottom: 0;
    height: 3px;
    background: var(--c);
  }
</style>
