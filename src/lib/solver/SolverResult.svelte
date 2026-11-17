<script lang="ts">
  import { api } from "../api";
  import { signed } from "../app/format";
  import PlayingCard from "../PlayingCard.svelte";
  import { actionColors, actionLabel, bb, bytes, duration } from "./actions";
  import { RANKS } from "./range";
  import StrategyGrid from "./StrategyGrid.svelte";
  import { solver } from "./store.svelte";
  import type { SolverNode, SolverStep } from "./types";

  let { onEdit, onNew }: { onEdit: () => void; onNew: () => void } = $props();

  type Crumb = { step: SolverStep; label: string };
  let line = $state<Crumb[]>([]);
  let node = $state<SolverNode | null>(null);
  let error = $state<string | null>(null);
  let loading = $state(false);
  const cache = new Map<string, SolverNode>();

  const spot = $derived(solver.solved ?? solver.spot);
  const summary = $derived(solver.summary);
  const path = $derived(line.map((c) => c.step));

  $effect(() => {
    const p = path;
    const key = JSON.stringify(p);
    const hit = cache.get(key);
    if (hit) {
      node = hit;
      error = null;
      return;
    }
    let live = true;
    loading = true;
    api
      .solverNode(p)
      .then((n) => {
        cache.set(key, n);
        if (live) {
          node = n;
          error = null;
        }
      })
      .catch((e) => live && (error = String(e)))
      .finally(() => live && (loading = false));
    return () => (live = false);
  });

  const name = (p: "oop" | "ip") => `${solver.positions[p] || p.toUpperCase()} (${p === "oop" ? "OOP" : "IP"})`;
  const short = (p: "oop" | "ip") => solver.positions[p] || p.toUpperCase();
  const colors = $derived(node ? actionColors(node.actions) : []);
  const labels = $derived(node ? node.actions.map((a) => actionLabel(a, node!.pot)) : []);
  const hero = $derived(node && solver.hero && solver.hero.player === node.player ? solver.hero.cards : null);
  const heroHand = $derived(
    hero && node ? node.hands.find((h) => h.combo === hero[0] + hero[1] || h.combo === hero[1] + hero[0]) : undefined,
  );

  function act(k: number) {
    if (!node?.player) return;
    line = [...line, { step: { action: k }, label: `${short(node.player)} ${labels[k]}` }];
  }

  function deal(card: string) {
    if (!node) return;
    line = [...line, { step: { card }, label: `${node.board.length === 3 ? "Turn" : "River"} ${card}` }];
  }

  function back(to: number) {
    line = line.slice(0, to);
  }

  const SUITS = ["s", "h", "d", "c"];
  const pct = (x: number) => `${Math.round(x * 100)} %`;
  const terminalText = $derived.by(() => {
    if (!node || node.kind !== "terminal") return "";
    const last = line.at(-1)?.label ?? "";
    return last.endsWith("Fold") ? "Gefoldet – die Hand ist vorbei." : "Showdown – die Hand ist vorbei.";
  });
</script>

<div class="result">
  <section class="panel head">
    <div class="spot">
      <div class="board">
        {#each node?.board ?? [] as c (c)}<PlayingCard card={c} size={30} />{/each}
      </div>
      <div class="facts">
        <span>Pot <b class="num">{bb(node?.pot ?? spot.pot, 1)} BB</b></span>
        <span>Stack <b class="num">{bb(node ? Math.min(...node.stacks) : spot.stack, 1)} BB</b></span>
        <span class="dim">{name("oop")} gegen {name("ip")}</span>
      </div>
    </div>
    {#if summary}
      <div class="stats">
        <span title="Was ein perfekter Gegenspieler im Schnitt mehr gewinnen könnte">
          Ausbeutbarkeit <b class="num" class:warn={!summary.converged}>{summary.exploitability_pct.toLocaleString("de-DE", { maximumFractionDigits: 2 })} %</b>
        </span>
        <span>Iterationen <b class="num">{summary.iterations}</b></span>
        <span>Zeit <b class="num">{duration(summary.elapsed_ms / 1000)}</b></span>
        <span>Speicher <b class="num">{bytes(summary.memory_bytes)}{summary.compressed ? " (16 Bit)" : ""}</b></span>
      </div>
    {/if}
    <div class="buttons">
      <button class="btn" onclick={onEdit}>Spot bearbeiten</button>
      <button class="btn ghost" onclick={onNew}>Verwerfen</button>
    </div>
  </section>

  <p class="notice">
    Die Lösung gilt nur für die gewählten Ranges und Einsatzgrößen – nicht für das, was deine Gegner wirklich spielen.
    {#if solver.notes.oop || solver.notes.ip}<b>Die Ranges sind Annahmen aus Position und Preflop-Aktion.</b>{/if}
    {#if summary && !summary.converged}Das Ziel wurde nicht erreicht; die Strategie ist noch ungenau.{/if}
  </p>

  <nav class="line" aria-label="Spielverlauf">
    <button class:on={line.length === 0} onclick={() => back(0)}>Start</button>
    {#each line as c, i (i)}
      <span class="sep">›</span>
      <button class:on={i === line.length - 1} onclick={() => back(i + 1)}>{c.label}</button>
    {/each}
  </nav>

  {#if error}
    <p class="err">{error}</p>
  {:else if node}
    {#if node.kind === "action" && node.player}
      <section class="actions" aria-label="Aktionen">
        <span class="actor"><b>{name(node.player)}</b> ist dran</span>
        <div class="choices">
          {#each node.actions as a, k (k)}
            <button class="choice" onclick={() => act(k)}>
              <i style="background:{colors[k]}"></i>
              <span>{labels[k]}</span>
              <b class="num">{pct(a.frequency)}</b>
            </button>
          {/each}
        </div>
      </section>
      <div class="main" class:busy={loading}>
        <div class="grid-wrap">
          <StrategyGrid {node} {labels} {colors} {hero} />
        </div>
        <aside class="side">
          <section class="panel box">
            <h3>Range je Aktion</h3>
            <div class="stack-bar">
              {#each node.actions as a, k (k)}<i style="width:{a.frequency * 100}%; background:{colors[k]}"></i>{/each}
            </div>
            <table>
              <tbody>
                {#each node.actions as a, k (k)}
                  <tr>
                    <td><i class="dot" style="background:{colors[k]}"></i>{labels[k]}</td>
                    <td class="num">{pct(a.frequency)}</td>
                    <td class="num dim">{bb(a.combos, 1)} Kombos</td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </section>
          <section class="panel box">
            <h3>Spieler</h3>
            <table>
              <thead><tr><th></th><th>Kombos</th><th>Equity</th><th>EV</th></tr></thead>
              <tbody>
                {#each ["oop", "ip"] as const as p, i (p)}
                  <tr class:me={node.player === p}>
                    <td>{short(p)}</td>
                    <td class="num">{bb(node.players[i].combos, 1)}</td>
                    <td class="num">{pct(node.players[i].equity)}</td>
                    <td class="num">{bb(node.players[i].ev, 2)}</td>
                  </tr>
                {/each}
              </tbody>
            </table>
            <p class="faint small">EV in BB: erwarteter Anteil am Pot minus weitere eigene Einsätze.</p>
          </section>
          {#if heroHand}
            <section class="panel box">
              <h3>Deine Hand {heroHand.combo}</h3>
              {#each heroHand.strategy as s, k (k)}
                <div class="kv"><span><i class="dot" style="background:{colors[k]}"></i>{labels[k]}</span><b class="num">{pct(s)}</b></div>
              {/each}
              <div class="kv"><span>Equity</span><b class="num">{pct(heroHand.equity)}</b></div>
              <div class="kv"><span>EV</span><b class="num">{signed(heroHand.ev, 2)} BB</b></div>
            </section>
          {/if}
        </aside>
      </div>
    {:else if node.kind === "chance"}
      <section class="panel chance">
        <h3>{node.board.length === 3 ? "Turn" : "River"} wählen</h3>
        <div class="deck">
          {#each SUITS as s (s)}
            {#each RANKS.split("") as r (r)}
              {@const c = r + s}
              {#if node.cards.includes(c)}
                <button class="pick" onclick={() => deal(c)} aria-label={c}><PlayingCard card={c} size={28} /></button>
              {:else}
                <span class="pick gone"><PlayingCard card={c} size={28} /></span>
              {/if}
            {/each}
          {/each}
        </div>
      </section>
    {:else}
      <section class="panel end">
        <p>{terminalText}</p>
        <button class="btn" onclick={() => back(line.length - 1)}>Einen Schritt zurück</button>
      </section>
    {/if}
  {/if}
</div>

<style>
  .result {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 12px 24px;
    padding: 12px 16px;
  }
  .spot {
    display: flex;
    align-items: center;
    gap: 14px;
  }
  .board {
    display: flex;
    gap: 4px;
  }
  .facts,
  .stats {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 16px;
    font-size: 13px;
    color: var(--text-2);
  }
  .facts b,
  .stats b {
    color: var(--text);
    font-weight: 500;
  }
  .stats b.warn {
    color: var(--amber);
  }
  .buttons {
    display: flex;
    gap: 8px;
    margin-left: auto;
  }
  .notice {
    margin: 0;
    padding: 8px 12px;
    border-left: 3px solid var(--amber);
    background: color-mix(in srgb, var(--amber) 7%, transparent);
    color: var(--text-2);
    font-size: 13px;
  }
  .notice b {
    color: var(--text);
    font-weight: 600;
  }
  .line {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px;
    font-size: 13px;
  }
  .line button {
    padding: 3px 8px;
    border: 1px solid var(--line);
    border-radius: var(--r-sm);
    background: var(--surface);
    color: var(--text-2);
    cursor: pointer;
  }
  .line button.on {
    color: var(--text);
    border-color: var(--line-2);
    background: var(--surface-3);
  }
  .sep {
    color: var(--text-3);
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px 16px;
  }
  .actor {
    color: var(--text-2);
    font-size: 13.5px;
  }
  .actor b {
    color: var(--text);
  }
  .choices {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .choice {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 32px;
    padding: 0 12px 0 10px;
    border: 1px solid var(--line-2);
    border-radius: var(--r-sm);
    background: var(--surface-2);
    font-size: 13px;
    cursor: pointer;
  }
  .choice:hover {
    background: var(--surface-3);
  }
  .choice i,
  .dot {
    display: inline-block;
    width: 10px;
    height: 10px;
    border-radius: 2px;
    flex-shrink: 0;
  }
  .dot {
    margin-right: 7px;
    vertical-align: -1px;
  }
  .choice b {
    color: var(--text-2);
    font-weight: 500;
  }
  .main {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 300px;
    gap: 16px;
    align-items: start;
  }
  .main.busy {
    opacity: 0.6;
  }
  .grid-wrap {
    max-width: 660px;
    container-type: inline-size;
  }
  .side {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .box {
    padding: 12px 14px;
  }
  h3 {
    margin: 0 0 8px;
    font-size: 13px;
    font-weight: 600;
  }
  .stack-bar {
    display: flex;
    height: 10px;
    border-radius: 3px;
    overflow: hidden;
    margin-bottom: 8px;
    background: var(--surface-2);
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 12.5px;
  }
  th {
    color: var(--text-3);
    font-weight: 500;
    font-size: 11px;
    text-align: right;
  }
  td {
    padding: 3px 0;
  }
  td + td {
    text-align: right;
    padding-left: 8px;
  }
  tr.me td:first-child {
    font-weight: 600;
  }
  .small {
    margin: 6px 0 0;
    font-size: 11.5px;
  }
  .kv {
    display: flex;
    justify-content: space-between;
    font-size: 12.5px;
    padding: 2px 0;
  }
  .chance,
  .end {
    padding: 14px 16px;
  }
  .deck {
    display: grid;
    grid-template-columns: repeat(13, minmax(0, max-content));
    gap: 4px;
  }
  .pick {
    padding: 0;
    border: 0;
    background: none;
    cursor: pointer;
    line-height: 0;
  }
  .pick:hover {
    transform: translateY(-2px);
  }
  .pick.gone {
    opacity: 0.15;
    cursor: default;
  }
  .end {
    display: flex;
    align-items: center;
    gap: 16px;
  }
  .end p {
    margin: 0;
  }
  .err {
    color: var(--neg);
  }
  @container (max-width: 900px) {
    .main {
      grid-template-columns: minmax(0, 1fr);
    }
  }
  @container (max-width: 560px) {
    .deck {
      grid-template-columns: repeat(7, max-content);
    }
    .buttons {
      margin-left: 0;
    }
  }
</style>
