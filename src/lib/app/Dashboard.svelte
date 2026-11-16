<script lang="ts">
  import { api } from "../api";
  import { STAT_META, fmt, stat, tone } from "../stats";
  import type { Dashboard, GameKind, HudConfig, PlayerView, StatId } from "../types";
  import GameFilter from "./GameFilter.svelte";

  let { refresh }: { refresh: number } = $props();

  let game = $state<GameKind | null>("cash");
  let data = $state<Dashboard | null>(null);
  let hero = $state<PlayerView | null>(null);
  let hud = $state<HudConfig | null>(null);

  $effect(() => {
    void refresh;
    const g = game;
    api.dashboard(g).then(async (d) => {
      data = d;
      const name = d.heroes[0];
      hero = name ? ((await api.players(g, name, 20)).find((p) => p.name === name) ?? null) : null;
    });
    api.getSettings().then((s) => (hud = s.hud));
  });

  const W = 1000;
  const H = 260;

  const series = $derived.by(() => {
    if (!data || data.results.length === 0) return null;
    let sum = 0;
    const pts = data.results.map((r) => (sum += r.big_blind > 0 ? r.net / r.big_blind : 0));
    const step = Math.max(1, Math.ceil(pts.length / 500));
    const sampled = pts.filter((_, i) => i % step === 0 || i === pts.length - 1);
    const lo = Math.min(0, ...sampled);
    const hi = Math.max(0, ...sampled);
    const span = hi - lo || 1;
    const x = (i: number) => (i / Math.max(sampled.length - 1, 1)) * W;
    const y = (v: number) => H - 12 - ((v - lo) / span) * (H - 24);
    const line = sampled.map((v, i) => `${i ? "L" : "M"}${x(i).toFixed(1)},${y(v).toFixed(1)}`).join("");
    return {
      line,
      area: `${line}L${W},${y(0)}L0,${y(0)}Z`,
      zero: y(0),
      total: sum,
      hands: pts.length,
      bb100: (sum / pts.length) * 100,
      up: sum >= 0,
    };
  });

  const TILES: StatId[] = ["vpip", "pfr", "three_bet", "steal", "cbet", "af", "wtsd", "wsd"];
</script>

<div class="page">
  <div class="page-head">
    <div>
      <h1>Übersicht</h1>
      <p>{data?.heroes.length ? `Deine Ergebnisse als ${data.heroes.join(", ")}` : "Noch keine eigenen Hände importiert"}</p>
    </div>
    <GameFilter bind:value={game} />
  </div>

  <div class="kpis">
    <div class="kpi panel">
      <span>Hände</span>
      <strong class="num">{(series?.hands ?? 0).toLocaleString("de-DE")}</strong>
    </div>
    <div class="kpi panel">
      <span>Ergebnis</span>
      <strong class="num {series && series.total < 0 ? 'neg' : 'pos'}"
        >{series ? `${series.total > 0 ? "+" : ""}${series.total.toFixed(1)}` : "–"}<small>BB</small></strong
      >
    </div>
    <div class="kpi panel">
      <span>Winrate</span>
      <strong class="num {series && series.bb100 < 0 ? 'neg' : 'pos'}"
        >{series ? series.bb100.toFixed(1) : "–"}<small>BB/100</small></strong
      >
    </div>
    <div class="kpi panel">
      <span>VPIP / PFR</span>
      <strong class="num">
        {#if hero && hud}
          <span class="tone-{tone(hud, stat(hero.stats, 'vpip'))}">{fmt(stat(hero.stats, "vpip"))}</span><em>/</em><span
            class="tone-{tone(hud, stat(hero.stats, 'pfr'))}">{fmt(stat(hero.stats, "pfr"))}</span
          >
        {:else}–{/if}
      </strong>
    </div>
  </div>

  <section class="panel chart">
    <h2 class="panel-title">Gewinnkurve in Big Blinds</h2>
    {#if series}
      <svg viewBox="0 0 {W} {H}" preserveAspectRatio="none">
        <defs>
          <linearGradient id="area" x1="0" x2="0" y1="0" y2="1">
            <stop offset="0" stop-color={series.up ? "#3ddc97" : "#ff5b5b"} stop-opacity="0.28" />
            <stop offset="1" stop-color={series.up ? "#3ddc97" : "#ff5b5b"} stop-opacity="0" />
          </linearGradient>
        </defs>
        <line x1="0" x2={W} y1={series.zero} y2={series.zero} class="zero" />
        <path d={series.area} fill="url(#area)" />
        <path d={series.line} class="line" class:down={!series.up} />
      </svg>
    {:else}
      <div class="empty">Sobald du Hände spielst, erscheint hier deine Kurve.</div>
    {/if}
  </section>

  {#if hero && hud}
    <section class="panel">
      <h2 class="panel-title">Dein Spielstil</h2>
      <div class="tiles">
        {#each TILES as id (id)}
          {@const s = stat(hero.stats, id)}
          <div class="tile" title={STAT_META[id].help}>
            <span>{STAT_META[id].name}</span>
            <strong class="num tone-{tone(hud, s)}">{fmt(s)}{STAT_META[id].ratio || !s?.value ? "" : "%"}</strong>
            <small class="num">{s?.made ?? 0} / {s?.opp ?? 0}</small>
          </div>
        {/each}
      </div>
    </section>
  {/if}
</div>

<style>
  .kpis {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 14px;
    margin-bottom: 14px;
  }
  .kpi {
    padding: 16px 18px;
  }
  .kpi > span {
    color: var(--text-dim);
    font-size: 12.5px;
  }
  .kpi strong {
    display: block;
    margin-top: 6px;
    font-size: 30px;
    font-weight: 600;
    line-height: 1;
  }
  .kpi small {
    font-size: 13px;
    margin-left: 5px;
    color: var(--text-faint);
    font-weight: 500;
  }
  .kpi em {
    font-style: normal;
    color: var(--text-faint);
    margin: 0 3px;
  }
  .chart {
    margin-bottom: 14px;
  }
  .chart svg {
    display: block;
    width: 100%;
    height: 280px;
    padding: 8px 6px 10px;
  }
  .zero {
    stroke: rgba(255, 255, 255, 0.12);
    stroke-dasharray: 4 6;
    vector-effect: non-scaling-stroke;
  }
  .line {
    fill: none;
    stroke: var(--win);
    stroke-width: 2;
    vector-effect: non-scaling-stroke;
    stroke-linejoin: round;
  }
  .line.down {
    stroke: var(--loss);
  }
  .tiles {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 1px;
    margin: 14px 0 0;
    background: var(--line);
    border-top: 1px solid var(--line);
    border-radius: 0 0 14px 14px;
    overflow: hidden;
  }
  .tile {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 14px 18px;
    background: var(--panel);
  }
  .tile span {
    color: var(--text-dim);
    font-size: 12.5px;
  }
  .tile strong {
    font-size: 24px;
    font-weight: 600;
  }
  .tile small {
    color: var(--text-faint);
    font-size: 11.5px;
  }
</style>
