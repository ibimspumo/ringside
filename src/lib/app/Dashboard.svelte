<script lang="ts">
  import AnalyzeBar from "../analysis/AnalyzeBar.svelte";
  import Summary from "../analysis/Summary.svelte";
  import { analysis } from "../analysis/store.svelte";
  import { LEARN, SPOT, SPOT_HINT, bb } from "../analysis/labels";
  import type { SpotLoss, SpotType } from "../analysis/types";
  import { api } from "../api";
  import type { LearnTarget } from "./Leaks.svelte";
  import { PALETTES, RATED_STATS, STAT_META, bountyText, compact, fmt, stat } from "../stats";
  import type { Dashboard, Filter, GroupView, HudConfig, ResultPoint, StatId } from "../types";
  import { int, sign, signed } from "./format";
  import ResultsChart, { type Series } from "./ResultsChart.svelte";
  import Button from "../ui/Button.svelte";
  import EmptyState from "../ui/EmptyState.svelte";
  import Panel from "../ui/Panel.svelte";
  import Term from "../ui/Term.svelte";
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
  const BB_HELP = "BB = Big Blind, der große Pflichteinsatz. Ergebnisse in Big Blinds lassen sich über alle Limits und Turnierstufen vergleichen.";
  const WINRATE_HELP =
    "Winrate: wie viele Big Blinds du im Schnitt je 100 Hände gewinnst (BB/100). Über 0 heißt: Du gewinnst auf Dauer, unter 0: Du verlierst.";
  const SD_HELP = "Showdown: Am Ende der Hand werden die Karten aufgedeckt. Diese Linie zeigt, was du in solchen Händen gewonnen oder verloren hast.";
  const NSD_HELP =
    "Ohne Showdown: Pots, die enden, bevor Karten aufgedeckt werden – weil jemand foldet. Steigt die Linie, gewinnst du Pots durch Druck; fällt sie, gibst du oft auf.";
  const BOUNTY_HELP = "Bounty: Geld, das du bekommst, wenn du in einem Knockout-Turnier einen Spieler rauswirfst.";
  const POSITION_HELP: Record<string, string> = {
    UTG: "Under the Gun: als Erster vor dem Flop dran, nach dem Flop fast immer zuerst. Schwerste Position – hier nur starke Hände.",
    "UTG+1": "Direkt nach UTG: früh dran, fast so schwer wie UTG.",
    LJ: "Lojack: mittlere Position, drei Plätze vor dem Button.",
    HJ: "Hijack: zwei Plätze vor dem Button, schon recht späte Position.",
    CO: "Cutoff: direkt vor dem Button. Hier lohnen sich viele Hände.",
    BTN: "Button: nach dem Flop immer als Letzter dran. Beste Position am Tisch – hier gewinnen die meisten Spieler am meisten.",
    SB: "Small Blind: Pflichteinsatz und nach dem Flop zuerst dran. Fast alle Spieler verlieren hier auf Dauer.",
    BB: "Big Blind: großer Pflichteinsatz. Ein Minus ist hier normal; wichtig ist, nicht mehr als nötig zu verlieren.",
  };
  /** Below this many hands a winrate mostly shows the cards that came. */
  const FEW_HANDS = 30_000;

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
  let positions = $state<GroupView[]>([]);
  /** Stats the leak finder flags for some position or stack depth, with where. */
  let flagged = $state<Map<StatId, string>>(new Map());
  /** The spot type that costs the most, for the next step at the top. */
  let next = $state<SpotLoss | null>(null);
  let failed = $state(false);
  let retry = $state(0);
  /** The curve fills whatever height the answer column gives the row. */
  let plotH = $state(0);

  $effect(() => {
    void refresh;
    void retry;
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
    void analysis.status?.counts.analyzed;
    api.analysisSummary(f).then(
      (s) => live && (next = s.by_spot.find((x) => x.spot && x.loss_bb > 0.5) ?? null),
      () => live && (next = null),
    );
    api.leaks(f).then(
      (rep) => {
        if (!live) return;
        const m = new Map<StatId, string>();
        for (const l of rep.leaks) {
          if (l.uncertain || m.has(l.stat)) continue;
          const band = (rep.bands[l.band] ?? "").replaceAll(" ", "\u00a0").replaceAll("–", "\u2060–\u2060");
          m.set(l.stat, `${l.positions.length ? `bei ${l.positions.join(", ")}` : "über alle Positionen"} · ${band}`);
        }
        flagged = m;
      },
      () => live && (flagged = new Map()),
    );
    api.reports(f).then(
      (rep) => live && (positions = rep.positions),
      () => live && (positions = []),
    );
    return () => (live = false);
  });

  const r = $derived(data?.results ?? null);
  const bb100 = $derived(r && r.hands > 0 ? (r.net_bb / r.hands) * 100 : 0);
  const hero = $derived(data?.hero ?? null);
  const heroHands = $derived(hero ? (stat(hero.stats, "hands")?.value ?? 0) : 0);
  const palette = $derived(hud ? (PALETTES[hud.palette] ?? PALETTES.traffic) : PALETTES.traffic);
  const bounties = $derived(r ? bountyText(r.bounties) : "");
  /** While the analysis still runs, the losses below are a snapshot. */
  const running = $derived(analysis.status?.progress?.state === "running");
  const counts = $derived(analysis.status?.counts);
  const preflopDone = $derived(!!counts && counts.hands > 0 && counts.analyzed >= counts.hands);
  const byPosition = $derived(
    positions
      .map((p) => {
        const hands = stat(p.stats, "hands")?.value ?? 0;
        return { key: p.key, hands, net: p.net_bb, rate: hands > 0 ? (p.net_bb / hands) * 100 : 0 };
      })
      .filter((p) => p.hands > 0),
  );
  const rateMax = $derived(Math.max(1, ...byPosition.map((p) => Math.abs(p.rate))));
  const plain = (x: number) => Math.abs(x).toLocaleString("de-DE", { maximumFractionDigits: 1 });

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

  function zone(g: number): { label: string; color: string } {
    if (g < 1 / 3) return { label: "niedrig", color: palette.low };
    if (g <= 2 / 3) return { label: "normal", color: palette.mid };
    return { label: "hoch", color: palette.high };
  }
</script>

{#snippet nextStep(spot: SpotType, loss: number)}
  <div class="next">
    <span class="next-label"
      >Dein nächster Schritt{#if running}<span class="early"
          ><Term tip="Die Analyse läuft noch. Mit jeder bewerteten Hand kann sich die teuerste Baustelle ändern.">vorläufig</Term></span
        >{/if}</span
    >
    <p><b>{SPOT[spot]}</b> kostet dich laut Hand-Analyse am meisten: {bb(loss)}. {SPOT_HINT[spot]}</p>
    {#if onLearn}
      <Button variant="primary" onclick={() => onLearn(LEARN[spot][0].target)}>{LEARN[spot][0].label}</Button>
    {/if}
  </div>
{/snippet}

<div class="page dash">
  {#if failed}
    <Panel>
      <EmptyState
        tone="error"
        title="Die Übersicht konnte nicht geladen werden."
        text="Deine Hände sind sicher gespeichert. Versuch es noch einmal; bleibt der Fehler, starte Ringside neu."
      >
        {#snippet action()}<Button variant="primary" onclick={() => retry++}>Erneut laden</Button>{/snippet}
      </EmptyState>
    </Panel>
  {:else if data && r && r.hands === 0}
    <Panel>
      <EmptyState
        title="Noch keine Hände in dieser Auswahl"
        text="Spiel ein paar Hände bei PokerStars – Ringside liest sie von selbst ein.{filter.since !== null || filter.game !== null
          ? ' Hast du schon gespielt, wähle oben „Alles“ und „Alle“.'
          : ''}"
      />
    </Panel>
  {:else if data && r}
    <section class="panel hero">
      <div class="answer">
        <span class="eyebrow">Dein Ergebnis</span>
        <strong class="big num {sign(r.net_bb)}">{signed(r.net_bb)}<span class="big-unit"><Term tip={BB_HELP}>BB</Term></span></strong>
        <p class="say">
          {r.net_bb >= 0 ? "Du liegst im Plus" : "Du liegst im Minus"} – nach {int(r.hands)} Händen{data.heroes.length
            ? ` als ${data.heroes.join(", ")}`
            : ""}.
          {#if r.hands < FEW_HANDS}<span class="chance">Bei so wenigen Händen spielt der Zufall noch stark mit.</span>{/if}
        </p>
        {#if next?.spot}<div class="next-narrow">{@render nextStep(next.spot, next.loss_bb)}</div>{/if}

        <dl class="facts">
          <div>
            <dt><Term tip={WINRATE_HELP}>Winrate</Term></dt>
            <dd class="num {sign(bb100)}">{signed(bb100, 2)}<span class="unit">BB/100</span></dd>
            <small>{bb100 >= 0 ? "Du gewinnst im Schnitt" : "Du verlierst im Schnitt"} {plain(bb100)} BB je 100 Hände.</small>
          </div>
          <div>
            <dt><Term tip="{LUCK_HELP} {EV_HELP}">Glück</Term></dt>
            <dd class="num">{r.all_ins ? signed(r.luck_bb) : "–"}<span class="unit">BB</span></dd>
            <small>
              {#if !r.all_ins}
                Noch kein All-in mit offenen Karten.
              {:else if r.luck_bb >= 0}
                In {int(r.all_ins)} All-ins mehr gewonnen als erwartet. Mit erwarteten All-in-Ergebnissen: {signed(r.ev_bb)} BB.
              {:else}
                In {int(r.all_ins)} All-ins weniger gewonnen als erwartet – Pech, kein Fehler.
              {/if}
            </small>
          </div>
          <div>
            <dt><Term tip={NSD_HELP}>Ohne Showdown</Term></dt>
            <dd class="num {sign(r.non_showdown_bb)}">{signed(r.non_showdown_bb)}<span class="unit">BB</span></dd>
            <small>
              {r.non_showdown_bb >= 0 ? "Du gewinnst Pots, ohne Karten zu zeigen." : "Du gibst Pots oft kampflos ab."}
              Mit Showdown: <span class="num {sign(r.showdown_bb)}">{signed(r.showdown_bb)} BB</span>
            </small>
          </div>
          {#if filter.game !== "cash"}
            <div>
              <dt><Term tip={BOUNTY_HELP}>Bounties</Term></dt>
              <dd class="num bounties" class:pos={!!bounties}>
                {#each bounties ? bounties.split(" · ") : ["–"] as b, i (b)}<span class:more={i > 0}>{b}</span>{/each}
              </dd>
              <small>{bounties ? "Zusätzlich gewonnen durch Knockouts." : "Noch kein Knockout-Geld."}</small>
            </div>
          {/if}
        </dl>
      </div>

      <div class="curve">
        <div class="curve-head">
          <h2 class="curve-title">So kam es dazu</h2>
          <div class="legend">
            <span><i class="sw total"></i>Gesamt <b class="num {sign(r.net_bb)}">{signed(r.net_bb)}</b></span>
            <span><i class="sw ev"></i><Term side="bottom" tip={EV_HELP}>All-in-EV</Term> <b class="num {sign(r.ev_bb)}">{signed(r.ev_bb)}</b></span>
            <span><i class="sw sd"></i><Term side="bottom" tip={SD_HELP}>Mit Showdown</Term> <b class="num {sign(r.showdown_bb)}">{signed(r.showdown_bb)}</b></span>
            <span
              ><i class="sw nsd"></i><Term side="bottom" tip={NSD_HELP}>Ohne Showdown</Term>
              <b class="num {sign(r.non_showdown_bb)}">{signed(r.non_showdown_bb)}</b></span
            >
          </div>
        </div>
        <div class="plot" bind:clientHeight={plotH}>
          <div class="fill"><ResultsChart points={r.points} series={SERIES} height={Math.max(280, plotH)} /></div>
        </div>
      </div>
    </section>

    {#if next?.spot}<div class="next-wide">{@render nextStep(next.spot, next.loss_bb)}</div>{/if}

    <section class="loss">
      <div class="section-head">
          <h2 class="section-title">Wo du Chips verlierst</h2>
          {#if running}
            <span class="snapshot"
              >{preflopDone
                ? "Zwischenstand – nach dem Flop kommen noch Entscheidungen dazu."
                : "Zwischenstand – die Zahlen wachsen, solange die Analyse läuft."}</span
            >
          {/if}
      </div>
      <AnalyzeBar compact title="Analyse starten" />
      <Summary {filter} {refresh} onOpen={(id, at) => onHand(id, at)} onSpot={() => onHand(-1, null)} {onLearn} />
    </section>

    {#if byPosition.length}
      <Panel
        flush
        title="Wo du gewinnst – nach Position"
        sub="Big Blinds je 100 Hände auf jedem Platz am Tisch. In den Blinds verlieren fast alle, weil dort Pflichteinsätze liegen; auf Button und Cutoff solltest du am meisten gewinnen."
      >
        <ul class="positions">
          <li class="pos-head" aria-hidden="true">
            <span>Platz</span><span class="r">Hände</span><span></span><span class="r">je 100 Hände</span><span class="r">gesamt</span>
          </li>
          {#each byPosition as p (p.key)}
            {@const w = (Math.abs(p.rate) / rateMax) * 50}
            <li>
              <span class="pos-name">{#if POSITION_HELP[p.key]}<Term tip={POSITION_HELP[p.key]}>{p.key}</Term>{:else}{p.key}{/if}</span>
              <span class="pos-hands num">{int(p.hands)} Hände</span>
              <span class="pos-bar" aria-hidden="true">
                <i class={p.rate >= 0 ? "up" : "down"} style="{p.rate >= 0 ? 'left' : 'right'}:50%; width:{w}%"></i>
              </span>
              <b class="num {sign(p.rate)}">{signed(p.rate, 1)}<span class="unit">BB/100</span></b>
              <span class="pos-net num">{signed(p.net)} BB</span>
            </li>
          {/each}
        </ul>
      </Panel>
    {/if}

    <Panel
      flush
      title="Dein Spielstil"
      sub="Wie oft du was tust (gemacht von Gelegenheiten), gemessen an deinen HUD-Schwellen. „Hoch“ oder „niedrig“ ist kein Fehler – ob ein Wert dich Geld kostet, prüft die Seite Leaks."
    >
      {#if hero && hud}
        <div class="style">
          <div class="who">
            <Wheel stats={hero.stats} {hud} size={64} hands={compact(heroHands)} />
            <div>
              <strong title={hero.name}>{hero.name}</strong>
              <span class="num who-hands">{int(heroHands)} Hände</span>
            </div>
          </div>
          <div class="stats">
            {#each RATED_STATS as id (id)}
              {@const s = stat(hero.stats, id)}
              {@const g = gauge(id, s?.value ?? null)}
              {@const z = g === null ? null : zone(g)}
              <div class="stat">
                <span class="name"><Term tip="{STAT_META[id].name}: {STAT_META[id].help}">{STAT_META[id].name}</Term></span>
                <span class="value">
                  <b class="num" class:none={g === null && !s?.opp}
                    >{fmt(s)}{#if s && s.value !== null && !STAT_META[id].ratio}<span class="unit">%</span>{/if}</b
                  >
                  {#if z}<em>{flagged.get(id) ? `gesamt ${z.label}` : z.label}</em>{/if}
                </span>
                <span class="gauge" aria-hidden="true" style="--lo:{palette.low}; --mid:{palette.mid}; --hi:{palette.high}">
                  <i></i><i></i><i></i>
                  {#if g !== null}<em style="left:{g * 100}%"></em>{/if}
                </span>
                <small
                  >{STAT_META[id].ratio
                    ? `${int(s?.made ?? 0)} Bets/Raises zu ${int(s?.opp ?? 0)} Calls`
                    : `${int(s?.made ?? 0)} von ${int(s?.opp ?? 0)}`}</small
                >
                {#if flagged.get(id)}
                  <small class="leak-hint"
                    ><Term tip="Die Seite Leaks meldet diesen Wert an einer Stelle als Abweichung vom Richtwert: {flagged.get(id)}."
                      >Leak {flagged.get(id)}</Term
                    ></small
                  >
                {/if}
              </div>
            {/each}
          </div>
        </div>
      {:else}
        <EmptyState compact title="Noch keine eigenen Hände in dieser Auswahl" text="Ringside erkennt deinen Namen automatisch aus den Hand-Historys." />
      {/if}
    </Panel>
  {/if}
</div>

<style>
  .dash {
    padding-inline: 24px;
    display: flex;
    flex-direction: column;
    gap: 18px;
    padding-top: 20px;
  }
  /* The answer and the curve that explains it. */
  .hero {
    display: grid;
    grid-template-columns: minmax(300px, 380px) minmax(0, 1fr);
  }
  .answer {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 20px 22px 18px;
    border-right: 1px solid var(--line);
  }
  .eyebrow {
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .big {
    font-size: var(--fs-hero);
    font-weight: 600;
    line-height: 1.05;
    letter-spacing: -0.01em;
    white-space: nowrap;
  }
  .big-unit {
    margin-left: 3px;
    color: var(--text-3);
    font-size: 0.5em;
    font-weight: var(--fw-medium);
  }
  .say {
    margin: 2px 0 0;
    font-size: var(--fs-base);
    line-height: 1.5;
    color: var(--text);
  }
  .say .chance {
    display: block;
    margin-top: 2px;
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .next {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: 6px 18px;
    padding: 14px 18px;
    border: 1px solid var(--line);
    border-left: 3px solid var(--text);
    border-radius: var(--r-lg);
    background: var(--surface);
  }
  .next-label {
    font-size: var(--fs-md);
    font-weight: 600;
    color: var(--text-2);
    white-space: nowrap;
  }
  .next p {
    margin: 0;
    font-size: var(--fs-base);
    line-height: 1.45;
    color: var(--text-2);
  }
  .next p b {
    color: var(--text);
  }
  .next-narrow {
    display: none;
    margin-top: var(--sp-3);
  }
  /* Narrow pages: the next step follows the result instead of waiting below the chart. */
  @container (max-width: 860px) {
    .next-wide {
      display: none;
    }
    .next-narrow {
      display: block;
    }
    .next {
      grid-template-columns: minmax(0, 1fr) auto;
    }
    .next-label {
      grid-column: 1 / -1;
    }
  }
  .bounties {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
  }
  .bounties .more {
    font-size: var(--fs-md);
    font-weight: var(--fw-medium);
  }
  .early {
    margin-left: var(--sp-2);
    color: var(--amber);
    font-weight: var(--fw-regular);
  }
  .facts {
    display: flex;
    flex-direction: column;
    margin: 14px 0 0;
    border-top: 1px solid var(--line);
  }
  .facts div {
    align-content: start;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: baseline;
    column-gap: 12px;
    padding: 10px 0 9px;
    border-bottom: 1px solid var(--line);
  }
  .facts div:last-child {
    border-bottom: 0;
    padding-bottom: 0;
  }
  .facts dt {
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .facts dd {
    margin: 0;
    font-size: var(--fs-xl);
    font-weight: 600;
    white-space: nowrap;
  }
  .facts small {
    grid-column: 1 / -1;
    margin-top: 2px;
    font-size: var(--fs-md);
    line-height: 1.45;
    color: var(--text-2);
  }
  .curve {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
    padding: 16px 14px 10px 4px;
  }
  .plot {
    position: relative;
    flex: 1;
    min-height: 280px;
  }
  .fill {
    position: absolute;
    inset: 0;
  }
  .curve-title {
    margin: 0;
    font-size: var(--fs-md);
    font-weight: var(--fw-semibold);
  }
  .curve-head {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    justify-content: space-between;
    gap: 6px 16px;
    padding-left: 18px;
  }
  .legend {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 16px;
    font-size: var(--fs-md);
    color: var(--text-2);
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

  .legend b {
    margin-left: 4px;
    font-weight: 600;
  }
  .positions {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 2px;
    margin: 0;
    padding: 12px 20px 16px;
    list-style: none;
  }
  .positions li {
    display: grid;
    grid-template-columns: 52px 92px minmax(80px, 1fr) 112px 92px;
    align-items: center;
    gap: 12px;
    height: 32px;
    border-bottom: 1px solid var(--line);
    font-size: var(--fs-md);
  }
  .positions li.pos-head {
    height: 24px;
    border-bottom-color: var(--line-2);
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .pos-head .r {
    text-align: right;
  }
  .pos-name {
    font-weight: 600;
  }
  .pos-hands,
  .pos-net {
    color: var(--text-2);
    font-size: var(--fs-md);
    text-align: right;
    white-space: nowrap;
  }
  .pos-bar {
    position: relative;
    height: 10px;
    border-radius: 3px;
    background: var(--surface-2);
  }
  .pos-bar::after {
    content: "";
    position: absolute;
    left: 50%;
    top: -3px;
    bottom: -3px;
    width: 1px;
    background: var(--line-2);
  }
  .pos-bar i {
    position: absolute;
    top: 0;
    bottom: 0;
    animation: grow-x var(--dur-slow) var(--ease-enter) both;
  }
  .pos-bar i.up {
    transform-origin: left;
    border-radius: 0 3px 3px 0;
    background: color-mix(in srgb, var(--pos) 80%, transparent);
  }
  .pos-bar i.down {
    transform-origin: right;
    border-radius: 3px 0 0 3px;
    background: color-mix(in srgb, var(--neg) 80%, transparent);
  }
  .positions b {
    font-size: var(--fs-lg);
    font-weight: 600;
    text-align: right;
    white-space: nowrap;
  }
  .loss {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .section-head {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 4px 14px;
    margin-top: 6px;
  }
  .snapshot {
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .section-title {
    margin: 0;
    font-family: var(--font-num);
    font-size: var(--fs-lg);
    font-weight: 600;
  }

  .style {
    display: grid;
    grid-template-columns: 196px minmax(0, 1fr);
    margin-top: 14px;
    border-top: 1px solid var(--line);
    overflow: hidden;
    border-radius: 0 0 var(--r-lg) var(--r-lg);
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
    font-size: var(--fs-lg);
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .who-hands {
    color: var(--text-2);
    font-size: var(--fs-md);
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
    min-width: 0;
  }
  .stat .name {
    min-height: 2.7em;
    font-size: var(--fs-md);
    line-height: 1.35;
    color: var(--text-2);
  }
  .value {
    display: flex;
    align-items: baseline;
    gap: 8px;
  }
  .stat b {
    font-size: var(--fs-2xl);
    font-weight: 600;
    line-height: 1.2;
    color: var(--text);
  }
  .stat b.none {
    color: var(--text-3);
  }
  .value em {
    font-style: normal;
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .stat .leak-hint {
    color: var(--amber);
    white-space: normal;
    line-height: 1.35;
  }
  .gauge {
    position: relative;
    display: flex;
    gap: 1px;
    width: 72px;
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
    font-size: var(--fs-md);
    line-height: 1.35;
    color: var(--text-2);
  }

  /* Wide windows: the page grows so the curve and the style row use the room. */
  @container (min-width: 1900px) {
    .dash {
      max-width: none;
    }
    .hero {
      grid-template-columns: 420px minmax(0, 1fr);
    }
    .stats {
      grid-template-columns: repeat(12, minmax(0, 1fr));
    }
  }
  @container (max-width: 1100px) {
    .stats {
      grid-template-columns: repeat(4, minmax(0, 1fr));
    }
  }
  @container (max-width: 860px) {
    .hero {
      grid-template-columns: 1fr;
    }
    .answer {
      border-right: 0;
      border-bottom: 1px solid var(--line);
    }
    .facts {
      display: grid;
      grid-template-columns: repeat(2, minmax(0, 1fr));
      column-gap: 24px;
    }
    .facts div:nth-last-child(2) {
      border-bottom: 0;
    }
    .style {
      grid-template-columns: 1fr;
    }
    .who {
      border-right: 0;
      border-bottom: 1px solid var(--line);
    }
  }
</style>
