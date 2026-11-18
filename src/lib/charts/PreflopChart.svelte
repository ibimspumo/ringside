<script lang="ts">
  import { signed } from "../app/format";
  import type { PreflopAction, PreflopNode, PreflopStep } from "./preflop-types";
  import { actionColor, actionName, actionPhrase } from "./preflop-labels";
  import HandMatrix, { type MatrixCell } from "./HandMatrix.svelte";
  import Hint from "./Hint.svelte";
  import { TERM, amount } from "./words";

  /** 13×13 starting-hand grid of one preflop decision; each cell is split by action share. */
  let {
    node,
    actions,
    line,
    pin = null,
    only = null,
  }: {
    node: PreflopNode;
    actions: PreflopAction[];
    line: PreflopStep[];
    pin?: string | null;
    /** From the action bar: show only hands that take this action. */
    only?: number | null;
  } = $props();

  const RANKS = "AKQJT98765432";
  type Part = { a: number; freq: number; ev: number | null };
  /** `inRange` false: the hand left the range earlier, all its frequencies are 0. */
  type Hand = { key: string; combos: number; parts: Part[]; inRange: boolean };

  // HandClass::index is row * 13 + col with the ace first, suited above the diagonal.
  const hands = $derived(
    Array.from({ length: 169 }, (_, i): Hand => {
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
      return { key, combos: r === c ? 6 : r < c ? 4 : 12, parts, inRange: parts.some((p) => p.freq > 0) };
    }),
  );

  /** Played actions stack from the left, fold stays as the empty rest. */
  function fill(h: Hand): string {
    const stops: string[] = [];
    let x = 0;
    for (const p of h.parts) {
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
  const played = (h: Hand) => h.parts.filter((p) => actions[p.a].kind !== "fold").reduce((s, p) => s + p.freq, 0);

  /** The action played most, unless the hand is pure: then nothing is printed. */
  function mixedShare(h: Hand): number | null {
    const top = Math.max(...h.parts.map((p) => p.freq));
    return top > 0.995 ? null : Math.round(top * 100);
  }

  const pct = (f: number) => `${Math.round(f * 100)} %`;

  const cells = $derived(
    hands.map(
      (h): MatrixCell => ({
        key: h.key,
        combos: h.combos,
        bg: fill(h),
        filled: played(h) >= 0.005,
        badge: h.inRange ? mixedShare(h) : null,
        dim: only !== null && (h.parts[only]?.freq ?? 0) < 0.005,
        out: !h.inRange,
        label: h.inRange
          ? `${h.key}: ${h.parts.map((p) => `${actionName(actions[p.a], line)} ${pct(p.freq)}`).join(", ")}`
          : `${h.key}: nicht in der Range`,
      }),
    ),
  );

  const bestEv = (h: Hand) => Math.max(...h.parts.filter((p) => p.ev !== null).map((p) => p.ev as number));
  /** Actions this hand takes, most often first. */
  const taken = (h: Hand) => h.parts.filter((p) => p.freq >= 0.005).sort((a, b) => b.freq - a.freq);

  function verdict(h: Hand): string {
    if (!h.inRange) return "Im Modell schon vorher gefoldet";
    const t = taken(h);
    if (t.length === 1) return `Immer ${actionPhrase(actions[t[0].a], line)}`;
    return `Gemischt: ${t.map((p) => `${pct(p.freq)} ${actionPhrase(actions[p.a], line)}`).join(", ")}`;
  }

  /** Why the pure choice is right (its EV lead over the next best action), or why mixing is fine. */
  function why(h: Hand): string {
    const t = taken(h);
    if (!h.inRange || !t.length) return "";
    if (t.length > 1) return "Die gespielten Aktionen bringen fast gleich viel.";
    const own = h.parts[t[0].a].ev;
    const next = h.parts.filter((p) => p.a !== t[0].a && p.ev !== null).sort((x, y) => (y.ev as number) - (x.ev as number))[0];
    if (own === null || !next) return "";
    return `Bringt ${amount(own - (next.ev as number))} BB mehr als ${actionPhrase(actions[next.a], line)}.`;
  }
</script>

<HandMatrix {cells} {pin}>
  {#snippet tip(i)}
    {@const h = hands[i]}
    <div class="t-head"><b class="t-key">{h.key}</b></div>
    <p class="t-verdict">{verdict(h)}</p>
    {#if h.inRange}
      <div class="bar" aria-hidden="true" style="background:{cells[i].bg}"></div>
      {#if why(h)}<p class="t-why">{why(h)}</p>{/if}
    {/if}
  {/snippet}
  {#snippet card(i)}
    {@const h = hands[i]}
    {@const best = bestEv(h)}
    <p class="verdict">{verdict(h)}.</p>
    {#if !h.inRange}
      <p class="why">Diese Hand hat vorher anders gespielt (meist gefoldet) und erreicht diese Situation nie.</p>
    {:else}
      {#if why(h)}<p class="why">{why(h)}{#if taken(h).length > 1}{" "}<Hint text={TERM.mixed}>Warum gemischt?</Hint>{/if}</p>{/if}
      <div class="split" aria-hidden="true" style="background:{cells[i].bg}"></div>
      <table class="acts">
        <thead><tr><th>Aktion</th><th>Wie oft</th><th><Hint text={TERM.ev}>EV</Hint></th></tr></thead>
        <tbody>
          {#each h.parts as p (p.a)}
            <tr class:best={p.ev !== null && Math.abs(p.ev - best) < 0.005}>
              <td><i class="sw" style="background:{actionColor(actions[p.a].kind)}"></i>{actionName(actions[p.a], line)}</td>
              <td class="num">{pct(p.freq)}</td>
              <td class="num">{p.ev === null ? "–" : `${signed(p.ev, 2)} BB`}</td>
            </tr>
          {/each}
        </tbody>
      </table>
      <p class="foot">Fett: die Aktion mit dem höchsten EV. EV im Modell, gemessen am Stack vor der Hand.</p>
    {/if}
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
  .t-verdict {
    margin: 2px 0 0;
  }
  .t-why {
    margin: 0;
    color: var(--text-2);
  }
  .bar,
  .split {
    height: 5px;
    margin: 6px 0 5px;
    border-radius: 3px;
    box-shadow: inset 0 0 0 1px var(--line);
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
    white-space: nowrap;
  }
  .acts tr.best td {
    font-weight: 600;
  }
  .foot {
    margin: 8px 0 0;
    font-size: 11.5px;
    color: var(--text-2);
  }
  .sw {
    display: inline-block;
    width: 9px;
    height: 9px;
    border-radius: 2px;
    margin-right: 7px;
    box-shadow: inset 0 0 0 1px var(--line-2);
  }
</style>
