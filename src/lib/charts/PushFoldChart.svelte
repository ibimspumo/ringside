<script lang="ts">
  import type { PushFoldNode } from "../types";
  import { signed } from "../app/format";
  import RangeGrid, { type RangeCell } from "../ui/RangeGrid.svelte";
  import Tooltip from "../ui/Tooltip.svelte";
  import { classCombos, classLabel } from "../solver/range";
  import { TERM, amount } from "./words";
  import { actionColor } from "./preflop-labels";

  /** 13×13 grid of one push/fold node on the shared RangeGrid: each hand split into all-in (or call) and fold. */
  let {
    node,
    action,
    pin = null,
    only = null,
  }: {
    node: PushFoldNode;
    action: "shove" | "call";
    /** Hand to show pinned and outlined, e.g. the one played in a hand history. */
    pin?: string | null;
    /** From the action bar: 0 shows only hands that shove or call, 1 only hands that fold. */
    only?: number | null;
  } = $props();

  type Hand = { key: string; combos: number; freq: number; ev: number; fold: number };

  // HandClass::index is row * 13 + col with the ace first, suited above the diagonal.
  const hands = $derived(
    Array.from({ length: 169 }, (_, i): Hand => ({
      key: classLabel(i),
      combos: classCombos(i),
      freq: node.freq[i] / 255,
      ev: node.ev_act[i] / 100,
      fold: node.ev_fold / 100,
    })),
  );

  const verb = $derived(action === "shove" ? "All-in" : "Call");
  const doing = $derived(action === "shove" ? "all-in" : "callen");
  const tone = $derived(actionColor(action === "shove" ? "all_in" : "call"));
  const FOLD = actionColor("fold");

  const pct = (f: number) => `${Math.round(f * 100)} %`;
  /** Whole percent; 0 and 100 count as pure. */
  const share = (h: Hand) => Math.round(h.freq * 100);

  const cells = $derived(
    hands.map(
      (h): RangeCell => ({
        mix: [
          { color: tone, share: h.freq },
          { color: FOLD, share: 1 - h.freq },
        ],
        value: share(h) > 0 && share(h) < 100 ? String(share(h)) : undefined,
        note: `${verb} ${pct(h.freq)}`,
      }),
    ),
  );

  const pinIndex = $derived(pin ? hands.findIndex((h) => h.key === pin) : -1);
  let selected = $state(-1);
  let box = $state<HTMLDivElement>();
  // A hand handed over is pinned and scrolled into view, so its card sits beside it.
  $effect(() => {
    if (pinIndex < 0) return;
    selected = pinIndex;
    const t = setTimeout(() => box?.querySelector(`[data-i="${pinIndex}"]`)?.scrollIntoView({ block: "center" }), 150);
    return () => clearTimeout(t);
  });

  function verdict(h: Hand): string {
    if (share(h) >= 100) return `Immer ${doing}`;
    if (share(h) <= 0) return "Immer folden";
    return `Gemischt: ${share(h)} % ${doing}, sonst Fold`;
  }

  function why(h: Hand): string {
    const d = h.ev - h.fold;
    if (Math.abs(d) < 0.005) return `${verb} und Fold bringen gleich viel.`;
    return d > 0 ? `${verb} bringt im Schnitt ${amount(d)} BB mehr als Fold.` : `${verb} verliert im Schnitt ${amount(d)} BB gegenüber Fold.`;
  }
  const diffTone = (h: Hand) => (h.ev - h.fold > 0.005 ? "pos" : h.ev - h.fold < -0.005 ? "neg" : "");
</script>

<div bind:this={box}>
<RangeGrid
  {cells}
  label="Push/Fold-Range als 13 mal 13 Raster"
  hero={pinIndex}
  {selected}
  dimmed={only === null ? undefined : (i) => (only === 0 ? hands[i].freq < 0.005 : hands[i].freq > 0.995)}
  onSelect={(i) => (selected = selected === i ? -1 : i)}
  onClose={() => (selected = -1)}
>
  {#snippet tip(i)}
    {@const h = hands[i]}
    <p class="t-head"><b class="t-key">{h.key}</b> {verdict(h)}</p>
    <div class="bar" aria-hidden="true"><i style="width:{h.freq * 100}%;background:{tone}"></i></div>
    <p class="t-why">{why(h).replace(/ im Schnitt/, "")}</p>
  {/snippet}
  {#snippet card(i)}
    {@const h = hands[i]}
    <header class="c-head">
      <b class="c-key">{h.key}</b>
      <Tooltip text={TERM.combos}><span class="term">{h.combos} Kombos</span></Tooltip>
    </header>
    <p class="verdict">{verdict(h)}.</p>
    <p class="why">{why(h)}</p>
    {#if share(h) > 0 && share(h) < 100}
      <p class="why"><Tooltip text={TERM.mixed}><span class="term">Warum gemischt?</span></Tooltip></p>
    {/if}
    <div class="bar split" aria-hidden="true"><i style="width:{h.freq * 100}%;background:{tone}"></i></div>
    <table class="acts">
      <thead><tr><th>Aktion</th><th>Wie oft</th><th><Tooltip text={TERM.ev}><span class="term">EV</span></Tooltip></th></tr></thead>
      <tbody>
        <tr class:best={h.ev >= h.fold}>
          <td><i class="sw" style="background:{tone}"></i>{verb}</td>
          <td class="num">{pct(h.freq)}</td>
          <td class="num">{signed(h.ev, 2)} BB</td>
        </tr>
        <tr class:best={h.fold > h.ev}>
          <td><i class="sw" style="background:{FOLD}"></i>Fold</td>
          <td class="num">{pct(1 - h.freq)}</td>
          <td class="num">{signed(h.fold, 2)} BB</td>
        </tr>
      </tbody>
      <tfoot>
        <tr><td colspan="2">Unterschied</td><td class="num diff {diffTone(h)}">{signed(h.ev - h.fold, 2)} BB</td></tr>
      </tfoot>
    </table>
  {/snippet}
</RangeGrid>
</div>

<style>
  .t-head {
    margin: 0;
    display: flex;
    align-items: baseline;
    gap: var(--sp-2);
  }
  .t-key {
    font-family: var(--font-num);
    font-size: var(--fs-base);
  }
  .c-key {
    font-family: var(--font-num);
    font-size: var(--fs-3xl);
    line-height: 1;
  }
  .t-why {
    margin: 0;
    color: var(--text-2);
  }
  .c-head {
    display: flex;
    align-items: baseline;
    gap: var(--sp-3);
    margin-bottom: var(--sp-2);
  }
  .term {
    color: var(--text-2);
    font-size: var(--fs-sm);
    text-decoration: underline dotted color-mix(in srgb, currentColor 50%, transparent);
    text-underline-offset: 3px;
  }
  .bar {
    display: flex;
    height: 5px;
    margin: var(--sp-2) 0 var(--sp-1);
    border-radius: 3px;
    background: var(--act-fold);
    overflow: hidden;
  }
  .bar.split {
    height: 8px;
    margin: var(--sp-3) 0;
  }
  .verdict {
    margin: 0;
    font-size: var(--fs-base);
    font-weight: 600;
  }
  .why {
    margin: 2px 0 0;
    color: var(--text-2);
  }
  .acts {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--fs-sm);
  }
  .acts th {
    font-weight: 500;
    color: var(--text-2);
    text-align: right;
    padding: 0 0 var(--sp-1);
  }
  .acts td {
    padding: var(--sp-1) 0;
    text-align: right;
    border-top: 1px solid var(--line);
  }
  .acts th:first-child,
  .acts td:first-child {
    text-align: left;
  }
  .acts tr.best td {
    font-weight: 600;
  }
  .acts tfoot td {
    color: var(--text-2);
  }
  .acts tfoot td.diff {
    font-weight: 600;
  }
  .sw {
    display: inline-block;
    width: 9px;
    height: 9px;
    border-radius: 2px;
    margin-right: 7px;
  }
</style>
