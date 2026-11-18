<script lang="ts">
  import type { PushFoldNode } from "../types";
  import { signed } from "../app/format";
  import HandMatrix, { type MatrixCell } from "./HandMatrix.svelte";
  import Hint from "./Hint.svelte";
  import { TERM, amount } from "./words";
  import { actionColor } from "./preflop-labels";

  /** 13×13 starting-hand grid of one push/fold node, filled by shove or call frequency. */
  let {
    node,
    action,
    pin = null,
    only = null,
  }: {
    node: PushFoldNode;
    action: "shove" | "call";
    pin?: string | null;
    /** From the action bar: 0 shows only hands that shove or call, 1 only hands that fold. */
    only?: number | null;
  } = $props();

  const RANKS = "AKQJT98765432";
  type Hand = { key: string; combos: number; freq: number; ev: number; fold: number };

  // HandClass::index is row * 13 + col with the ace first, suited above the diagonal.
  const hands = $derived(
    Array.from({ length: 169 }, (_, i): Hand => {
      const r = Math.floor(i / 13);
      const c = i % 13;
      const hi = RANKS[Math.min(r, c)];
      const lo = RANKS[Math.max(r, c)];
      const key = r === c ? hi + lo : r < c ? `${hi}${lo}s` : `${hi}${lo}o`;
      return { key, combos: r === c ? 6 : r < c ? 4 : 12, freq: node.freq[i] / 255, ev: node.ev_act[i] / 100, fold: node.ev_fold / 100 };
    }),
  );

  const verb = $derived(action === "shove" ? "All-in" : "Call");
  const doing = $derived(action === "shove" ? "all-in" : "callen");
  const tone = $derived(actionColor(action === "shove" ? "all_in" : "call"));

  const pct = (f: number) => `${Math.round(f * 100)} %`;
  /** Whole percent; 0 and 100 count as pure. */
  const share = (h: Hand) => Math.round(h.freq * 100);

  /** Played share from the left, fold stays as the empty rest – the same as the preflop grid. */
  function fill(h: Hand): string {
    if (share(h) <= 0) return "var(--surface-2)";
    if (share(h) >= 100) return tone;
    const p = (h.freq * 100).toFixed(1);
    return `linear-gradient(to right, ${tone} ${p}%, var(--surface-2) ${p}%)`;
  }

  const cells = $derived(
    hands.map(
      (h): MatrixCell => ({
        key: h.key,
        combos: h.combos,
        bg: fill(h),
        filled: share(h) > 0,
        badge: share(h) > 0 && share(h) < 100 ? share(h) : null,
        dim: only === 0 ? h.freq < 0.005 : only === 1 ? h.freq > 0.995 : false,
        label: `${h.key}: ${verb} ${pct(h.freq)}`,
      }),
    ),
  );

  /** The answer for one hand in a few words. */
  function verdict(h: Hand): string {
    if (share(h) >= 100) return `Immer ${doing}`;
    if (share(h) <= 0) return "Immer folden";
    return `Gemischt: ${share(h)} % ${doing}, sonst Fold`;
  }

  function why(h: Hand): string {
    const d = h.ev - h.fold;
    if (Math.abs(d) < 0.005) return `${verb} und Fold bringen gleich viel.`;
    return d > 0
      ? `${verb} bringt im Schnitt ${amount(d)} BB mehr als Fold.`
      : `${verb} verliert im Schnitt ${amount(d)} BB gegenüber Fold.`;
  }
  const diffTone = (h: Hand) => (h.ev - h.fold > 0.005 ? "pos" : h.ev - h.fold < -0.005 ? "neg" : "");
</script>

<HandMatrix {cells} {pin}>
    {#snippet tip(i)}
      {@const h = hands[i]}
      <div class="t-head"><b class="t-key">{h.key}</b><span>{verdict(h)}</span></div>
      <div class="bar" aria-hidden="true"><i style="width:{h.freq * 100}%;background:{tone}"></i></div>
      <div class="row"><span>{why(h).replace(/ im Schnitt/, "")}</span></div>
    {/snippet}
    {#snippet card(i)}
      {@const h = hands[i]}
      <p class="verdict">{verdict(h)}.</p>
      <p class="why">{why(h)}</p>
      {#if share(h) > 0 && share(h) < 100}
        <p class="why"><Hint text={TERM.mixed}>Warum gemischt?</Hint></p>
      {/if}
      <div class="split" aria-hidden="true">
        <i style="width:{h.freq * 100}%;background:{tone}"></i>
      </div>
      <table class="acts">
        <thead><tr><th>Aktion</th><th>Wie oft</th><th><Hint text={TERM.ev}>EV</Hint></th></tr></thead>
        <tbody>
          <tr class:best={h.ev >= h.fold}>
            <td><i class="sw" style="background:{tone}"></i>{verb}</td>
            <td class="num">{pct(h.freq)}</td>
            <td class="num">{signed(h.ev, 2)} BB</td>
          </tr>
          <tr class:best={h.fold > h.ev}>
            <td><i class="sw fold"></i>Fold</td>
            <td class="num">{pct(1 - h.freq)}</td>
            <td class="num">{signed(h.fold, 2)} BB</td>
          </tr>
        </tbody>
        <tfoot>
          <tr><td colspan="2">Unterschied</td><td class="num {diffTone(h)}">{signed(h.ev - h.fold, 2)} BB</td></tr>
        </tfoot>
      </table>
    {/snippet}
</HandMatrix>

<style>
  .t-head {
    display: flex;
    align-items: baseline;
    gap: 8px;
  }
  .t-key {
    font-family: var(--font-num);
    font-size: 15px;
  }
  .bar,
  .split {
    display: flex;
    height: 5px;
    margin: 6px 0 5px;
    border-radius: 3px;
    background: var(--surface);
    overflow: hidden;
  }
  .split {
    height: 8px;
    margin: 10px 0 12px;
  }
  .verdict {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
  }
  .why {
    margin: 2px 0 0;
    color: var(--text-2);
  }
  .acts {
    width: 100%;
    border-collapse: collapse;
    font-size: 12.5px;
  }
  .acts th {
    font-weight: 500;
    color: var(--text-2);
    text-align: right;
    padding: 0 0 4px;
  }
  .acts td {
    padding: 4px 0;
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
  .acts tfoot td.num {
    font-weight: 600;
  }
  .sw {
    display: inline-block;
    width: 9px;
    height: 9px;
    border-radius: 2px;
    margin-right: 7px;
  }
  .sw.fold {
    background: var(--surface);
    box-shadow: inset 0 0 0 1px var(--line-2);
  }
</style>
