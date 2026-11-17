<script lang="ts" module>
  import type { LearnKind, StatId } from "../types";

  /** Where a leak in the learning area leads: an article, or an exercise at a level. */
  export type LearnTarget = { article: string } | { kind: LearnKind; level: number };
</script>

<script lang="ts">
  import { api, inTauri } from "../api";
  import { SKILLS } from "../learn/catalog";
  import { ARTICLES } from "../learn/wissen/articles";
  import { STAT_META } from "../stats";
  import type { Filter, LeakCheck, LeakReport, SpotFilter } from "../types";
  import { int, sign, signed } from "./format";

  let {
    filter,
    refresh,
    onHands,
    onLearn,
  }: {
    filter: Filter;
    refresh: number;
    onHands: (spot: SpotFilter, label: string) => void;
    onLearn: (target: LearnTarget) => void;
  } = $props();

  const ARTICLE: Partial<Record<StatId, string>> = {
    vpip: "vpip",
    pfr: "pfr",
    three_bet: "three-bet",
    fold_to_three_bet: "fold-three-bet",
    steal: "steal",
    fold_to_steal: "fold-steal",
    cbet: "cbet",
    fold_to_cbet: "fold-cbet",
    wtsd: "wtsd",
    wsd: "wsd",
    af: "af",
  };

  /** Exercises that train the decision behind a leak, where Ringside has one. */
  function exercise(l: LeakCheck): { kind: LearnKind; level: number } | null {
    const short = l.game === "tournament" && l.band <= 1;
    if (short && (l.stat === "steal" || l.stat === "pfr" || l.stat === "vpip")) return { kind: "push_fold_open", level: 1 };
    if (short && l.stat === "fold_to_steal") return { kind: "push_fold_call", level: 1 };
    if (l.stat === "fold_to_cbet" || l.stat === "wtsd") return { kind: "pot_odds", level: 3 };
    return null;
  }

  let data = $state<LeakReport | null>(null);
  let failed = $state(false);

  $effect(() => {
    void refresh;
    const f = { ...filter };
    let live = true;
    api
      .leaks(f)
      .then((r) => {
        if (!live) return;
        data = r;
        failed = false;
      })
      .catch(() => live && (failed = true));
    return () => (live = false);
  });

  const sure = $derived(data ? data.leaks.filter((l) => !l.uncertain) : []);
  const unsure = $derived(data ? data.leaks.filter((l) => l.uncertain) : []);
  const sources = $derived(new Map((data?.sources ?? []).map((s) => [s.id, s])));

  const ratio = (l: LeakCheck) => !!STAT_META[l.stat].ratio;
  const num = (x: number, d = 1) => x.toLocaleString("de-DE", { minimumFractionDigits: d, maximumFractionDigits: d });
  const val = (l: LeakCheck, x: number) => (ratio(l) ? num(x, 2) : `${num(x)} %`);
  const range = (l: LeakCheck) => (ratio(l) ? `${num(l.low)}–${num(l.high)}` : `${num(l.low, 0)}–${num(l.high, 0)} %`);
  const gap = (l: LeakCheck) => (ratio(l) ? num(l.deviation, 2) : `${num(l.deviation)} Punkte`);
  const where = (l: LeakCheck) => (l.positions.length ? l.positions.join(", ") : "alle Positionen");
  const game = (l: LeakCheck) => (l.game === "tournament" ? "Turnier" : "Cash");
  const pct = (x: number) => (x >= 0.995 ? "über 99 %" : `${Math.round(x * 100)} %`);

  /** Position on the bar from 0 to 1: percentages on 0–100, AF on 0–6 or wider. */
  function scale(l: LeakCheck) {
    const max = ratio(l) ? Math.max(6, Math.ceil(Math.min(l.ci_high, 20))) : 100;
    return (x: number) => Math.min(1, Math.max(0, x / max));
  }

  function hands(l: LeakCheck) {
    const made = l.direction === null ? null : l.direction === "too_high";
    const did = made === null ? "" : l.stat === "af" ? (made ? " · Bet/Raise" : " · Call") : made ? " · gemacht" : " · nicht gemacht";
    onHands(
      { stat: l.stat, made, positions: l.positions, band: l.band, game: l.game },
      `${STAT_META[l.stat].name} · ${l.positions.length ? l.positions.join(", ") : "alle"} · ${data?.bands[l.band] ?? ""}${did}`,
    );
  }

  async function open(url: string, e: MouseEvent) {
    if (!inTauri) return;
    e.preventDefault();
    const { openUrl } = await import("@tauri-apps/plugin-opener");
    openUrl(url);
  }
</script>

{#snippet bar(l: LeakCheck)}
  {@const x = scale(l)}
  <div class="bar" aria-hidden="true">
    <span class="ref" style="left:{x(l.low) * 100}%; width:{(x(l.high) - x(l.low)) * 100}%"></span>
    <span class="ci" style="left:{x(l.ci_low) * 100}%; width:{Math.max(0.4, (x(l.ci_high) - x(l.ci_low)) * 100)}%"></span>
    <span class="dot" class:off={l.direction !== null} style="left:{x(l.value) * 100}%"></span>
  </div>
{/snippet}

{#snippet leak(l: LeakCheck)}
  {@const article = ARTICLE[l.stat]}
  {@const title = article ? ARTICLES.find((a) => a.id === article)?.title : undefined}
  {@const ex = exercise(l)}
  <article class="leak panel" class:unsure={l.uncertain}>
    <header>
      <h3>
        {STAT_META[l.stat].name}
        <span class:hi={l.direction === "too_high"} class="dir">{l.direction === "too_high" ? "zu hoch" : "zu niedrig"}</span>
      </h3>
      <div class="tags">
        <span>{where(l)}</span>
        <span>{data?.bands[l.band]}</span>
        <span>{game(l)}</span>
        {#if l.uncertain}
          <span class="warn" title="Unter 30 Gelegenheiten oder weniger als 90 % sicher. Spiel mehr Hände, bevor du daran arbeitest.">noch unsicher</span>
        {/if}
      </div>
    </header>

    <div class="figures">
      <div class="fig">
        <span class="label">Du</span>
        <b class="num">{val(l, l.value)}</b>
        <small class="num"
          >{ratio(l) ? `${int(l.made)} Bets/Raises zu ${int(l.opp)} Calls` : `${int(l.made)} von ${int(l.opp)}`}</small
        >
      </div>
      <div class="fig">
        <span class="label">Richtwert</span>
        <b class="num">{range(l)}</b>
        <small>{gap(l)} {l.direction === "too_high" ? "darüber" : "darunter"}</small>
      </div>
      <div class="fig" title="Wahrscheinlichkeit, dass dein wahrer Wert auf dieser Seite außerhalb des Richtbereichs liegt (aus dem Wilson-Intervall). Dazu das 95-%-Intervall deines Werts.">
        <span class="label">Sicherheit</span>
        <b class="num">{pct(l.confidence)}</b>
        <small class="num">95 %: {val(l, l.ci_low)} – {val(l, l.ci_high)}</small>
      </div>
      {#if l.cost}
        <div
          class="fig"
          title="Grob: dein Ergebnis in diesen {int(l.cost.hands)} Händen gegenüber deinem Schnitt auf {where(l)} über alle Stacktiefen. Das zeigt, was der Spot gekostet hat, nicht sicher, dass dieser eine Wert die Ursache ist. Gilt für den ganzen Spot, nicht nur für diesen Wert."
        >
          <span class="label">Kosten grob</span>
          <b class="num neg">≈ {int(Math.round(l.cost.total_bb))} BB</b>
          <small class="num"
            ><span class={sign(l.cost.spot_bb100)}>{signed(l.cost.spot_bb100)}</span> statt
            <span class={sign(l.cost.base_bb100)}>{signed(l.cost.base_bb100)}</span> BB/100</small
          >
        </div>
      {/if}
    </div>

    {@render bar(l)}

    <footer>
      <div class="cites">
        {#each l.cites as c (c.source)}
          {@const s = sources.get(c.source)}
          <span title={s?.scope}>{s?.publisher ?? c.source}: „{c.says}“</span>
        {/each}
        {#if l.note}<span class="faint">{l.note}</span>{/if}
      </div>
      <div class="links">
        <button class="btn" onclick={() => hands(l)}>Hände ansehen</button>
        {#if article}
          <button class="btn ghost" onclick={() => onLearn({ article })}>Wissen: {title ?? STAT_META[l.stat].name}</button>
        {/if}
        {#if ex}
          <button class="btn ghost" onclick={() => onLearn(ex)}>Übung: {SKILLS[ex.kind].short}</button>
        {/if}
      </div>
    </footer>
  </article>
{/snippet}

<div class="page leaks">
  {#if failed}
    <div class="panel empty">Die Daten konnten nicht geladen werden. Starte Ringside neu, falls das so bleibt.</div>
  {:else if data}
    <section class="panel intro">
      <div class="kpi">
        <span class="label">Leaks</span>
        <strong class="num" class:neg={sure.length > 0}>{int(sure.length)}</strong>
        <small>sicher genug zum Arbeiten</small>
      </div>
      <div class="kpi">
        <span class="label">Noch unsicher</span>
        <strong class="num">{int(unsure.length)}</strong>
        <small>zu wenig Hände für ein Urteil</small>
      </div>
      <div class="kpi">
        <span class="label">Im Richtbereich</span>
        <strong class="num pos">{int(data.fine.length)}</strong>
        <small>{int(data.too_small)} Spots unter 10 Gelegenheiten</small>
      </div>
      <p class="about">
        Ringside vergleicht deine Werte je Position und Stacktiefe mit veröffentlichten Richtwerten. Das sind Orientierungen
        aus den unten genannten Quellen, keine „richtige“ Strategie. Sortiert nach Sicherheit × Abweichung.
      </p>
    </section>

    {#if data.leaks.length === 0}
      <div class="panel empty">
        {data.fine.length
          ? "Kein Wert liegt außerhalb seines Richtbereichs."
          : "Noch zu wenig Hände in dieser Auswahl. Ab 10 Gelegenheiten je Spot vergleicht Ringside deine Werte."}
      </div>
    {:else}
      <div class="list">
        {#each data.leaks as l (`${l.stat}|${l.game}|${l.positions.join()}|${l.band}`)}
          {@render leak(l)}
        {/each}
      </div>
    {/if}

    {#if data.fine.length}
      <details class="panel fine">
        <summary>Im Richtbereich ({int(data.fine.length)})</summary>
        <table class="table">
          <thead>
            <tr><th>Wert</th><th>Wo</th><th>Tiefe</th><th class="r">Du</th><th class="r">Richtwert</th><th class="r">Gelegenheiten</th></tr>
          </thead>
          <tbody>
            {#each data.fine as l (`${l.stat}|${l.game}|${l.positions.join()}|${l.band}`)}
              <tr>
                <td>{STAT_META[l.stat].name}</td>
                <td>{where(l)} · {game(l)}</td>
                <td>{data.bands[l.band]}</td>
                <td class="r num">{val(l, l.value)}</td>
                <td class="r num">{range(l)}</td>
                <td class="r num">{int(l.opp)}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </details>
    {/if}

    <section class="panel sources">
      <h2 class="panel-title">Quellen der Richtwerte</h2>
      <ul>
        {#each data.sources as s (s.id)}
          <li>
            {#if s.url}
              <a href={s.url} target="_blank" rel="noreferrer" onclick={(e) => open(s.url as string, e)}>{s.publisher}: {s.title}</a>
            {:else}
              <span>{s.publisher}: {s.title}</span>
            {/if}
            <small class="faint">{s.scope}</small>
          </li>
        {/each}
      </ul>
    </section>
  {/if}
</div>

<style>
  .leaks {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding-top: 20px;
  }
  .intro {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
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
  }
  .kpi small {
    font-size: 12px;
    color: var(--text-3);
  }
  .about {
    grid-column: 1 / -1;
    margin: 0;
    padding: 11px 18px 13px;
    border-top: 1px solid var(--line);
    font-size: 13px;
    color: var(--text-2);
    line-height: 1.5;
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .leak {
    padding: 14px 18px 14px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .leak.unsure {
    background: transparent;
    border-style: dashed;
  }
  .leak header {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    justify-content: space-between;
    gap: 6px 14px;
  }
  h3 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
  }
  .dir {
    margin-left: 6px;
    color: var(--sd);
    font-weight: 600;
  }
  .dir.hi {
    color: var(--amber);
  }
  .tags {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .tags span {
    padding: 2px 8px;
    border-radius: 999px;
    background: var(--surface-2);
    color: var(--text-2);
    font-size: 12px;
  }
  .tags .warn {
    background: transparent;
    border: 1px solid var(--amber);
    color: var(--amber);
    cursor: help;
  }
  .figures {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
    gap: 10px 18px;
  }
  .fig {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .fig[title] {
    cursor: help;
  }
  .fig b {
    font-size: 20px;
    font-weight: 600;
  }
  .fig small {
    font-size: 12px;
    color: var(--text-3);
  }
  .bar {
    position: relative;
    height: 14px;
    border-radius: 3px;
    background: var(--surface-2);
  }
  .bar .ref {
    position: absolute;
    top: 0;
    bottom: 0;
    background: color-mix(in srgb, var(--pos) 28%, transparent);
    border-left: 1px solid var(--pos);
    border-right: 1px solid var(--pos);
  }
  .bar .ci {
    position: absolute;
    top: 6px;
    height: 2px;
    background: var(--text-2);
  }
  .bar .dot {
    position: absolute;
    top: 2px;
    width: 10px;
    height: 10px;
    margin-left: -5px;
    border-radius: 50%;
    background: var(--text);
    box-shadow: 0 0 0 2px var(--surface);
  }
  .bar .dot.off {
    background: var(--amber);
  }
  footer {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 10px 16px;
  }
  .cites {
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: 12px;
    color: var(--text-2);
    min-width: 0;
  }
  .cites span[title] {
    cursor: help;
  }
  .links {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .fine summary {
    padding: 13px 18px;
    cursor: pointer;
    font-size: 13px;
    font-weight: 600;
  }
  .fine .table {
    margin-bottom: 6px;
  }
  .fine td,
  .fine th {
    padding: 7px 18px;
  }
  .r {
    text-align: right;
  }
  .sources {
    padding: 14px 18px 12px;
  }
  .sources ul {
    margin: 10px 0 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 8px;
    font-size: 13px;
  }
  .sources li {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .sources a {
    color: var(--text);
  }
  .sources small {
    font-size: 12px;
  }

  @media (max-width: 720px) {
    .intro {
      grid-template-columns: 1fr;
    }
    .kpi {
      border-left: 0;
      border-top: 1px solid var(--line);
    }
    .kpi:first-child {
      border-top: 0;
    }
    .fine .table th:nth-child(6),
    .fine .table td:nth-child(6) {
      display: none;
    }
    .fine td,
    .fine th {
      padding: 7px 10px;
    }
  }
</style>
