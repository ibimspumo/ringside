<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "../api";
  import ClientLock from "../app/ClientLock.svelte";
  import type { PushFoldAnte, PushFoldChart as Chart, PushFoldGrid, PushFoldNode } from "../types";
  import PushFoldChart from "./PushFoldChart.svelte";
  import PreflopCharts from "./PreflopCharts.svelte";

  const ANTE_LABEL: Record<PushFoldAnte, string> = { none: "Ohne Ante", classic: "Ante", big_blind: "BB-Ante" };

  /** Push/fold for short stacks, the preflop model for deeper ones. */
  let mode = $state<"pushfold" | "preflop">("pushfold");
  let grid = $state<PushFoldGrid | null>(null);
  let ante = $state<PushFoldAnte>("big_blind");
  let players = $state(6);
  let stack = $state(10);
  /** Free stack entry; solved live when it is not on the grid. */
  let custom = $state("");
  let seat = $state(0);
  /** Node key within the seat: "open" or "<shover>:<callers>". */
  let spot = $state("open");

  let chart = $state<Chart | null>(null);
  let loading = $state(false);
  let error = $state<string | null>(null);
  const cache = new Map<string, Chart>();

  onMount(() => {
    api.pushfoldGrid().then((g) => (grid = g));
  });

  const anteInfo = $derived(grid?.antes.find((a) => a.ante === ante) ?? null);
  const stacks = $derived(anteInfo?.stacks ?? []);
  const playerCounts = $derived(grid ? range(grid.min_players, grid.max_players) : []);

  function range(a: number, b: number): number[] {
    return Array.from({ length: b - a + 1 }, (_, i) => a + i);
  }

  $effect(() => {
    if (!grid) return;
    const key = `${ante}|${players}|${stack}`;
    const hit = cache.get(key);
    if (hit) {
      chart = hit;
      error = null;
      return;
    }
    let live = true;
    loading = true;
    error = null;
    api
      .pushfoldChart(ante, players, stack)
      .then((c) => {
        cache.set(key, c);
        if (live) chart = c;
      })
      .catch((e) => live && (error = String(e)))
      .finally(() => live && (loading = false));
    return () => (live = false);
  });

  // A smaller table or a stack off the grid of another ante keeps the nearest valid choice.
  $effect(() => {
    if (chart && seat > chart.players - 1) seat = chart.players - 1;
  });
  $effect(() => {
    if (stacks.length && !stacks.includes(stack) && custom === "") stack = nearest(stacks, stack);
  });

  function nearest(list: number[], x: number): number {
    return list.reduce((a, b) => (Math.abs(b - x) < Math.abs(a - x) ? b : a));
  }

  const nodeKey = (n: PushFoldNode) => (n.shover === null ? "open" : `${n.shover}:${n.callers.join(",")}`);
  const pos = (s: number) => chart?.nodes.find((n) => n.seat === s)?.position ?? "";

  function label(n: PushFoldNode): string {
    if (n.shover === null) return "Shove – alle vor dir haben gefoldet";
    const base = `Call gegen Shove von ${pos(n.shover)}`;
    return n.callers.length ? `${base}, nach Call von ${n.callers.map(pos).join(", ")}` : base;
  }

  const seatNodes = $derived(chart?.nodes.filter((n) => n.seat === seat) ?? []);
  const node = $derived(seatNodes.find((n) => nodeKey(n) === spot) ?? seatNodes[0] ?? null);
  const positions = $derived(chart ? range(0, chart.players - 1).map((s) => ({ seat: s, name: pos(s) })) : []);

  function pickSeat(s: number) {
    seat = s;
    // Keep the kind of spot when the new seat has it: open shove, else the first call.
    const keys = chart?.nodes.filter((n) => n.seat === s).map(nodeKey) ?? [];
    if (!keys.includes(spot)) spot = keys[0] ?? "open";
  }

  function applyCustom() {
    const v = Number(custom.replace(",", "."));
    if (Number.isFinite(v) && v > 0) stack = Math.round(v * 100) / 100;
  }

  function bb(x: number): string {
    return x.toLocaleString("de-DE", { maximumFractionDigits: 3 });
  }

  const anteText = $derived.by(() => {
    if (!anteInfo) return "";
    if (anteInfo.ante_bb > 0) return `Ante ${bb(anteInfo.ante_bb)} BB je Spieler`;
    if (anteInfo.bb_ante_bb > 0) return `BB-Ante ${bb(anteInfo.bb_ante_bb)} BB, zahlt der Big Blind`;
    return "Nur Blinds";
  });
</script>

<ClientLock feature="Die Chart-Seite">
  <div class="page">
    <div class="page-head">
      <div>
        <h1>Charts</h1>
        {#if mode === "pushfold"}
          <p>Wann du all-in gehst oder einen Shove callst – im Gleichgewicht für alle Positionen.</p>
        {:else}
          <p>Open, Call, 3-Bet, 4-Bet und All-in mit 15 bis 100 BB – gelöst in einem vereinfachten Modell.</p>
        {/if}
      </div>
      <div class="seg mode" role="group" aria-label="Art der Charts">
        <button class:on={mode === "pushfold"} onclick={() => (mode = "pushfold")}>Push/Fold</button>
        <button class:on={mode === "preflop"} onclick={() => (mode = "preflop")}>Preflop</button>
      </div>
    </div>

    {#if mode === "preflop"}
      <PreflopCharts />
    {:else if grid}
      <section class="panel controls">
        <div class="ctl">
          <span class="lbl">Spieler</span>
          <div class="seg" role="group" aria-label="Spieler am Tisch">
            {#each playerCounts as n (n)}
              <button class:on={players === n} onclick={() => (players = n)}>{n}</button>
            {/each}
          </div>
        </div>
        <div class="ctl">
          <span class="lbl">Ante</span>
          <div class="seg" role="group" aria-label="Ante-Struktur">
            {#each grid.antes as a (a.ante)}
              <button class:on={ante === a.ante} onclick={() => (ante = a.ante)}>{ANTE_LABEL[a.ante]}</button>
            {/each}
          </div>
        </div>
        <div class="ctl">
          <label class="lbl" for="pf-stack">Stack</label>
          <div class="stack">
            <select
              id="pf-stack"
              class="field"
              value={stacks.includes(stack) ? String(stack) : "custom"}
              onchange={(e) => {
                const v = e.currentTarget.value;
                if (v === "custom") return;
                custom = "";
                stack = Number(v);
              }}
            >
              {#each stacks as s (s)}
                <option value={String(s)}>{bb(s)} BB</option>
              {/each}
              <option value="custom" disabled={stacks.includes(stack)}>Eigener: {bb(stack)} BB</option>
            </select>
            <form
              class="custom"
              onsubmit={(e) => {
                e.preventDefault();
                applyCustom();
              }}
            >
              <input
                class="field num"
                inputmode="decimal"
                placeholder="z. B. 13,5"
                aria-label="Eigener Stack in BB"
                bind:value={custom}
              />
              <button class="btn" type="submit" disabled={!custom}>Rechnen</button>
            </form>
          </div>
        </div>
      </section>

      {#if positions.length}
        <section class="panel controls">
          <div class="ctl">
            <span class="lbl">Position</span>
            <div class="seg wrap" role="group" aria-label="Position">
              {#each positions as p (p.seat)}
                <button class:on={seat === p.seat} onclick={() => pickSeat(p.seat)}>{p.name}</button>
              {/each}
            </div>
          </div>
          <div class="ctl grow">
            <label class="lbl" for="pf-spot">Situation</label>
            <select id="pf-spot" class="field spot" value={node ? nodeKey(node) : ""} onchange={(e) => (spot = e.currentTarget.value)}>
              {#each seatNodes as n (nodeKey(n))}
                <option value={nodeKey(n)}>{label(n)}</option>
              {/each}
            </select>
          </div>
        </section>
      {/if}

      <div class="body">
        <section class="panel chart-panel" aria-busy={loading}>
          <div class="panel-head">
            <h2 class="panel-title">
              {#if node}
                {node.position} · {node.shover === null ? "Shove" : "Call"}
              {:else}
                Chart
              {/if}
            </h2>
            {#if node}
              <span class="faint num">spielt {node.act_pct.toLocaleString("de-DE", { maximumFractionDigits: 1 })} % der Hände</span>
            {/if}
          </div>
          <div class="chart-wrap" class:stale={loading}>
            {#if error}
              <p class="msg neg">{error}</p>
            {:else if node}
              <PushFoldChart {node} action={node.shover === null ? "shove" : "call"} />
            {:else}
              <p class="msg faint">Wird berechnet …</p>
            {/if}
          </div>
          {#if loading && chart}
            <p class="busy faint">Wird berechnet … ohne vorgerechnete Tabelle dauert das einige Sekunden.</p>
          {/if}
        </section>

        <aside class="side">
          {#if chart && node}
            <section class="panel facts">
              <div class="panel-head"><h2 class="panel-title">Spot</h2></div>
              <dl>
                <dt>Tisch</dt>
                <dd>{chart.players} Spieler, je {bb(chart.stack_bb)} BB</dd>
                <dt>Antes</dt>
                <dd>{anteText}</dd>
                <dt>Erreicht</dt>
                <dd class="num">{(node.reach * 100).toLocaleString("de-DE", { maximumFractionDigits: 1 })} % der Hände</dd>
                <dt>Quelle</dt>
                <dd>{chart.precomputed ? "Vorgerechnet" : "Gerade live berechnet"}</dd>
              </dl>
              <div class="legend">
                <span><i class="sw shove"></i>Shove</span>
                <span><i class="sw call"></i>Call</span>
                <span><i class="sw fold"></i>Fold</span>
              </div>
              <p class="note">
                Teilweise gefüllte Felder spielst du gemischt; die Zahl ist der Anteil in Prozent. Gepaart auf der Diagonale,
                suited oben rechts, offsuit unten links.
              </p>
            </section>
            <section class="panel facts">
              <div class="panel-head"><h2 class="panel-title">Modell</h2></div>
              <p class="note">
                Reines Push/Fold: Jeder geht all-in oder foldet, Limps und kleine Raises gibt es nicht. Höchstens drei Spieler
                gehen gemeinsam all-in, danach folden alle anderen. Bewertet wird in Chips (Chip-EV), ohne Turnier-Auszahlungen
                (ICM). Alle Spieler haben denselben Stack.
              </p>
              {#if !chart.converged}
                <p class="note warn">
                  Nicht ganz im Gleichgewicht: Ein Spieler könnte noch {chart.exploitability_bb.toLocaleString("de-DE", {
                    maximumFractionDigits: 3,
                  })} BB je Hand gewinnen.
                </p>
              {/if}
            </section>
          {/if}
        </aside>
      </div>
    {/if}
  </div>
</ClientLock>

<style>
  .mode {
    flex-shrink: 0;
  }
  .controls {
    display: flex;
    flex-wrap: wrap;
    gap: 14px 24px;
    padding: 14px 18px;
    margin-bottom: 12px;
  }
  .ctl {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }
  .ctl.grow {
    flex: 1 1 260px;
  }
  .lbl {
    font-size: 12px;
    color: var(--text-2);
  }
  .seg.wrap {
    flex-wrap: wrap;
  }
  .stack {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  .custom {
    display: flex;
    gap: 6px;
  }
  .custom .field {
    width: 92px;
  }
  .spot {
    width: 100%;
    max-width: 440px;
  }
  .body {
    display: grid;
    grid-template-columns: minmax(0, 600px) minmax(0, 1fr);
    gap: 16px;
    align-items: start;
  }
  .chart-panel {
    container-type: inline-size;
  }
  .chart-wrap {
    padding: 14px 18px 18px;
    transition: opacity 0.15s;
  }
  .chart-wrap.stale {
    opacity: 0.45;
  }
  .busy {
    margin: -6px 18px 14px;
    font-size: 12.5px;
  }
  .msg {
    margin: 0;
    padding: 40px 0;
    text-align: center;
  }
  .side {
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-width: 0;
  }
  .facts {
    padding-bottom: 16px;
  }
  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 6px 14px;
    margin: 12px 18px 0;
    font-size: 13px;
  }
  dt {
    color: var(--text-2);
  }
  dd {
    margin: 0;
  }
  .legend {
    display: flex;
    gap: 14px;
    margin: 14px 18px 0;
    font-size: 12.5px;
    color: var(--text-2);
  }
  .legend span {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .sw {
    width: 12px;
    height: 12px;
    border-radius: 3px;
  }
  .sw.shove {
    background: color-mix(in srgb, var(--neg) 78%, var(--surface-2));
  }
  .sw.call {
    background: color-mix(in srgb, var(--pos) 70%, var(--surface-2));
  }
  .sw.fold {
    background: var(--surface-2);
    box-shadow: inset 0 0 0 1px var(--line-2);
  }
  .note {
    margin: 10px 18px 0;
    font-size: 12.5px;
    line-height: 1.5;
    color: var(--text-2);
  }
  .note.warn {
    color: var(--amber);
  }

  @container (max-width: 980px) {
    .body {
      grid-template-columns: minmax(0, 640px);
    }
  }
  @container (max-width: 560px) {
    .controls {
      padding: 12px 14px;
    }
    .chart-wrap {
      padding: 12px 10px 14px;
    }
  }
</style>
