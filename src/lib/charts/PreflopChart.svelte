<script lang="ts">
  import { signed } from "../app/format";
  import RangeGrid, { type RangeCell } from "../ui/RangeGrid.svelte";
  import Tooltip from "../ui/Tooltip.svelte";
  import { classCombos, classLabel } from "../solver/range";
  import type { PreflopAction, PreflopNode, PreflopStep } from "./preflop-types";
  import { actionColor, actionName, actionPhrase } from "./preflop-labels";
  import { TERM, amount } from "./words";

  /** 13×13 grid of one preflop decision on the shared RangeGrid; each hand split by action share. */
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
    /** Hand to show pinned and outlined, e.g. the one played in a hand history. */
    pin?: string | null;
    /** From the action bar: show only hands that take this action. */
    only?: number | null;
  } = $props();

  type Part = { a: number; freq: number; ev: number | null };
  /** `inRange` false: the hand left the range earlier, all its frequencies are 0. */
  type Hand = { key: string; combos: number; parts: Part[]; inRange: boolean };

  // HandClass::index is row * 13 + col with the ace first, suited above the diagonal.
  const hands = $derived(
    Array.from({ length: 169 }, (_, i): Hand => {
      const parts = actions.map((_, a) => ({
        a,
        freq: (node.freq[a]?.[i] ?? 0) / 255,
        ev: node.ev[a] ? node.ev[a][i] / 100 : null,
      }));
      return { key: classLabel(i), combos: classCombos(i), parts, inRange: parts.some((p) => p.freq > 0) };
    }),
  );

  const pct = (f: number) => `${Math.round(f * 100)} %`;

  /** Parts with fold last, the same order as everywhere on the page. */
  const ordered = (h: Hand) => [...h.parts.filter((p) => actions[p.a].kind !== "fold"), ...h.parts.filter((p) => actions[p.a].kind === "fold")];

  /** How often the hand is played (not folded), shown for mixed hands only. */
  function playedShare(h: Hand): string | undefined {
    const played = h.parts.filter((p) => actions[p.a].kind !== "fold").reduce((t, p) => t + p.freq, 0);
    const v = Math.round(played * 100);
    return v > 0 && v < 100 ? String(v) : undefined;
  }

  const cells = $derived(
    hands.map((h): RangeCell =>
      h.inRange
        ? {
            mix: ordered(h).map((p) => ({ color: actionColor(actions[p.a].kind), share: p.freq })),
            value: playedShare(h),
            note: h.parts.map((p) => `${actionName(actions[p.a], line)} ${pct(p.freq)}`).join(", "),
          }
        : null,
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

  /** The cell's mix as a bar, the same colours as the grid. */
  function mixBar(h: Hand): string {
    let at = 0;
    const stops = ordered(h)
      .filter((p) => p.freq > 0)
      .map((p) => {
        const from = at;
        at += p.freq * 100;
        return `${actionColor(actions[p.a].kind)} ${from.toFixed(1)}% ${at.toFixed(1)}%`;
      });
    return stops.length ? `linear-gradient(to right, ${stops.join(", ")})` : "var(--grid-cell-out)";
  }
</script>

<div bind:this={box}>
<RangeGrid
  {cells}
  label="Preflop-Range als 13 mal 13 Raster"
  hero={pinIndex}
  {selected}
  dimmed={only === null ? undefined : (i) => (hands[i].parts[only]?.freq ?? 0) < 0.005}
  onSelect={(i) => (selected = selected === i ? -1 : i)}
  onClose={() => (selected = -1)}
>
  {#snippet tip(i)}
    {@const h = hands[i]}
    <p class="t-head"><b class="t-key">{h.key}</b></p>
    <p class="t-verdict">{verdict(h)}</p>
    {#if h.inRange}
      <div class="bar" aria-hidden="true" style="background:{mixBar(h)}"></div>
      {#if why(h)}<p class="t-why">{why(h)}</p>{/if}
    {/if}
  {/snippet}
  {#snippet card(i)}
    {@const h = hands[i]}
    {@const best = bestEv(h)}
    <header class="c-head">
      <b class="c-key">{h.key}</b>
      <Tooltip text={TERM.combos}><span class="term">{h.combos} Kombos</span></Tooltip>
    </header>
    <p class="verdict">{verdict(h)}.</p>
    {#if !h.inRange}
      <p class="why">Diese Hand hat vorher anders gespielt (meist gefoldet) und erreicht diese Situation nie.</p>
    {:else}
      {#if why(h)}
        <p class="why">
          {why(h)}{#if taken(h).length > 1}{" "}<Tooltip text={TERM.mixed}><span class="term">Warum gemischt?</span></Tooltip>{/if}
        </p>
      {/if}
      <div class="bar split" aria-hidden="true" style="background:{mixBar(h)}"></div>
      <table class="acts">
        <thead><tr><th>Aktion</th><th>Wie oft</th><th><Tooltip text={TERM.ev}><span class="term">EV</span></Tooltip></th></tr></thead>
        <tbody>
          {#each ordered(h) as p (p.a)}
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
</RangeGrid>
</div>

<style>
  .t-head {
    margin: 0;
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
  .t-verdict {
    margin: 2px 0 0;
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
    height: 5px;
    margin: var(--sp-2) 0 var(--sp-1);
    border-radius: 3px;
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
    white-space: nowrap;
  }
  .acts tr.best td {
    font-weight: 600;
  }
  .foot {
    margin: var(--sp-2) 0 0;
    font-size: var(--fs-xs);
    color: var(--text-2);
  }
  .sw {
    display: inline-block;
    width: 9px;
    height: 9px;
    border-radius: 2px;
    margin-right: 7px;
  }
</style>
