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

  /** What a value measures, what a deviation usually means, and where to start. */
  const EXPLAIN: Partial<Record<StatId, { what: string; high: string; low: string; tryHigh: string; tryLow: string }>> = {
    vpip: {
      what: "Wie oft du vor dem Flop freiwillig Chips in den Pot legst, also callst oder erhöhst. Die Blinds zählen nicht.",
      high: "Du spielst zu viele Hände. Schwache Hände landen später oft gegen bessere.",
      low: "Du spielst zu wenige Hände und lässt Gelegenheiten liegen, vor allem aus später Position.",
      tryHigh: "Aus früher Position die schwächsten Hände folden, Hände nach Position auswählen.",
      tryLow: "Aus Cutoff, Button und Small Blind mehr Hände öffnen.",
    },
    pfr: {
      what: "Wie oft du vor dem Flop erhöhst.",
      high: "Du erhöhst sehr viele Hände. Aufmerksame Gegner setzen dich mit 3-Bets unter Druck.",
      low: "Du callst oft, wo ein Raise besser wäre, und gibst damit die Initiative ab.",
      tryHigh: "Aus früher Position enger öffnen.",
      tryLow: "Hände, die du spielst, lieber erhöhen als callen, vor allem als Erster im Pot.",
    },
    three_bet: {
      what: "Wie oft du gegen einen Open-Raise noch einmal erhöhst (Re-Raise).",
      high: "Du 3-bettest sehr viel. Gegner callen oder 4-betten dich leichter.",
      low: "Du 3-bettest kaum. Deine Re-Raises zeigen fast immer ein Monster, und du gewinnst selten Pots vor dem Flop.",
      tryHigh: "Schwächere 3-Bets durch Calls oder Folds ersetzen.",
      tryLow: "Neben starken Händen auch ein paar Hände mit Blockern (z. B. A5s) 3-betten.",
    },
    fold_to_three_bet: {
      what: "Wie oft du nach deinem Raise foldest, wenn jemand erneut erhöht.",
      high: "Du gibst nach einem Re-Raise zu oft auf. Gegner können dich mit Bluff-3-Bets ausnutzen.",
      low: "Du gehst gegen 3-Bets sehr oft mit, auch mit Händen, die in einem großen Pot schlecht spielen.",
      tryHigh: "Die besseren Hände deiner Open-Range gegen 3-Bets verteidigen (callen oder 4-betten).",
      tryLow: "Schwache Opens gegen 3-Bets aufgeben, besonders ohne Position.",
    },
    steal: {
      what: "Wie oft du aus Cutoff, Button oder Small Blind erhöhst, wenn alle vor dir gefoldet haben.",
      high: "Du stiehlst sehr breit. Die Blinds wehren sich dann gern mit 3-Bets.",
      low: "Du lässt leichte Pots liegen: Aus später Position gewinnt ein Raise oft sofort Blinds und Antes.",
      tryHigh: "Gegen Blinds, die viel 3-betten, etwas enger stehlen.",
      tryLow: "Vom Button und aus dem Cutoff deutlich mehr Hände öffnen, mit Antes erst recht.",
    },
    fold_to_steal: {
      what: "Wie oft du im Small oder Big Blind foldest, wenn jemand aus später Position erhöht.",
      high: "Du gibst deine Blinds zu leicht her. Gegner stehlen dann gewinnbringend gegen dich.",
      low: "Du verteidigst sehr breit, auch mit Händen, die ohne Position schwer zu spielen sind.",
      tryHigh: "Im Big Blind mehr Hände callen: Du hast schon gezahlt und bekommst gute Pot Odds.",
      tryLow: "Die schwächsten Verteidigungen weglassen, vor allem im Small Blind.",
    },
    cbet: {
      what: "Wie oft du am Flop setzt, wenn du vor dem Flop als Letzter erhöht hast (Continuation-Bet).",
      high: "Du setzt fast immer nach. Wenn das Board nicht zu dir passt, callen oder raisen Gegner leichter.",
      low: "Du gibst die Initiative oft ab und lässt Pots liegen, die eine kleine Bet gewonnen hätte.",
      tryHigh: "Auf Boards, die dem Caller helfen (verbunden, mittelhoch), öfter checken.",
      tryLow: "Auf trockenen Boards mit hoher Karte öfter klein setzen.",
    },
    fold_to_cbet: {
      what: "Wie oft du am Flop foldest, wenn der Raiser vor dem Flop nachsetzt.",
      high: "Du gibst zu oft auf, sobald der Raiser setzt. Jede C-Bet wird gegen dich profitabel.",
      low: "Du callst C-Bets sehr oft, auch ohne genug Equity für die Pot Odds.",
      tryHigh: "Gegen kleine C-Bets mit Draws und Overcards öfter weiterspielen.",
      tryLow: "Mit Luft und schwachen Draws auch mal folden, die Pot Odds geben den Ausschlag.",
    },
    wtsd: {
      what: "Wie oft du bis zum Showdown kommst, wenn du den Flop gesehen hast.",
      high: "Du gehst oft bis zum Ende mit, meist durch Calls auf Turn und River mit mittelstarken Händen.",
      low: "Du gibst oft vor dem Showdown auf. Gegner können dich mit späten Bluffs zum Folden bringen.",
      tryHigh: "Auf Turn und River gegen große Bets genauer prüfen, ob der Call sich lohnt.",
      tryLow: "Mit Händen, die oft vorne sind, öfter bis zum Ende mitgehen.",
    },
    wsd: {
      what: "Wie viele deiner Showdowns du gewinnst.",
      high: "Du gewinnst fast jeden Showdown. Vermutlich gehst du nur mit sehr starken Händen hin und gibst vorher zu oft auf.",
      low: "Du verlierst viele Showdowns, oft ein Zeichen für zu viele Calls mit zweitbesten Händen.",
      tryHigh: "Mit mittleren Händen öfter bis zum Showdown mitgehen.",
      tryLow: "Mit zweitbesten Händen gegen starke Linien öfter folden.",
    },
    af: {
      what: "Wie oft du nach dem Flop setzt oder erhöhst, im Verhältnis zu deinen Calls.",
      high: "Sehr aggressiv: Du setzt und erhöhst viel und callst selten.",
      low: "Passiv: Du callst viel und setzt selten, und gewinnst deshalb seltener Pots ohne Showdown.",
      tryHigh: "Mit mittelstarken Händen öfter checken oder callen, statt zu setzen.",
      tryLow: "Starke Hände und gute Draws öfter selbst setzen oder erhöhen, statt nur zu callen.",
    },
  };

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
  {@const explain = EXPLAIN[l.stat]}
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

    {#if explain}
      <div class="explain">
        <p><b>Was ist das?</b> {explain.what}</p>
        <p>
          <b>{l.direction === "too_high" ? "Zu hoch heißt meist:" : "Zu niedrig heißt meist:"}</b>
          {l.direction === "too_high" ? explain.high : explain.low}
        </p>
        <p><b>Ansatz:</b> {l.direction === "too_high" ? explain.tryHigh : explain.tryLow}</p>
      </div>
    {/if}

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
    <div class="panel empty-state">Die Daten konnten nicht geladen werden. Starte Ringside neu, falls das so bleibt.</div>
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
      <div class="about">
        <p>
          Ringside vergleicht deine Werte je Position und Stacktiefe mit veröffentlichten Richtwerten. Das sind Orientierungen
          aus den unten genannten Quellen, keine „richtige“ Strategie. Sortiert nach Sicherheit × Abweichung.
        </p>
        <dl class="legend">
          <div><dt><span class="key ref"></span>Grüner Bereich</dt><dd>der Richtwert aus den Quellen</dd></div>
          <div><dt><span class="key dot"></span>Punkt</dt><dd>dein Wert</dd></div>
          <div>
            <dt><span class="key ci"></span>Linie</dt>
            <dd>wo dein wahrer Wert mit 95 % liegt. Je weniger Hände, desto länger die Linie.</dd>
          </div>
          <div>
            <dt>Sicherheit</dt>
            <dd>wie sicher dein wahrer Wert außerhalb des Richtwerts liegt. Ab 90 % lohnt es sich, daran zu arbeiten.</dd>
          </div>
          <div><dt>20–40 BB …</dt><dd>dein effektiver Stack in der Hand; Turnier und Cash werden getrennt geprüft.</dd></div>
        </dl>
      </div>
    </section>

    {#if data.leaks.length === 0}
      <div class="panel empty-state">
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
  .about p {
    margin: 0;
  }
  .legend {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 22px;
    margin: 10px 0 0;
    font-size: 12.5px;
  }
  .legend div {
    display: flex;
    gap: 6px;
    align-items: baseline;
  }
  .legend dt {
    display: flex;
    gap: 6px;
    align-items: center;
    color: var(--text);
    font-weight: 600;
    white-space: nowrap;
  }
  .legend dd {
    margin: 0;
  }
  .key {
    display: inline-block;
    flex: none;
  }
  .key.ref {
    width: 18px;
    height: 10px;
    border-left: 1px solid var(--pos);
    border-right: 1px solid var(--pos);
    background: color-mix(in srgb, var(--pos) 28%, transparent);
  }
  .key.dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: var(--amber);
  }
  .key.ci {
    width: 18px;
    height: 2px;
    background: var(--text-2);
  }
  .explain {
    display: grid;
    gap: 4px;
    margin-top: 12px;
    padding: 10px 12px;
    border-radius: var(--r-sm);
    background: var(--surface-2);
    font-size: 13px;
    line-height: 1.5;
    color: var(--text-2);
  }
  .explain p {
    margin: 0;
  }
  .explain b {
    color: var(--text);
    font-weight: 600;
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
