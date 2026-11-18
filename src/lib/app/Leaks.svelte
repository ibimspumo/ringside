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
  import Badge from "../ui/Badge.svelte";
  import Button from "../ui/Button.svelte";
  import IconButton from "../ui/IconButton.svelte";
  import EmptyState from "../ui/EmptyState.svelte";
  import Panel from "../ui/Panel.svelte";
  import Tag from "../ui/Tag.svelte";
  import Term from "../ui/Term.svelte";
  import { slide } from "svelte/transition";
  import { cubicOut } from "svelte/easing";

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

  /** Under 10 BB a steal is an all-in, and defending means calling one. */
  const SHORT: Partial<Record<StatId, { what: string; high: string; low: string; tryHigh: string; tryLow: string }>> = {
    steal: {
      what: "Wie oft du mit unter 10 BB all-in gehst, wenn alle vor dir gefoldet haben. So kurz gibt es meist nur All-in oder Fold.",
      high: "Du gehst mit zu vielen Händen all-in. Die Blinds callen dann mit besseren Händen.",
      low: "Du foldest zu oft. Mit so wenig Chips verlierst du durch die Blinds und Antes mehr, als ein All-in riskiert.",
      tryHigh: "Die schwächsten All-ins weglassen; die Push/Fold-Tabellen zeigen die Grenze je Position.",
      tryLow: "Öfter all-in gehen, vor allem aus Small Blind und Button – die Push/Fold-Tabellen zeigen, mit welchen Händen.",
    },
    fold_to_steal: {
      what: "Wie oft du im Small oder Big Blind foldest, wenn jemand mit unter 10 BB all-in geht.",
      high: "Du foldest zu oft gegen kurze All-ins. Gegner gewinnen die Blinds dann fast umsonst.",
      low: "Du callst All-ins zu breit, auch mit Händen, die meist hinten liegen.",
      tryHigh: "Gegen kurze All-ins mit mehr Händen callen; die Pot Odds sind oft gut.",
      tryLow: "Nur mit Händen callen, die gegen eine All-in-Range vorne liegen.",
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
  function exercise(l: LeakCheck): { kind: LearnKind; level: number; why: string } | null {
    const short = l.game === "tournament" && l.band <= 1;
    const deep = l.band === 1 ? "Mit 10–20 BB" : "So kurz";
    if (short && (l.stat === "steal" || l.stat === "pfr" || l.stat === "vpip"))
      return { kind: "push_fold_open", level: 1, why: `${deep} heißt Eröffnen oft All-in – die Übung trainiert genau diese Wahl.` };
    if (short && l.stat === "fold_to_steal")
      return { kind: "push_fold_call", level: 1, why: `${deep} kommt der Steal oft als All-in – die Übung trainiert, wann du callen solltest.` };
    if (l.stat === "fold_to_cbet" || l.stat === "wtsd")
      return { kind: "pot_odds", level: 3, why: "Ob ein Call sich lohnt, entscheiden die Pot Odds – die Übung trainiert das Rechnen." };
    return null;
  }

  let data = $state<LeakReport | null>(null);
  let failed = $state(false);
  let retry = $state(0);
  let howOpen = $state(false);
  /** Null until the user toggles it: short lists start open. */
  let fineOpen = $state<boolean | null>(null);

  $effect(() => {
    void refresh;
    void retry;
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
  const first = $derived(sure[0] ?? null);

  const ratio = (l: LeakCheck) => !!STAT_META[l.stat].ratio;
  const num = (x: number, d = 1) => x.toLocaleString("de-DE", { minimumFractionDigits: d, maximumFractionDigits: d });
  const val = (l: LeakCheck, x: number) => (ratio(l) ? num(x, 2) : `${num(x)}\u00a0%`);
  const range = (l: LeakCheck) => (ratio(l) ? `${num(l.low)}–${num(l.high)}` : `${num(l.low, 0)}–${num(l.high, 0)}\u00a0%`);
  const gap = (l: LeakCheck) => (ratio(l) ? num(l.deviation, 2) : `${num(l.deviation)} Punkte`);
  const where = (l: LeakCheck) => (l.positions.length ? l.positions.join(", ") : "alle Positionen");
  const game = (l: LeakCheck) => (l.game === "tournament" ? "Turnier" : "Cash");
  const pct = (x: number) => (x >= 0.995 ? "über 99\u00a0%" : `${Math.round(x * 100)}\u00a0%`);
  const key = (l: LeakCheck) => `${l.stat}|${l.game}|${l.positions.join()}|${l.band}`;

  const BAND_HELP = "Dein effektiver Stack in der Hand, in Big Blinds: der kleinere der Stacks von dir und deinem Gegner. Turnier und Cash werden getrennt geprüft.";
  const SURE_HELP =
    "Wie sicher dein wahrer Wert außerhalb des Richtwerts liegt (aus dem Wilson-Intervall). Ab 90 % lohnt es sich, daran zu arbeiten.";
  const COST_HELP =
    "Grob: dein Ergebnis in diesem Spot gegenüber deinem Schnitt auf denselben Positionen über alle Stacktiefen. Das zeigt, was der Spot gekostet hat, nicht sicher, dass dieser eine Wert die Ursache ist.";

  /** Position on the bar from 0 to 1: percentages on 0–100, AF on 0–6 or wider. */
  function scale(l: LeakCheck) {
    const max = ratio(l) ? Math.max(6, Math.ceil(Math.min(l.ci_high, 20))) : 100;
    return { max, x: (x: number) => Math.min(1, Math.max(0, x / max)) };
  }
  /** Keeps a label above or below the bar inside the bar at both ends. */
  const edge = (x: number) => (x < 0.12 ? "start" : x > 0.88 ? "end" : "mid");

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

{#snippet bar(l: LeakCheck, small = false)}
  {@const sc = scale(l)}
  {@const x = sc.x}
  {@const mid = (x(l.low) + x(l.high)) / 2}
  <div class="meter" class:small>
    {#if !small}<span class="end num">0{ratio(l) ? "" : " %"}</span>{/if}
    <div class="track-wrap">
      {#if !small}
        <span class="lab top {edge(mid)}" style="left:{mid * 100}%">Richtwert <b class="num">{range(l)}</b></span>
      {/if}
      <div class="track" aria-hidden="true">
        <span class="ref" style="left:{x(l.low) * 100}%; width:{(x(l.high) - x(l.low)) * 100}%"></span>
        <span class="ci" style="left:{x(l.ci_low) * 100}%; width:{Math.max(0.4, (x(l.ci_high) - x(l.ci_low)) * 100)}%"></span>
        <span class="dot" class:off={l.direction !== null} style="left:{x(l.value) * 100}%"></span>
      </div>
      {#if !small}
        <span class="lab bottom {edge(x(l.value))}" class:off={l.direction !== null} style="left:{x(l.value) * 100}%"
          >Du <b class="num">{val(l, l.value)}</b></span
        >
      {/if}
    </div>
    {#if !small}<span class="end num">{ratio(l) ? sc.max : "100 %"}</span>{/if}
  </div>
{/snippet}

{#snippet leak(l: LeakCheck, rank: number | null)}
  {@const article = ARTICLE[l.stat]}
  {@const title = article ? ARTICLES.find((a) => a.id === article)?.title : undefined}
  {@const ex = exercise(l)}
  {@const explain = (l.game === "tournament" && l.band === 0 ? SHORT[l.stat] : undefined) ?? EXPLAIN[l.stat]}
  {@const high = l.direction === "too_high"}
  <article class="leak panel" class:unsure={l.uncertain}>
    <header>
      <div class="name">
        {#if rank !== null}<span class="rank num">{rank}</span>{/if}
        <h3>{STAT_META[l.stat].name}</h3>
        <Badge tone={high ? "warn" : "info"}>{high ? "zu hoch" : "zu niedrig"}</Badge>
        {#if l.uncertain}
          <Badge tone="warn"
            ><Term tip="Unter 30 Gelegenheiten oder weniger als 90 % sicher. Spiel mehr Hände, bevor du daran arbeitest.">noch unsicher</Term></Badge
          >
        {/if}
      </div>
      <div class="tags">
        <Tag>{where(l)}</Tag>
        <Tag><Term tip={BAND_HELP}>{data?.bands[l.band]}</Term></Tag>
        <Tag>{game(l)}</Tag>
      </div>
    </header>

    <div class="body">
      <div class="measure">
        {@render bar(l)}
        <div class="figures">
          <div class="fig">
            <span class="label">Abstand zum Richtwert</span>
            <b class="num">{gap(l)}</b>
            <small
              >{high ? "darüber" : "darunter"} · {ratio(l)
                ? `${int(l.made)} Bets/Raises zu ${int(l.opp)} Calls`
                : `${int(l.made)} von ${int(l.opp)} Gelegenheiten`}</small
            >
          </div>
          <div class="fig">
            <span class="label"><Term tip={SURE_HELP}>Sicherheit des Leaks</Term></span>
            <b class="num">{pct(l.confidence)}</b>
            <small
              ><Term tip="Mit 95 % Wahrscheinlichkeit liegt dein wahrer Wert in diesem Bereich. Je mehr Hände, desto enger."
                >dein Wert wahrscheinlich <span class="nowrap">{val(l, l.ci_low)}–{val(l, l.ci_high)}</span></Term
              ></small
            >
          </div>
          {#if !l.cost}
            <div class="fig">
              <span class="label">Kosten grob</span>
              <b class="num faint">–</b>
              <small>für diesen Spot nicht schätzbar</small>
            </div>
          {:else}
            <div class="fig">
              <span class="label"><Term tip="{COST_HELP} Gilt für den ganzen Spot ({int(l.cost.hands)} Hände), nicht nur für diesen Wert.">Kosten grob</Term></span>
              <b class="num neg">≈ {int(Math.round(l.cost.total_bb))} BB</b>
              <small
                ><span class={sign(l.cost.spot_bb100)}>{signed(l.cost.spot_bb100)}</span> statt
                <span class={sign(l.cost.base_bb100)}>{signed(l.cost.base_bb100)}</span> BB/100</small
              >
            </div>
          {/if}
        </div>
        <footer class="cites">
          {#each l.cites as c (c.source)}
            {@const s = sources.get(c.source)}
            <span>Quelle: {#if s?.scope}<Term tip={s.scope}>{s.publisher}</Term>{:else}{s?.publisher ?? c.source}{/if} – „{c.says}“</span>
          {/each}
          {#if l.note}<span>{l.note}</span>{/if}
        </footer>
      </div>

      <div class="advice">
        {#if explain}
          <p class="what"><b>Was ist das?</b> {explain.what}</p>
          <p class="mean"><b>{high ? "Zu hoch heißt meist:" : "Zu niedrig heißt meist:"}</b> {high ? explain.high : explain.low}</p>
          {#if l.uncertain}
            <p class="todo wait"><b>Jetzt:</b> Mehr Gelegenheiten sammeln und noch nichts umstellen.</p>
            <p class="later"><b>Falls es so bleibt:</b> {high ? explain.tryHigh : explain.tryLow}</p>
          {:else}
            <p class="todo"><b>Was du tun kannst:</b> {high ? explain.tryHigh : explain.tryLow}</p>
          {/if}
        {/if}
        {#if ex}<p class="why">{ex.why}</p>{/if}
        <div class="links">
          {#if ex}
            <Button size="sm" variant={l.uncertain ? "quiet" : rank === 1 ? "primary" : "secondary"} onclick={() => onLearn(ex)}
              >Übung: {SKILLS[ex.kind].short}</Button
            >
          {/if}
          {#if article}
            <Button size="sm" variant={ex || l.uncertain ? "quiet" : "secondary"} onclick={() => onLearn({ article })}
              >Wissen: {title ?? STAT_META[l.stat].name}</Button
            >
          {/if}
          <Button size="sm" variant={!l.uncertain && (ex || article) ? "quiet" : "secondary"} onclick={() => hands(l)}>Hände ansehen</Button>
        </div>
      </div>
    </div>
  </article>
{/snippet}

<div class="page leaks">
  {#if failed}
    <Panel>
      <EmptyState
        tone="error"
        title="Die Leaks konnten nicht geladen werden."
        text="Deine Hände sind sicher gespeichert. Versuch es noch einmal; bleibt der Fehler, starte Ringside neu."
      >
        {#snippet action()}<Button variant="primary" onclick={() => retry++}>Erneut laden</Button>{/snippet}
      </EmptyState>
    </Panel>
  {:else if data}
    <section class="panel verdict" class:bare={!data.leaks.length && !data.fine.length}>
      <div class="answer">
        {#if first}
          <h2>{sure.length === 1 ? "1 Leak gefunden" : `${int(sure.length)} Leaks gefunden`}</h2>
          <p>
            Fang oben an: Dein Wert <b>{STAT_META[first.stat].name}</b> ist {first.direction === "too_high" ? "zu hoch" : "zu niedrig"}
            ({where(first)}, {data.bands[first.band]}{first.cost ? `, kostet grob ${int(Math.round(first.cost.total_bb))} BB` : ""}).
          </p>
        {:else if unsure.length}
          <h2>Noch kein sicherer Leak</h2>
          <p>
            {unsure.length === 1 ? "Ein Wert weicht" : `${int(unsure.length)} Werte weichen`} vom Richtwert ab, aber es gibt noch zu wenig Hände
            für ein sicheres Urteil. Spiel weiter; Ringside prüft bei jedem Import neu.
          </p>
        {:else if data.fine.length}
          <h2>Kein Leak gefunden</h2>
          <p>Alle geprüften Werte liegen im Richtbereich. Mehr Hände machen die Prüfung genauer.</p>
        {:else}
          <h2>Noch zu wenig Hände</h2>
          <p>
            Ringside vergleicht jeden Wert getrennt nach Position und Stacktiefe – erst wenn ein solcher Spot 10 Gelegenheiten hat,
            zählt er. Deine gespeicherten Hände reichen dafür in dieser Auswahl noch nicht{data.too_small
              ? ` – ${int(data.too_small)} Spots sammeln gerade Gelegenheiten`
              : ""}. Spiel ein paar Hände{filter.since !== null ||
            filter.game !== null
              ? " oder wähle oben „Alles“ und „Alle“"
              : "; Ringside prüft bei jedem Import neu"}.
          </p>
        {/if}
        <p class="honest">
          Richtwerte aus veröffentlichten Quellen – eine Orientierung, keine „richtige“ Strategie. Einzelne teure Entscheidungen
          zeigt die Übersicht.
        </p>
      </div>
      {#if data.leaks.length || data.fine.length}
      <ul class="counts">
        <li>
          <b class="num">{int(unsure.length)}</b>
          <span><strong>Noch unsicher</strong><em>zu wenig Hände für ein Urteil</em></span>
        </li>
        <li class="ok">
          <b class="num">{int(data.fine.length)}</b>
          <span><strong>Im Richtbereich</strong><em>hier passt es</em></span>
        </li>
        <li class="na">
          <b class="num">{int(data.too_small)}</b>
          <span><strong>Nicht bewertbar</strong><em>unter 10 Gelegenheiten</em></span>
        </li>
      </ul>
      <div class="how">
        <Button size="sm" variant="quiet" aria-expanded={howOpen} onclick={() => (howOpen = !howOpen)}>
          {#snippet icon()}<svg class="chev" class:open={howOpen} viewBox="0 0 24 24" width="12" height="12" aria-hidden="true"
              ><path d="M9 6l6 6-6 6" /></svg
            >{/snippet}
          So liest du die Balken
        </Button>
        {#if howOpen}
        <dl class="legend" transition:slide={{ duration: 220, easing: cubicOut }}>
          <div><dt><span class="key ref"></span>Grüner Bereich</dt><dd>der Richtwert aus den Quellen</dd></div>
          <div><dt><span class="key dot"></span>Punkt</dt><dd>dein Wert</dd></div>
          <div><dt><span class="key ci"></span>Linie</dt><dd>wo dein wahrer Wert mit 95 % liegt. Je weniger Hände, desto länger die Linie.</dd></div>
          <div><dt>Sicherheit</dt><dd>wie sicher dein wahrer Wert außerhalb des Richtwerts liegt. Ab 90 % lohnt es sich, daran zu arbeiten.</dd></div>
          <div><dt>20–40 BB …</dt><dd>dein effektiver Stack in der Hand; Turnier und Cash werden getrennt geprüft.</dd></div>
        </dl>
        {/if}
      </div>
      {/if}
    </section>

    {#if data.leaks.length}
      <div class="list">
        {#each sure as l, i (key(l))}
          {@render leak(l, i + 1)}
        {/each}
        {#if unsure.length}
          <h2 class="group">Noch unsicher <span>– beobachten, noch nicht daran arbeiten</span></h2>
          {#each unsure as l (key(l))}
            {@render leak(l, null)}
          {/each}
        {/if}
      </div>
    {/if}

    {#if data.fine.length}
      {@const shown = fineOpen ?? data.fine.length <= 8}
      <Panel flush title="Im Richtbereich ({int(data.fine.length)})" sub="Hier passt es – diese Werte liegen im Richtbereich der Quellen.">
        {#snippet actions()}
          <IconButton
            size="sm"
            label={shown ? "Liste einklappen" : "Liste zeigen"}
            aria-expanded={shown}
            onclick={() => (fineOpen = !shown)}
            ><svg class="chev" class:open={shown} viewBox="0 0 24 24" width="14" height="14" aria-hidden="true"
              ><path d="M9 6l6 6-6 6" /></svg
            ></IconButton
          >
        {/snippet}
        {#if shown}
        <ul class="fine-list" transition:slide={{ duration: 260, easing: cubicOut }}>
          {#each data.fine as l (key(l))}
            <li>
              <div class="fine-head">
                <b class="ok-name">
                  <svg viewBox="0 0 24 24" width="14" height="14" aria-hidden="true"><path d="M5 12.5l4.5 4.5L19 7.5" /></svg>
                  <Term tip="{STAT_META[l.stat].name}: {STAT_META[l.stat].help}">{STAT_META[l.stat].name}</Term>
                </b>
                <span class="fine-where">{where(l)} · {game(l)} · {data.bands[l.band]}</span>
              </div>
              {@render bar(l, true)}
              <div class="fine-nums num">
                <span>Du <b>{val(l, l.value)}</b></span>
                <span>Richtwert {range(l)}</span>
                <span class="fine-opp">{int(l.opp)} Gelegenheiten</span>
              </div>
            </li>
          {/each}
        </ul>
        {/if}
      </Panel>
    {/if}

    {#if !data.leaks.length && !data.fine.length}
      <p class="sources-line">
        Richtwerte aus: {#each data.sources as s, i (s.id)}{#if i}{", "}{/if}<Term tip="{s.title} – {s.scope}">{s.publisher}</Term>{/each}
      </p>
    {:else}
    <Panel title="Quellen der Richtwerte">
      <ul class="sources">
        {#each data.sources as s (s.id)}
          <li>
            {#if s.url}
              <a href={s.url} target="_blank" rel="noreferrer" onclick={(e) => open(s.url as string, e)}>{s.publisher}: {s.title}</a>
            {:else}
              <span>{s.publisher}: {s.title}</span>
            {/if}
            <small>{s.scope}</small>
          </li>
        {/each}
      </ul>
    </Panel>
    {/if}
  {/if}
</div>

<style>
  .leaks {
    padding-inline: 24px;
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding-top: 20px;
  }

  /* The answer: how many leaks, where to start. */
  .verdict {
    contain: paint;
    display: grid;
    grid-template-columns: minmax(0, 720px) auto;
    justify-content: start;
    align-items: start;
    gap: 16px 32px;
    padding: 20px 22px;
  }
  .verdict.bare {
    grid-template-columns: 1fr;
  }
  .answer h2 {
    margin: 0;
    font-family: var(--font-num);
    font-size: var(--fs-3xl);
    font-weight: 600;
    line-height: 1.15;
  }
  .answer p {
    margin: 6px 0 0;
    max-width: 720px;
    font-size: var(--fs-base);
    line-height: 1.55;
    color: var(--text);
  }
  .answer .honest {
    margin-top: 8px;
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .counts {
    display: grid;
    grid-template-columns: repeat(3, minmax(150px, 190px));
    gap: 8px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .counts li {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 170px;
    padding: 10px 14px;
    border-radius: var(--r-sm);
    background: var(--surface-2);
  }
  .counts b {
    font-size: var(--fs-3xl);
    font-weight: 600;
    line-height: 1;
    color: var(--text);
  }
  .counts li.ok b {
    color: var(--text);
  }
  .counts li.na b {
    color: var(--text-3);
  }
  .counts span {
    display: flex;
    flex-direction: column;
    font-size: var(--fs-md);
    line-height: 1.35;
    color: var(--text-2);
  }
  .counts em {
    font-style: normal;
  }
  .counts strong {
    font-size: var(--fs-base);
    font-weight: 600;
    color: var(--text);
  }
  .nowrap {
    white-space: nowrap;
  }
  .chev {
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
    transition: transform var(--dur) var(--ease-standard);
  }
  .chev.open {
    transform: rotate(90deg);
  }
  .how {
    grid-column: 1 / -1;
    margin: -4px 0 0;
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .legend {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 22px;
    margin: 10px 0 0;
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

  .list {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .group {
    margin: 10px 0 0;
    font-family: var(--font-num);
    font-size: var(--fs-lg);
    font-weight: 600;
  }
  .group span {
    font-family: var(--font-ui);
    font-size: var(--fs-base);
    font-weight: 400;
    color: var(--text-2);
  }

  /* One leak: what it is, how far off, what to do. */
  .leak {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 16px 20px 14px;
  }
  .leak.unsure {
    background: transparent;
    border-style: dashed;
  }
  .leak header {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 6px 14px;
  }
  .name {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .rank {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    background: var(--surface-3);
    font-size: var(--fs-md);
    font-weight: 600;
  }
  h3 {
    margin: 0;
    font-size: var(--fs-lg);
    font-weight: 600;
  }
  .tags {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .body {
    display: grid;
    grid-template-columns: minmax(0, 1.1fr) minmax(0, 1fr);
    gap: 18px 32px;
  }
  .measure {
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-width: 0;
  }
  .figures {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 10px 18px;
  }
  .fig {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .label {
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .fig b {
    font-size: var(--fs-xl);
    font-weight: 600;
  }
  .fig small {
    font-size: var(--fs-md);
    line-height: 1.4;
    color: var(--text-2);
  }

  /* The bar, labelled in place. */
  .meter {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: 10px;
    padding: 22px 0 22px;
  }
  .meter.small {
    display: block;
    padding: 0;
  }
  .end {
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .track-wrap {
    position: relative;
  }
  .track {
    position: relative;
    height: 14px;
    border-radius: 3px;
    background: var(--surface-2);
  }
  .small .track {
    height: 8px;
  }
  .ref {
    position: absolute;
    top: 0;
    bottom: 0;
    background: color-mix(in srgb, var(--pos) 28%, transparent);
    border-left: 1px solid var(--pos);
    border-right: 1px solid var(--pos);
  }
  .ci {
    position: absolute;
    top: 6px;
    height: 2px;
    background: var(--text-2);
    opacity: 0.6;
  }
  .small .ci {
    top: 3px;
  }
  .dot {
    position: absolute;
    top: 2px;
    width: 10px;
    height: 10px;
    margin-left: -5px;
    border-radius: 50%;
    background: var(--text);
    box-shadow: 0 0 0 2px var(--surface);
  }
  .small .dot {
    top: 0;
    width: 8px;
    height: 8px;
    margin-left: -4px;
  }
  .dot.off {
    background: var(--amber);
  }
  .lab {
    position: absolute;
    font-size: var(--fs-md);
    color: var(--text-2);
    white-space: nowrap;
    transform: translateX(-50%);
  }
  .lab b {
    font-weight: 600;
    color: var(--text);
  }
  .lab.top {
    bottom: calc(100% + 5px);
  }
  .lab.bottom {
    top: calc(100% + 5px);
  }
  .lab.bottom.off b {
    color: var(--amber);
  }
  .lab.start {
    transform: none;
  }
  .lab.end {
    transform: translateX(-100%);
  }

  .advice {
    display: flex;
    flex-direction: column;
    gap: 7px;
    padding: 14px 16px;
    border-radius: var(--r-sm);
    background: var(--surface-2);
    font-size: var(--fs-base);
    line-height: 1.5;
    color: var(--text-2);
  }
  .advice p {
    max-width: 72ch;
    margin: 0;
  }
  .advice b {
    color: var(--text);
    font-weight: 600;
  }
  .advice .todo {
    padding-left: 10px;
    border-left: 2px solid var(--pos);
    color: var(--text);
  }
  .advice .why {
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .advice .todo.wait {
    border-left-color: var(--amber);
  }
  .links {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: auto;
    padding-top: 6px;
  }
  .cites {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin-top: auto;
    padding-top: 10px;
    border-top: 1px solid var(--line);
    font-size: var(--fs-md);
    color: var(--text-2);
  }

  .fine-list {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
    gap: 10px;
    margin: 0;
    padding: 0 20px 18px;
    list-style: none;
  }
  .fine-list li {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px 14px;
    border-radius: var(--r-sm);
    background: var(--surface-2);
  }
  .fine-head {
    display: flex;
    flex-direction: column;
    font-size: var(--fs-base);
  }
  .ok-name {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .ok-name svg {
    fill: none;
    stroke: var(--pos);
    stroke-width: 2.4;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .fine-list .small .track {
    background: var(--surface-3);
  }
  .fine-where {
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .fine-opp {
    color: var(--text-2);
  }
  .fine-nums {
    display: flex;
    flex-wrap: wrap;
    gap: 2px 14px;
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .fine-nums b {
    color: var(--text);
    font-weight: 600;
  }

  .sources-line {
    margin: 0;
    padding: 0 4px;
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .sources {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(360px, 1fr));
    gap: 10px 28px;
    margin: 10px 0 0;
    padding: 0;
    list-style: none;
    font-size: var(--fs-base);
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
    font-size: var(--fs-md);
    color: var(--text-2);
  }

  /* Wide windows: a readable page width, the bar column gets the extra room. */
  @container (min-width: 1900px) {
    .leaks {
      max-width: none;
    }
    .body {
      grid-template-columns: minmax(0, 1.5fr) minmax(0, 1fr);
    }
    .verdict {
      grid-template-columns: minmax(0, 720px) minmax(0, 1fr);
    }
    .counts {
      grid-template-columns: repeat(3, minmax(0, 1fr));
    }
  }
  @container (max-width: 1080px) {
    .verdict {
      grid-template-columns: 1fr;
    }
    .verdict {
      gap: 12px;
      padding: 16px 18px;
    }
    .counts {
      grid-template-columns: repeat(3, minmax(0, 1fr));
    }
    .counts li {
      min-width: 0;
      padding: 8px 12px;
    }
    .counts em {
      display: none;
    }
    .counts b {
      font-size: var(--fs-2xl);
    }
  }
  @container (max-width: 900px) {
    .body {
      grid-template-columns: 1fr;
    }
    /* One column: what to do comes right under the title, the figures follow. */
    .advice {
      order: -1;
    }
  }
</style>
