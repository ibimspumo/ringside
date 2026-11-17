<script lang="ts">
  import { onMount } from "svelte";
  import { api, onPreflopProgress } from "../api";
  import PreflopChart from "./PreflopChart.svelte";
  import { GROUP_LABEL, actionColor, actionName, bb, spotGroup, spotLabel, spotTitle, type SpotGroup } from "./preflop-labels";
  import type { PreflopAnte, PreflopGrid, PreflopNode, PreflopProgress, PreflopSpot, PreflopSpots } from "./preflop-types";

  /** Spots reached less often than this are left out of the list. */
  const MIN_REACH = 1e-4;
  const GROUPS: SpotGroup[] = ["open", "vs_open", "vs_3bet", "later"];
  const ANTE_LABEL: Record<PreflopAnte, string> = { none: "Ohne Ante", classic: "Ante", big_blind: "BB-Ante" };

  let grid = $state<PreflopGrid | null>(null);
  let ante = $state<PreflopAnte>("big_blind");
  let players = $state(9);
  let stack = $state(40);
  let custom = $state("");
  let seat = $state(0);
  let spotId = $state<number | null>(null);

  let spots = $state<PreflopSpots | null>(null);
  let node = $state<PreflopNode | null>(null);
  let loading = $state(false);
  let error = $state<string | null>(null);
  /** Checks of the running live solve; the first one sets the scale of the bar. */
  let progress = $state<{ first: PreflopProgress; last: PreflopProgress } | null>(null);
  const spotCache = new Map<string, PreflopSpots>();
  const nodeCache = new Map<string, PreflopNode>();

  onMount(() => {
    api.preflopGrid().then((g) => (grid = g));
    const off = onPreflopProgress((p) => {
      if (p.ante !== ante || p.players !== players || p.stack_bb !== stack) return;
      progress = progress ? { first: progress.first, last: p } : { first: p, last: p };
    });
    return () => {
      off.then((f) => f());
    };
  });

  const live = $derived(!!grid && players <= grid.live_max_players);
  const onGrid = $derived(grid?.stacks.includes(stack) ?? true);

  $effect(() => {
    if (!grid) return;
    const key = `${ante}|${players}|${stack}`;
    const hit = spotCache.get(key);
    if (hit) {
      spots = hit;
      error = null;
      return;
    }
    let alive = true;
    loading = true;
    error = null;
    progress = null;
    api
      .preflopSpots(ante, players, stack)
      .then((s) => {
        spotCache.set(key, s);
        if (alive) spots = s;
      })
      .catch((e) => {
        if (alive) {
          error = String(e);
          spots = null;
        }
      })
      .finally(() => alive && (loading = false));
    return () => (alive = false);
  });

  const visible = $derived(spots?.nodes.filter((n) => n.reach >= MIN_REACH || !n.line.length) ?? []);
  const seatSpots = $derived(visible.filter((n) => n.seat === seat));
  const spot = $derived(seatSpots.find((n) => n.id === spotId) ?? seatSpots[0] ?? null);
  const positions = $derived(spots?.positions.map((name, s) => ({ seat: s, name })) ?? []);
  const hidden = $derived((spots?.nodes.filter((n) => n.seat === seat).length ?? 0) - seatSpots.length);

  // Per-hand data of the chosen spot.
  $effect(() => {
    if (!spots || !spot) {
      node = null;
      return;
    }
    const key = `${spots.ante}|${spots.players}|${spots.stack_bb}|${spot.id}`;
    const hit = nodeCache.get(key);
    if (hit) {
      node = hit;
      return;
    }
    let alive = true;
    api
      .preflopNode(spots.ante, spots.players, spots.stack_bb, spot.id)
      .then((n) => {
        nodeCache.set(key, n);
        if (alive) node = n;
      })
      .catch((e) => alive && (error = String(e)));
    return () => (alive = false);
  });

  // A smaller table keeps the nearest seat; another seat keeps the kind of spot if it has one.
  $effect(() => {
    if (spots && seat > spots.players - 1) seat = spots.players - 1;
  });

  function pickSeat(s: number) {
    const kind = spot ? spotGroup(spot) : "open";
    seat = s;
    spotId = visible.find((n) => n.seat === s && spotGroup(n) === kind)?.id ?? null;
  }

  function applyCustom() {
    const v = Number(custom.replace(",", "."));
    if (Number.isFinite(v) && v > 0) stack = Math.round(v * 10) / 10;
  }

  const groups = $derived(
    GROUPS.map((g) => ({ g, items: seatSpots.filter((n) => spotGroup(n) === g).sort((a, b) => b.reach - a.reach) })).filter(
      (x) => x.items.length,
    ),
  );

  const pct = (x: number, d = 1) => x.toLocaleString("de-DE", { maximumFractionDigits: d });

  /** Share of the way from the first check to the target, on a log scale of NashConv. */
  const done = $derived.by(() => {
    if (!progress) return 0;
    const { first, last } = progress;
    const span = Math.log(first.exploitability / last.target_exploitability);
    const byExpl = span > 0 ? Math.log(first.exploitability / last.exploitability) / span : 0;
    return Math.min(1, Math.max(byExpl, last.iterations / last.max_iterations, 0));
  });

  const anteText = (s: { ante_bb: number; bb_ante_bb: number }) =>
    s.ante_bb > 0
      ? `Ante ${bb(s.ante_bb)} BB je Spieler`
      : s.bb_ante_bb > 0
        ? `BB-Ante ${bb(s.bb_ante_bb)} BB, zahlt der Big Blind`
        : "Keine Ante, nur Blinds";
  const played = (s: PreflopSpot) => s.actions.filter((a) => a.kind !== "fold").reduce((t, a) => t + a.pct, 0);

  const sizesText = $derived.by(() => {
    if (!spots) return "";
    const z = spots.sizes;
    return `Open ${bb(z.open_bb)} BB (SB ${bb(z.sb_open_bb)} BB), Raise gegen SB-Limp ${bb(z.iso_bb)} BB, 3-Bet ${bb(z.three_bet_ip)}× in Position / ${bb(z.three_bet_oop)}× ohne Position (+${bb(z.squeeze_per_caller)}× je Caller), 4-Bet ${bb(z.four_bet)}×`;
  });
</script>

<div class="model-note" role="note">
  <strong>Modell, kein exaktes Gleichgewicht.</strong>
  Preflop wird gelöst, das Spiel nach dem Flop aber nur geschätzt: Jeder bekommt seinen Anteil am Pot nach Equity, gewichtet mit
  Position, Initiative, Spielbarkeit der Hand und Stacktiefe. Feste Einsatzgrößen, höchstens ein Caller vor dem Big Blind.
  Ringside nennt das bewusst nicht „GTO“.
</div>

{#if grid}
  <section class="panel controls">
    <div class="ctl">
      <span class="lbl">Spieler</span>
      <div class="seg" role="group" aria-label="Spieler am Tisch">
        {#each grid.players as n (n)}
          <button class:on={players === n} onclick={() => (players = n)}>{n}-max</button>
        {/each}
      </div>
    </div>
    <div class="ctl">
      <span class="lbl">Ante</span>
      <div class="seg" role="group" aria-label="Ante-Struktur">
        {#each grid.antes as a (a.ante)}
          <button class:on={ante === a.ante} title={anteText(a)} onclick={() => (ante = a.ante)}>{ANTE_LABEL[a.ante]}</button>
        {/each}
      </div>
    </div>
    <div class="ctl">
      <label class="lbl" for="pre-stack">Stack</label>
      <div class="stack">
        <select
          id="pre-stack"
          class="field"
          value={onGrid ? String(stack) : "custom"}
          onchange={(e) => {
            const v = e.currentTarget.value;
            if (v === "custom") return;
            custom = "";
            stack = Number(v);
          }}
        >
          {#each grid.stacks as s (s)}
            <option value={String(s)}>{bb(s)} BB</option>
          {/each}
          <option value="custom" disabled={onGrid}>Eigener: {bb(stack)} BB</option>
        </select>
        {#if live}
          <form
            class="custom"
            onsubmit={(e) => {
              e.preventDefault();
              applyCustom();
            }}
          >
            <input class="field num" inputmode="decimal" placeholder="z. B. 35" aria-label="Eigener Stack in BB" bind:value={custom} />
            <button class="btn" type="submit" disabled={!custom}>Rechnen</button>
          </form>
        {/if}
      </div>
    </div>
  </section>

  {#if spots}
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
        <label class="lbl" for="pre-spot">Situation</label>
        <select id="pre-spot" class="field spot" value={spot ? String(spot.id) : ""} onchange={(e) => (spotId = Number(e.currentTarget.value))}>
          {#each groups as grp (grp.g)}
            <optgroup label={GROUP_LABEL[grp.g]}>
              {#each grp.items as s (s.id)}
                <option value={String(s.id)}>{spotLabel(s)} ({pct(s.reach * 100, 2)} %)</option>
              {/each}
            </optgroup>
          {/each}
        </select>
      </div>
    </section>
  {/if}

  <div class="body">
    <section class="panel chart-panel" aria-busy={loading}>
      <div class="panel-head">
        <h2 class="panel-title">{spot ? spotTitle(spot) : "Chart"}</h2>
        {#if spot}
          <span class="faint num">spielt {pct(played(spot))} % der Hände, die hier ankommen</span>
        {/if}
      </div>
      {#if spot}
        <div class="actions">
          {#each spot.actions as a, i (i)}
            <span class="act"><i class="sw" style="background:{actionColor(a.kind)}"></i>{actionName(a, spot.line)} <b class="num">{pct(a.pct)} %</b></span>
          {/each}
        </div>
      {/if}
      <div class="chart-wrap" class:stale={loading}>
        {#if error}
          <p class="msg neg">{error}</p>
        {:else if spot && node && node.id === spot.id}
          <PreflopChart {node} actions={spot.actions} line={spot.line} />
        {:else}
          <p class="msg faint">Wird berechnet …</p>
        {/if}
      </div>
      {#if loading && !onGrid}
        <div class="busy">
          <div class="bar" role="progressbar" aria-label="Fortschritt der Live-Berechnung" aria-valuemin="0" aria-valuemax="100" aria-valuenow={Math.round(done * 100)}>
            <i style="width:{(done * 100).toFixed(1)}%"></i>
          </div>
          <p class="faint">
            Wird live berechnet … Auf einem Laptop mit 4 Kernen dauert das {players > 6 ? "etwa 4 bis 8 Minuten" : "etwa 2 bis 3 Minuten"}.
            {#if progress}
              <span class="num">Iteration {progress.last.iterations}, ausnutzbar {pct(progress.last.exploitability, 4)} BB (Ziel {pct(progress.last.target_exploitability, 4)} BB)</span>
            {/if}
          </p>
          <button class="btn" onclick={() => api.preflopCancel()}>Abbrechen</button>
        </div>
      {/if}
    </section>

    <aside class="side">
      {#if spots && spot}
        <section class="panel facts">
          <div class="panel-head"><h2 class="panel-title">Spot</h2></div>
          <dl>
            <dt>Tisch</dt>
            <dd>{spots.players} Spieler, je {bb(spots.stack_bb)} BB</dd>
            <dt>Antes</dt>
            <dd>{anteText(spots)}</dd>
            <dt>Erreicht</dt>
            <dd class="num">{pct(spot.reach * 100, 2)} % der Hände</dd>
            <dt>Quelle</dt>
            <dd>{spots.precomputed ? "Vorgerechnet" : "Gerade live berechnet"}</dd>
            <dt>Ausnutzbar</dt>
            <dd class="num">
              {pct(spots.exploitability_bb, 4)} BB je Hand (alle Spieler zusammen, im Modell)
            </dd>
          </dl>
          {#if hidden > 0}
            <p class="note">{hidden} sehr seltene Situationen dieser Position sind ausgeblendet.</p>
          {/if}
          <p class="note">
            Jede Zelle ist nach Anteil der Aktionen gefüllt, Fold bleibt leer. Die Zahl unten rechts nennt bei gemischten Händen
            die häufigste Aktion in Prozent. Gepaart auf der Diagonale, suited oben rechts, offsuit unten links.
          </p>
        </section>

        <section class="panel facts">
          <div class="panel-head"><h2 class="panel-title">Modell</h2></div>
          <p class="note">
            <b>Größen:</b>
            {sizesText}; 5-Bet nur All-in. Ein Raise, der mindestens {pct(spots.sizes.commit * 100, 0)} % des Stacks kostet, wird
            zum All-in.
          </p>
          <p class="note">
            <b>Nach dem Flop:</b> Jeder Spieler erhält vom Pot den Anteil Equity<sup>{bb(spots.realization.sharpness)}</sup> ×
            Gewicht, geteilt durch die Summe aller Spieler. Der Exponent lässt starke Hände mehr und schwache weniger
            realisieren. Das Gewicht wächst mit der Position (×{bb(spots.realization.position)} je Spieler, der vorher handelt),
            für den letzten Aggressor (×{bb(spots.realization.initiative)}) und mit der Spielbarkeit (suited
            ×{bb(spots.realization.suited)}, offsuit ×{bb(spots.realization.offsuit)}, verbunden +{pct(spots.realization.connected * 100, 0)} %,
            große Lücke −{pct(spots.realization.gapped * 100, 0)} %). Je kleiner das Stack-zu-Pot-Verhältnis, desto mehr
            zählt nur die reine Equity; All-in-Pots werden exakt abgerechnet.
          </p>
          <p class="note">
            <b>Vereinfachungen:</b> Nach einer 3-Bet spielt nur der erste, der weitermacht; nach einem All-in nur der erste Caller.
            Höchstens drei Spieler sehen einen Flop. Chip-EV ohne ICM, Kartenentfernung paarweise genähert.
          </p>
          {#if !spots.converged}
            <p class="note warn">Nicht ganz konvergiert: Die Ausnutzbarkeit liegt über dem Ziel von {pct(grid.target_exploitability_bb, 4)} BB.</p>
          {/if}
          {#if spots.realized.length}
            <table class="eqr">
              <caption>Realisierte Equity im Modell (häufigste Flops)</caption>
              <thead><tr><th>Pot</th><th>Spieler</th><th>Equity</th><th>Realisiert</th></tr></thead>
              <tbody>
                {#each spots.realized.slice(0, 5) as r, i (i)}
                  <tr>
                    <td class="num">{bb(r.pot_bb)} BB</td>
                    <td>{r.positions.join(" / ")}</td>
                    <td class="num">{r.equity.map((e) => pct(e * 100, 0)).join(" / ")} %</td>
                    <td class="num">{r.realization.map((e) => pct(e * 100, 0)).join(" / ")} %</td>
                  </tr>
                {/each}
              </tbody>
            </table>
            <p class="note">Erster Spieler handelt nach dem Flop zuerst. Veröffentlichte Solver-Werte: BTN gegen BB etwa 115 % / 80 %.</p>
          {/if}
        </section>
      {/if}
    </aside>
  </div>
{/if}

<style>
  .model-note {
    margin-bottom: 12px;
    padding: 10px 14px;
    border-radius: var(--r);
    border: 1px solid color-mix(in srgb, var(--amber) 55%, var(--line));
    background: color-mix(in srgb, var(--amber) 10%, var(--surface));
    font-size: 13px;
    line-height: 1.5;
    color: var(--text-2);
  }
  .model-note strong {
    color: var(--text);
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
    max-width: 520px;
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
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 14px;
    padding: 10px 18px 0;
    font-size: 12.5px;
    color: var(--text-2);
  }
  .act {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .act b {
    color: var(--text);
    font-weight: 600;
  }
  .sw {
    width: 12px;
    height: 12px;
    border-radius: 3px;
    box-shadow: inset 0 0 0 1px var(--line-2);
  }
  .chart-wrap {
    padding: 12px 18px 18px;
    transition: opacity 0.15s;
  }
  .chart-wrap.stale {
    opacity: 0.45;
  }
  .busy {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 12px;
    margin: -6px 18px 14px;
    font-size: 12.5px;
  }
  .busy p {
    flex: 1 1 260px;
    margin: 0;
  }
  .bar {
    flex: 1 1 100%;
    height: 4px;
    border-radius: 2px;
    background: var(--surface-3);
    overflow: hidden;
  }
  .bar i {
    display: block;
    height: 100%;
    background: var(--text);
    transition: width 0.4s;
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
  .note {
    margin: 10px 18px 0;
    font-size: 12.5px;
    line-height: 1.5;
    color: var(--text-2);
  }
  .note b {
    color: var(--text);
    font-weight: 600;
  }
  .note.warn {
    color: var(--amber);
  }
  .eqr {
    margin: 12px 18px 0;
    width: calc(100% - 36px);
    border-collapse: collapse;
    font-size: 12.5px;
  }
  .eqr caption {
    text-align: left;
    color: var(--text-2);
    margin-bottom: 6px;
  }
  .eqr th {
    text-align: left;
    font-weight: 500;
    color: var(--text-3);
    padding: 2px 8px 4px 0;
  }
  .eqr td {
    padding: 3px 8px 3px 0;
    border-top: 1px solid var(--line);
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
    .actions {
      padding: 10px 12px 0;
    }
  }
</style>
