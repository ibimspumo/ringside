<script lang="ts">
  import { floating, signed, type Anchor } from "../app/format";
  import type { PreflopAction, PreflopNode, PreflopStep } from "./preflop-types";
  import { actionColor, actionName } from "./preflop-labels";

  /** 13×13 starting-hand grid of one preflop decision; each cell is split by action share. */
  let { node, actions, line }: { node: PreflopNode; actions: PreflopAction[]; line: PreflopStep[] } = $props();

  const RANKS = "AKQJT98765432";
  type Part = { a: number; freq: number; ev: number | null };
  /** `inRange` false: the hand left the range earlier, all its frequencies are 0. */
  type Cell = { i: number; key: string; combos: number; parts: Part[]; inRange: boolean };

  let hover = $state<{ cell: Cell; anchor: Anchor } | null>(null);

  // HandClass::index is row * 13 + col with the ace first, suited above the diagonal.
  const cells = $derived(
    Array.from({ length: 169 }, (_, i): Cell => {
      const r = Math.floor(i / 13);
      const c = i % 13;
      const hi = RANKS[Math.min(r, c)];
      const lo = RANKS[Math.max(r, c)];
      const key = r === c ? hi + lo : r < c ? `${hi}${lo}s` : `${hi}${lo}o`;
      const parts = actions.map((_, a) => ({
        a,
        freq: (node.freq[a]?.[i] ?? 0) / 255,
        ev: node.ev[a] ? node.ev[a][i] / 100 : null,
      }));
      return { i, key, combos: r === c ? 6 : r < c ? 4 : 12, parts, inRange: parts.some((p) => p.freq > 0) };
    }),
  );

  /** Played actions stack from the left, fold stays as the empty rest. */
  function fill(c: Cell): string {
    const stops: string[] = [];
    let x = 0;
    for (const p of c.parts) {
      if (actions[p.a].kind === "fold" || p.freq < 0.005) continue;
      const end = Math.min(100, x + p.freq * 100);
      stops.push(`${actionColor(actions[p.a].kind)} ${x.toFixed(1)}% ${end.toFixed(1)}%`);
      x = end;
    }
    if (!stops.length) return "var(--surface-2)";
    stops.push(`var(--surface-2) ${x.toFixed(1)}% 100%`);
    return `linear-gradient(to right, ${stops.join(", ")})`;
  }

  /** Share of the cell that is not folded: decides the ink colour. */
  const played = (c: Cell) => c.parts.filter((p) => actions[p.a].kind !== "fold").reduce((s, p) => s + p.freq, 0);

  /** The action played most, unless the hand is mixed: then nothing is printed. */
  function mixedShare(c: Cell): number | null {
    const top = Math.max(...c.parts.map((p) => p.freq));
    return top > 0.995 ? null : Math.round(top * 100);
  }

  function pct(f: number): string {
    return `${Math.round(f * 100)} %`;
  }

  function show(e: MouseEvent, cell: Cell) {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    hover = { cell, anchor: { left: r.left, top: r.top, right: r.right, bottom: r.bottom } };
  }

  const bestEv = (c: Cell) => Math.max(...c.parts.filter((p) => p.ev !== null).map((p) => p.ev as number));
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="matrix" onmouseleave={() => (hover = null)}>
  {#each cells as c (c.i)}
    {@const share = c.inRange ? mixedShare(c) : null}
    <div
      class="cell"
      class:on={played(c) >= 0.6}
      class:out={!c.inRange}
      style={c.inRange ? `background:${fill(c)}` : undefined}
      aria-label={c.inRange
        ? `${c.key}: ${c.parts.map((p) => `${actionName(actions[p.a], line)} ${pct(p.freq)}`).join(", ")}`
        : `${c.key}: nicht in der Range`}
      onmouseenter={(e) => show(e, c)}
    >
      <span class="k">{c.key}</span>
      {#if share !== null}<span class="v num">{share}</span>{/if}
    </div>
  {/each}
  {#if hover}
    {@const c = hover.cell}
    {@const best = bestEv(c)}
    <div class="tip" use:floating={{ anchor: hover.anchor }}>
      <header><b>{c.key}</b> · {c.combos} Kombos</header>
      {#if !c.inRange}
        <p class="out-note">Nicht in der Range: Diese Hand hat vorher anders gespielt und kommt hier nicht an.</p>
      {:else}
      <table>
        <thead><tr><th></th><th>Anteil</th><th>EV</th></tr></thead>
        <tbody>
          {#each c.parts as p (p.a)}
            <tr class:best={p.ev !== null && Math.abs(p.ev - best) < 0.005}>
              <td><i class="sw" style="background:{actionColor(actions[p.a].kind)}"></i>{actionName(actions[p.a], line)}</td>
              <td class="num">{pct(p.freq)}</td>
              <td class="num">{p.ev === null ? "–" : `${signed(p.ev, 2)} BB`}</td>
            </tr>
          {/each}
        </tbody>
      </table>
      <footer>EV im Modell, gemessen am Stack vor der Hand.</footer>
      {/if}
    </div>
  {/if}
</div>

<style>
  .matrix {
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
  .cell.on {
    color: #121418;
  }
  .cell.out {
    background: transparent;
    box-shadow: inset 0 0 0 1px var(--line);
    color: var(--text-3);
    opacity: 0.55;
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
  .tip table {
    border-collapse: collapse;
    font-size: 12.5px;
    margin-top: 6px;
  }
  .tip th {
    font-weight: 500;
    color: var(--text-3);
    text-align: right;
    padding: 0 0 2px 12px;
  }
  .tip td {
    padding: 2px 0 2px 12px;
    text-align: right;
  }
  .tip td:first-child {
    text-align: left;
    padding-left: 0;
    white-space: nowrap;
  }
  .tip tr.best td {
    font-weight: 600;
  }
  .out-note {
    margin: 6px 0 0;
    max-width: 220px;
    font-size: 12px;
    color: var(--text-2);
  }
  .tip footer {
    margin-top: 6px;
    font-size: 11.5px;
    color: var(--text-3);
  }
  .sw {
    display: inline-block;
    width: 9px;
    height: 9px;
    border-radius: 2px;
    margin-right: 6px;
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
