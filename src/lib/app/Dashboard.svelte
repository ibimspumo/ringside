<script lang="ts">
  import AnalyzeBar from "../analysis/AnalyzeBar.svelte";
  import Summary from "../analysis/Summary.svelte";
  import { api } from "../api";
  import type { LearnTarget } from "./Leaks.svelte";
  import { PALETTES, RATED_STATS, STAT_META, bountyText, compact, fmt, stat } from "../stats";
  import type { Dashboard, Filter, HudConfig, ResultPoint, StatId } from "../types";
  import { int, sign, signed } from "./format";
  import ResultsChart, { type Series } from "./ResultsChart.svelte";
  import Wheel from "./Wheel.svelte";

  const SERIES: Series<ResultPoint>[] = [
    { key: "net_bb", label: "Gesamt", color: "var(--text)", width: 2 },
    { key: "ev_bb", label: "All-in-EV", color: "var(--amber)", width: 1.5, dash: "5 3" },
    { key: "showdown_bb", label: "Mit Showdown", color: "var(--sd)", width: 1.5 },
    { key: "non_showdown_bb", label: "Ohne Showdown", color: "var(--neg)", width: 1.5 },
  ];

  const EV_HELP =
    "All-in-EV: dein Ergebnis, wenn jedes All-in mit offenen Karten so ausgegangen wäre, wie es im Schnitt ausgeht. " +
    "Ringside rechnet je Side Pot die exakte Equity × Pot nach Rake − deinen Einsatz. Bounties zählen nicht mit.";
  const LUCK_HELP =
    "Glück: tatsächliches Ergebnis − EV in diesen All-ins. Positiv heißt, du hast öfter gewonnen, als die Karten erwarten ließen.";

  let {
    filter,
    hud,
    refresh,
    onHand = () => {},
    onLearn,
  }: {
    filter: Filter;
    hud: HudConfig | null;
    refresh: number;
    /** Opens a hand in the replayer at one of its decisions. */
    onHand?: (handId: number, actionIndex: number | null) => void;
    onLearn?: (target: LearnTarget) => void;
  } = $props();

  let data = $state<Dashboard | null>(null);
  let failed = $state(false);

  $effect(() => {
    void refresh;
    const f = { ...filter };
    let live = true;
    api
      .dashboard(f)
      .then((d) => {
        if (!live) return;
        data = d;
        failed = false;
      })
      .catch(() => live && (failed = true));
    return () => (live = false);
  });

  const r = $derived(data?.results ?? null);
  const bb100 = $derived(r && r.hands > 0 ? (r.net_bb / r.hands) * 100 : 0);
  const hero = $derived(data?.hero ?? null);
  const heroHands = $derived(hero ? (stat(hero.stats, "hands")?.value ?? 0) : 0);
  const palette = $derived(hud ? (PALETTES[hud.palette] ?? PALETTES.traffic) : PALETTES.traffic);

  /**
   * Three equal zones (below, inside, above the HUD thresholds); the marker sits where the value
   * falls inside its zone. The outer zones reach 1.5× the middle width past each threshold.
   */
  function gauge(id: StatId, value: number | null): number | null {
    const t = hud?.thresholds[id];
    if (!t || value === null) return null;
    const span = Math.max(t.high - t.low, 0.0001) * 1.5;
    const lo = Math.max(0, t.low - span);
    const hi = STAT_META[id].ratio ? t.high + span : Math.min(100, t.high + span);
    const part = (v: number, a: number, b: number) => (b > a ? Math.min(1, Math.max(0, (v - a) / (b - a))) : 1);
    if (value < t.low) return part(value, lo, t.low) / 3;
    if (value <= t.high) return (1 + part(value, t.low, t.high)) / 3;
    return (2 + part(value, t.high, hi)) / 3;
  }
</script>

<div class="page">
  {#if failed}
    <div class="panel empty-state">Die Daten konnten nicht geladen werden. Starte Ringside neu, falls das so bleibt.</div>
  {:else if data && r}
    <section class="panel kpis">
      <div class="kpi">
        <span class="label">Hände</span>
        <strong class="num">{int(r.hands)}</strong>
        <small>{data.heroes.length ? `als ${data.heroes.join(", ")}` : "noch kein eigener Name erkannt"}</small>
      </div>
      <div class="kpi">
        <span class="label">Ergebnis</span>
        <strong class="num {sign(r.net_bb)}">{signed(r.net_bb)}<span class="unit">BB</span></strong>
        <small>in Big Blinds</small>
      </div>
      <div class="kpi">
        <span class="label">Winrate</span>
        <strong class="num {sign(bb100)}">{signed(bb100, 2)}<span class="unit">BB/100</span></strong>
        <small>pro 100 Hände</small>
      </div>
      {#if filter.game !== "cash"}
        <div class="kpi">
          <span class="label">Bounties gewonnen</span>
          <strong class="num" class:pos={!!bountyText(r.bounties)}>{bountyText(r.bounties) || "–"}</strong>
          <small>aus Knockouts</small>
        </div>
      {/if}
      <div class="kpi" title="{LUCK_HELP} {EV_HELP}">
        <span class="label">Glück</span>
        <strong class="num {sign(r.luck_bb)}">{r.all_ins ? signed(r.luck_bb) : "–"}<span class="unit">BB</span></strong>
        <small>{r.all_ins ? `aus ${int(r.all_ins)} All-ins` : "kein All-in mit offenen Karten"}</small>
      </div>
      <div class="kpi split">
        <span class="label">Showdown</span>
        <div class="pair">
          <span><i class="sw sd"></i>Mit</span>
          <b class="num {sign(r.showdown_bb)}">{signed(r.showdown_bb)}<span class="unit">BB</span></b>
        </div>
        <div class="pair">
          <span><i class="sw nsd"></i>Ohne</span>
          <b class="num {sign(r.non_showdown_bb)}">{signed(r.non_showdown_bb)}<span class="unit">BB</span></b>
        </div>
      </div>
    </section>

    <div class="analysis">
      <AnalyzeBar />
      <Summary {filter} {refresh} onOpen={(id, at) => onHand(id, at)} onSpot={() => onHand(-1, null)} {onLearn} />
    </div>

    <section class="panel">
      <div class="panel-head">
        <h2 class="panel-title">Ergebnis-Verlauf</h2>
        <div class="legend">
          <span><i class="sw total"></i>Gesamt</span>
          <span class="help" title={EV_HELP}><i class="sw ev"></i>All-in-EV</span>
          <span><i class="sw sd"></i>Mit Showdown</span>
          <span><i class="sw nsd"></i>Ohne Showdown</span>
        </div>
      </div>
      <div class="chart">
        <ResultsChart points={r.points} series={SERIES} />
      </div>
    </section>

    <section class="panel">
      <div class="panel-head">
        <h2 class="panel-title">Dein Spielstil</h2>
        {#if hero}<span class="faint hint">Balken: niedrig · normal · hoch nach deinen HUD-Schwellen</span>{/if}
      </div>
      {#if hero && hud}
        <div class="style">
          <div class="who">
            <Wheel stats={hero.stats} {hud} size={64} hands={compact(heroHands)} />
            <div>
              <strong title={hero.name}>{hero.name}</strong>
              <span class="num dim">{int(heroHands)} Hände</span>
            </div>
          </div>
          <div class="stats">
            {#each RATED_STATS as id (id)}
              {@const s = stat(hero.stats, id)}
              {@const g = gauge(id, s?.value ?? null)}
              <div class="stat" title="{STAT_META[id].name}: {STAT_META[id].help}">
                <span class="name">{STAT_META[id].name}</span>
                <b class="num" class:none={g === null && !s?.opp}
                  >{fmt(s)}{#if s && s.value !== null && !STAT_META[id].ratio}<span class="unit">%</span>{/if}</b
                >
                <span class="gauge" aria-hidden="true" style="--lo:{palette.low}; --mid:{palette.mid}; --hi:{palette.high}">
                  <i></i><i></i><i></i>
                  {#if g !== null}<em style="left:{g * 100}%"></em>{/if}
                </span>
                <small class="num" title="Treffer / Gelegenheiten">{int(s?.made ?? 0)} / {int(s?.opp ?? 0)}</small>
              </div>
            {/each}
          </div>
        </div>
      {:else}
        <div class="empty-state">
          Noch keine eigenen Hände in dieser Auswahl. Ringside erkennt deinen Namen automatisch aus den Hand-Historys.
        </div>
      {/if}
    </section>
  {/if}
</div>

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding-top: 20px;
  }
  .analysis {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .kpis {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
  }
  .kpi {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 14px 18px 15px;
    border-left: 1px solid var(--line);
    min-width: 0;
  }
  .kpi:first-child {
    border-left: 0;
  }
  .label {
    font-size: 12.5px;
    color: var(--text-2);
  }
  .kpi strong {
    font-size: 26px;
    font-weight: 600;
    line-height: 1.15;
    white-space: nowrap;
  }
  .kpi small {
    font-size: 12px;
    color: var(--text-3);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .kpi.split {
    gap: 3px;
  }
  .pair {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 10px;
    font-size: 13px;
    color: var(--text-2);
  }
  .pair b {
    font-size: 17px;
    font-weight: 600;
  }
  .sw {
    display: inline-block;
    width: 12px;
    height: 2px;
    margin-right: 7px;
    vertical-align: 4px;
    background: var(--text);
  }
  .sw.sd {
    background: var(--sd);
  }
  .sw.nsd {
    background: var(--neg);
  }
  .sw.ev {
    background: repeating-linear-gradient(90deg, var(--amber) 0 5px, transparent 5px 8px);
  }
  .help {
    cursor: help;
  }
  .legend {
    display: flex;
    gap: 16px;
    font-size: 12.5px;
    color: var(--text-2);
  }
  .chart {
    padding: 10px 14px 10px 4px;
  }
  .hint {
    font-size: 12px;
  }

  .style {
    display: grid;
    grid-template-columns: 196px minmax(0, 1fr);
    margin-top: 14px;
    border-top: 1px solid var(--line);
  }
  .who {
    display: flex;
    align-items: flex-start;
    gap: 14px;
    padding: 18px;
    border-right: 1px solid var(--line);
  }
  .who > div {
    min-width: 0;
  }
  .who strong {
    display: block;
    font-size: 15px;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .who .dim {
    font-size: 13px;
  }
  .style {
    overflow: hidden;
    border-radius: 0 0 var(--r-lg) var(--r-lg);
  }
  .stats {
    display: grid;
    grid-template-columns: repeat(6, minmax(0, 1fr));
    margin: 0 -1px -1px 0;
  }
  .stat {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 13px 14px;
    border-bottom: 1px solid var(--line);
    border-right: 1px solid var(--line);
    cursor: help;
    min-width: 0;
  }
  .stat .name {
    font-size: 12px;
    color: var(--text-2);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .stat b {
    font-size: 22px;
    font-weight: 600;
    line-height: 1.2;
    color: var(--text);
  }
  .stat b.none {
    color: var(--text-3);
  }
  .gauge {
    position: relative;
    display: flex;
    gap: 1px;
    width: 64px;
    height: 4px;
    margin: 5px 0 4px;
  }
  .gauge i {
    flex: 1;
    border-radius: 1px;
  }
  .gauge i:nth-child(1) {
    background: color-mix(in srgb, var(--lo) 35%, transparent);
  }
  .gauge i:nth-child(2) {
    background: color-mix(in srgb, var(--mid) 35%, transparent);
  }
  .gauge i:nth-child(3) {
    background: color-mix(in srgb, var(--hi) 35%, transparent);
  }
  .gauge em {
    position: absolute;
    top: -2px;
    width: 2px;
    height: 8px;
    margin-left: -1px;
    border-radius: 1px;
    background: var(--text);
    box-shadow: 0 0 0 1px var(--surface);
  }
  .stat small {
    font-size: 11.5px;
    color: var(--text-3);
  }

  @container (max-width: 1100px) {
    .stats {
      grid-template-columns: repeat(4, minmax(0, 1fr));
    }
  }
  @container (max-width: 820px) {
    .style {
      grid-template-columns: 1fr;
    }
    .who {
      border-right: 0;
      border-bottom: 1px solid var(--line);
    }
    .legend {
      display: none;
    }
  }
</style>
