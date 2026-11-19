<script lang="ts">
  import { untrack } from "svelte";
  import { GRADE_OF, SOURCE } from "../analysis/labels";
  import { analysis, watchAnalysis } from "../analysis/store.svelte";
  import { api } from "../api";
  import type { LearnTarget } from "../app/Leaks.svelte";
  import { dateTime, dayShort } from "../app/format";
  import { errorText } from "../coach/api";
  import { notAnalyzable } from "../coach/meaning";
  import { classLabel } from "../solver/range";
  import { cash, tournamentName } from "../stats";
  import Badge from "../ui/Badge.svelte";
  import Button from "../ui/Button.svelte";
  import EmptyState from "../ui/EmptyState.svelte";
  import { bb, int, num, signed } from "../ui/format";
  import GradeBars from "../ui/GradeBars.svelte";
  import GradeIcon from "../ui/GradeIcon.svelte";
  import MiniCard from "../ui/MiniCard.svelte";
  import { usePageHead } from "../ui/page.svelte";
  import Panel from "../ui/Panel.svelte";
  import Reason from "../ui/Reason.svelte";
  import { GRADES, type Grade } from "../ui/poker";
  import RangeGrid, { type RangeCell } from "../ui/RangeGrid.svelte";
  import SegmentedControl from "../ui/SegmentedControl.svelte";
  import Term from "../ui/Term.svelte";
  import AskCoach from "./AskCoach.svelte";
  import StackChart from "./StackChart.svelte";
  import type { Breakdown, Moment, TournamentReport } from "./types";
  import NextSteps from "../next/NextSteps.svelte";
  import type { NextStep, StepHands } from "../next/types";

  let {
    tournamentId,
    refresh = 0,
    onBack,
    onHand,
    onHands,
    onLearn,
    onStepHands,
    onCoach,
  }: {
    tournamentId: number;
    refresh?: number;
    onBack: () => void;
    /** Opens a hand in "Hände", stopped before the decision at `at` when given. */
    onHand: (handId: number, at: number | null) => void;
    /** Every hand of the tournament in "Hände". */
    onHands: () => void;
    onLearn: (target: LearnTarget) => void;
    /** „Hände ansehen“ of a step of „Was als Nächstes?“. */
    onStepHands?: (hands: StepHands) => void;
    /** The coach page: its setup, a stored review, or a new review of this tournament. */
    onCoach: (what: "setup" | "review" | "analyze") => void;
  } = $props();

  let report = $state<TournamentReport | null>(null);
  let failed = $state<string | null>(null);
  let breakdown = $state<Breakdown["key"]>("situation");
  let gridMode = $state<"result" | "errors">("result");
  let picked = $state(-1);

  watchAnalysis();

  function load() {
    const id = tournamentId;
    api.tournamentReport(id).then(
      (r) => {
        if (id !== tournamentId) return;
        report = r;
        failed = null;
      },
      (e) => id === tournamentId && (failed = errorText(e)),
    );
  }

  $effect(() => {
    void tournamentId;
    void refresh;
    untrack(load);
  });

  /** A preliminary report follows the solver and the analysis until it is final. */
  const preliminary = $derived(!!report && (report.pending > 0 || report.unanalyzed > 0));
  $effect(() => {
    void analysis.solved;
    void analysis.status?.counts.analyzed;
    if (untrack(() => preliminary)) untrack(load);
  });

  const t = $derived(report?.tournament ?? null);
  const name = $derived(t ? tournamentName(t) : "");
  usePageHead(() => ({ sub: t ? `Bericht · ${name} · ${dateTime(t.started_at)}` : "Bericht" }));

  const s = $derived(report?.score ?? null);
  const avg = $derived(report?.average ?? null);
  /** Lower is better: loss per 100 decisions against the average of the earlier tournaments. */
  const versus = $derived.by(() => {
    if (!s?.per100 && s?.per100 !== 0) return null;
    if (avg?.per100 == null) return null;
    const d = s.per100 - avg.per100;
    return { diff: d, word: d < -0.5 ? "besser" : d > 0.5 ? "schlechter" : "gleich" } as const;
  });
  const gradeCounts = $derived(Object.fromEntries(GRADES.map((g, i) => [g, s?.ratings[i] ?? 0])) as Record<Grade, number>);

  const shown = $derived(report?.breakdowns.find((b) => b.key === breakdown) ?? null);
  const maxLoss = $derived(Math.max(0.01, ...(shown?.groups.map((g) => g.loss_bb) ?? [0])));

  const handIndex = new Map(Array.from({ length: 169 }, (_, i) => [classLabel(i), i]));
  const byClass = $derived(new Map((report?.starting ?? []).map((h) => [handIndex.get(h.class) ?? -1, h])));
  const cells = $derived.by<RangeCell[]>(() => {
    const list = report?.starting ?? [];
    const maxNet = Math.max(1, ...list.map((h) => Math.abs(h.net_bb)));
    const maxErr = Math.max(0.01, ...list.map((h) => h.loss_bb));
    return Array.from({ length: 169 }, (_, i) => {
      const h = byClass.get(i);
      if (!h) return null;
      if (gridMode === "errors") {
        return h.loss_bb > 0
          ? { color: "var(--grade-mistake)", weight: 0.25 + 0.75 * (h.loss_bb / maxErr), value: num(h.loss_bb, 1), note: `${h.class}: ${bb(h.loss_bb)} durch Fehler` }
          : { color: "var(--surface-3)", weight: 1, note: `${h.class}: ohne Fehler` };
      }
      return {
        color: h.net_bb >= 0 ? "var(--pos)" : "var(--neg)",
        weight: 0.25 + 0.75 * (Math.abs(h.net_bb) / maxNet),
        value: signed(h.net_bb, 0),
        note: `${h.class}: ${bb(h.net_bb, { sign: true })} in ${h.hands} ${h.hands === 1 ? "Hand" : "Händen"}`,
      };
    });
  });
  const topWin = $derived([...(report?.starting ?? [])].sort((a, b) => b.net_bb - a.net_bb)[0] ?? null);
  const topLoss = $derived([...(report?.starting ?? [])].sort((a, b) => a.net_bb - b.net_bb)[0] ?? null);

  /** Compared with the last tournaments listed: the share of them this one beat. */
  const trend = $derived.by(() => {
    const h = report?.history.filter((x) => x.per100 !== null) ?? [];
    if (!h.length || s?.per100 == null) return null;
    const beaten = h.filter((x) => s.per100! < x.per100!).length;
    return { beaten, of: h.length };
  });
  const maxPer100 = $derived(Math.max(0.1, s?.per100 ?? 0, ...(report?.history.map((x) => x.per100 ?? 0) ?? [])));

  const STREET = { preflop: "Preflop", flop: "Flop", turn: "Turn", river: "River" } as const;
  const cardsOf = (text: string) => text.split(/\s+/).filter(Boolean);

  function resultLine(): string {
    if (!t) return "";
    if (t.place !== null && t.prize) return `Platz ${int(t.place)}${t.entrants ? ` von ${int(t.entrants)}` : ""} – ${cash(t.prize, t.currency)} Preisgeld`;
    if (t.place !== null) return `Platz ${int(t.place)}${t.entrants ? ` von ${int(t.entrants)}` : ""}, ohne Preisgeld`;
    if (t.prize === 0) return "Ausgeschieden ohne Preisgeld";
    return "Noch offen";
  }

  function luckLine(r: TournamentReport): string {
    const l = r.luck;
    if (l.all_ins === 0) return "Kein All-in mit offenen Karten: Glück spielte bei deinen Chips kaum eine Rolle.";
    const chips = int(Math.abs(l.luck) / 100);
    if (Math.abs(l.luck_bb) < 1) return `In ${l.all_ins} All-ins hielten sich Glück und Pech die Waage.`;
    const what = l.luck_bb < 0 ? "Pech" : "Glück";
    return `Nach den Gewinnchancen ${l.all_ins === 1 ? "deines All-ins" : `deiner ${l.all_ins} All-ins`} wären im Schnitt ${bb(l.ev_bb, { sign: true })} zu erwarten gewesen, du hast ${bb(l.net_bb, { sign: true })} gemacht: ${what} von etwa ${bb(Math.abs(l.luck_bb))} (${chips} Chips).`;
  }

  const LUCK_HELP =
    "All-in-EV: Bei jedem All-in, bei dem alle Karten offen lagen, rechnet Ringside, was du im Schnitt gewonnen hättest. Der Unterschied zum echten Ergebnis ist Glück oder Pech – das kannst du nicht beeinflussen.";
  const LOSS_HELP =
    "Kostet im Schnitt: Wie viele Big Blinds deine Entscheidungen gegenüber der besten Wahl im Schnitt gekostet haben. Je 100 Entscheidungen gerechnet, damit lange und kurze Turniere vergleichbar sind. Weniger ist besser.";
  const GOOD_HELP = "Gute Entscheidungen: beste Wahl, gleich gute Mischung oder gut. Ungenauigkeiten und Fehler zählen nicht dazu.";
</script>

{#snippet here(step: NextStep)}
  {@const k = report?.kinds.find((x) => x.key === step.spot_key)}
  {#if k}
    <p class="here">
      In diesem Turnier: <b class="num">{bb(k.loss_bb)}</b> in {int(k.decisions)}
      {k.decisions === 1 ? "Entscheidung" : "Entscheidungen"}{k.flawed ? `, ${int(k.flawed)} davon ungenau oder schlechter` : ""}.
    </p>
  {/if}
{/snippet}

{#snippet moment(m: Moment, i: number, bad: boolean)}
  <article class="moment" class:bad>
    <header>
      {#if bad}<span class="rank num" aria-label="Fehler {i + 1}">{i + 1}</span>{/if}
      {#if m.rating}<GradeIcon grade={GRADE_OF[m.rating]} size={16} />{/if}
      <span class="cards">{#each cardsOf(m.hero_cards) as c (c)}<MiniCard card={c} />{/each}</span>
      <span class="where">{m.position ?? "?"} · {STREET[m.street]} · Hand {int(m.n)}</span>
    </header>
    <p class="situation">{m.situation}{m.stack_bb !== null ? ` · ${num(m.stack_bb, 0)} BB` : ""}</p>
    <p class="played">Gespielt: <b>{m.played}</b></p>
    {#if bad}
      <p class="better">Besser: <b>{m.best ?? "–"}</b> <span class="cost num">{bb(-m.loss_bb, { sign: true, digits: 2 })}</span></p>
    {:else if m.alternative && m.margin_bb !== null}
      <p class="better">Richtig. {m.alternative} hätte <b class="num">{bb(m.margin_bb, { digits: 2 })}</b> gekostet.</p>
    {:else}
      <p class="better">Richtig gespielt.</p>
    {/if}
    {#if m.why && bad}<Reason text={m.why} />{:else if m.why}<p class="why">{m.why}</p>{/if}
    <footer>
      {#if m.source}<span class="src">{SOURCE[m.source].label}</span>{/if}
      <Button size="sm" onclick={() => onHand(m.hand_id, m.action_index)}>Abspielen</Button>
    </footer>
  </article>
{/snippet}

<div class="page report">
  <div class="top">
    <Button size="sm" variant="quiet" onclick={onBack}>
      {#snippet icon()}<svg class="ico" viewBox="0 0 24 24" width="14" height="14" aria-hidden="true"><path d="M15 5l-7 7 7 7" /></svg>{/snippet}
      Alle Turniere
    </Button>
    {#if t}
      <span class="name" title="#{t.id}">{name}</span>
      <span class="date">{dayShort(t.started_at)} · {int(report?.hands ?? 0)} Hände</span>
      <span class="spacer"></span>
      <Button size="sm" onclick={onHands}>Alle Hände ansehen</Button>
    {/if}
  </div>

  {#if failed}
    <Panel>
      <EmptyState tone="error" title="Der Bericht ließ sich nicht berechnen." text={failed}>
        {#snippet action()}<Button onclick={load}>Erneut versuchen</Button>{/snippet}
      </EmptyState>
    </Panel>
  {:else if !report || !t || !s}
    <Panel><EmptyState tone="loading" title="Rechnet den Bericht …" /></Panel>
  {:else}
    {#if preliminary}
      <p class="note" role="status">
        <Badge tone="warn" dot>Vorläufig</Badge>
        {#if report.pending}{report.pending === 1 ? "1 Flop-Spot wird" : `${int(report.pending)} Flop-Spots werden`} noch gerechnet{/if}{#if report.pending && report.unanalyzed}{", "}{/if}{#if report.unanalyzed}{report.unanalyzed === 1 ? "1 Hand ist" : `${int(report.unanalyzed)} Hände sind`} noch nicht bewertet{/if}. Der Bericht
        aktualisiert sich danach von selbst.
      </p>
    {/if}

    <section class="answer" aria-label="Das Wichtigste">
      <div class="cell">
        <span class="eyebrow">Ergebnis</span>
        <strong class="big">{resultLine()}</strong>
        {#if t.place === null && t.prize === null}
          <span class="say"
            >Bisher <b class="num {report.luck.net_bb >= 0 ? 'pos' : 'neg'}">{bb(report.luck.net_bb, { sign: true })}</b> in Chips nach {int(report.hands)} Händen. Platz und Preisgeld
            stehen hier, sobald PokerStars die Turnier-Zusammenfassung schreibt.</span
          >
        {/if}
        {#if t.profit !== null}
          <span class="say">Nach Buy-in <b class="num {t.profit >= 0 ? 'pos' : 'neg'}">{t.profit > 0 ? "+" : ""}{cash(t.profit, t.currency)}</b>{t.bounties
              ? `, davon ${cash(t.bounties, t.currency)} Bounties`
              : ""}.</span
          >
        {/if}
      </div>
      <div class="cell">
        <span class="eyebrow"><Term tip={LUCK_HELP}>Glück gegen All-in-EV</Term></span>
        <div class="luck" aria-hidden="true">
          {#each [["Ergebnis", report.luck.net_bb], ["Normales Glück", report.luck.ev_bb]] as [label, v] (label)}
            {@const most = Math.max(1, Math.abs(report.luck.net_bb), Math.abs(report.luck.ev_bb))}
            <span class="luck-label">{label}</span>
            <span class="luck-bar"
              ><i class:neg-bar={(v as number) < 0} style:width="{(Math.abs(v as number) / most) * 50}%" style:margin-left={(v as number) < 0 ? `${50 - (Math.abs(v as number) / most) * 50}%` : "50%"}></i></span
            >
            <b class="num">{bb(v as number, { sign: true })}</b>
          {/each}
        </div>
        <span class="say">{luckLine(report)}</span>
      </div>
      <div class="cell">
        <span class="eyebrow"><Term tip={GOOD_HELP}>Deine Entscheidungen</Term></span>
        {#if s.decisions}
          <strong class="big num">{num(s.good_pct ?? 0, 0)}&nbsp;% gut</strong>
          <span class="say">
            {int(s.good)} von {int(s.decisions)} bewerteten Entscheidungen. <Term tip={LOSS_HELP}>Kostet im Schnitt</Term>
            <b class="num">{bb(s.per100 ?? 0)}</b> je 100 Entscheidungen{#if versus && avg}{" "}–
              {versus.word === "gleich" ? "so wie" : `${versus.word} als`} dein Schnitt (<span class="num">{bb(avg.per100 ?? 0)}</span>
              aus {int(avg.tournaments)} Turnieren){/if}.
          </span>
          <GradeBars stacked counts={gradeCounts} label="Deine Entscheidungen nach Stufe" />
        {:else}
          <strong class="big">Noch nichts bewertet</strong>
          <span class="say"
            >{t.game && !t.game.includes("Hold'em")
              ? "Ringside bewertet bisher nur No-Limit Hold'em. Ergebnis, Glück und Verlauf stehen trotzdem hier."
              : "Ringside bewertet die Hände, sobald PokerStars geschlossen ist. Danach steht hier, wie gut du gespielt hast."}</span
          >
        {/if}
      </div>
    </section>

    {#if s.decisions}
      <Panel title="Wo du gut spielst und wo du verlierst" sub="Verlust in Big Blinds gegenüber der besten Wahl. Ein Haken heißt: hier ohne Fehler.">
        {#snippet actions()}
          <SegmentedControl
            size="sm"
            label="Aufteilen nach"
            value={breakdown}
            options={(report?.breakdowns ?? []).map((b) => ({ value: b.key, label: b.title }))}
            onchange={(v) => (breakdown = v)}
          />
        {/snippet}
        {#if shown}
          <ul class="groups">
            {#each shown.groups as g (g.key)}
              <li>
                <span class="g-title">
                  {#if g.flawed === 0}<GradeIcon grade="best" size={14} />{:else}<GradeIcon grade={g.loss_bb >= 2 ? "mistake" : "inaccurate"} size={14} />{/if}
                  {g.title}
                </span>
                <span class="g-bar" aria-hidden="true"><i style:width="{(g.loss_bb / maxLoss) * 100}%"></i></span>
                <span class="g-cost num">{g.loss_bb > 0 ? bb(-g.loss_bb, { sign: true }) : "0 BB"}</span>
                <span class="g-meta">
                  {int(g.decisions)}× · {g.flawed ? `${int(g.flawed)} ungenau oder schlechter` : "alles gut"}{#if g.unsure}{" · "}<span class="unsure" title="Mehr als die Hälfte dieses Verlusts stammt aus Ringsides Einschätzung (Equity und Pot Odds), nicht aus Solver oder Modell.">vor allem Einschätzung</span>{/if}
                </span>
              </li>
            {/each}
          </ul>
        {/if}
      </Panel>

      <div class="pair">
        <Panel title="Die teuersten Fehler" sub="Ein Klick spielt die Hand bis zu dieser Entscheidung ab.">
          {#if report.worst.length}
            <div class="moments">{#each report.worst as m, i (m.hand_id + ":" + m.action_index)}{@render moment(m, i, true)}{/each}</div>
          {:else}
            <EmptyState compact title="Kein Fehler, der etwas gekostet hat." text="Alle bewerteten Entscheidungen waren gut." />
          {/if}
        </Panel>
        <Panel title="Deine besten Entscheidungen" sub="Wo die beste Wahl am meisten ausgemacht hat.">
          {#if report.best.length}
            <div class="moments">{#each report.best as m, i (m.hand_id + ":" + m.action_index)}{@render moment(m, i, false)}{/each}</div>
          {:else}
            <EmptyState compact title="Noch keine herausragende Entscheidung." />
          {/if}
        </Panel>
      </div>
    {/if}

    <div class="pair">
      <Panel title="Deine Hände" sub={gridMode === "result" ? "Was dir jede Starthand gebracht oder gekostet hat, in BB." : "Was Fehler mit jeder Starthand gekostet haben, in BB."}>
        {#snippet actions()}
          <SegmentedControl
            size="sm"
            label="Farbe im Raster"
            value={gridMode}
            options={[
              { value: "result", label: "Ergebnis" },
              { value: "errors", label: "Fehler" },
            ]}
            onchange={(v) => (gridMode = v)}
          />
        {/snippet}
        {#if report.starting.length}
          <div class="hands">
            <div class="grid-box">
              <RangeGrid {cells} size="sm" label="Starthände dieses Turniers" selected={picked} onSelect={(i) => (picked = picked === i ? -1 : i)} onClose={() => (picked = -1)}>
                {#snippet card(i)}
                  {@const h = byClass.get(i)}
                  {#if h}
                    <div class="pin">
                      <b class="num">{h.class}</b>
                      <p>{h.hands} {h.hands === 1 ? "Hand" : "Hände"} · {bb(h.net_bb, { sign: true })}{h.flawed ? ` · ${h.flawed} ${h.flawed === 1 ? "Fehler" : "Fehler"}` : ""}</p>
                      <div class="pin-hands">
                        {#each h.hand_ids.slice(0, 6) as id, k (id)}
                          <Button size="sm" onclick={() => onHand(id, null)}>Hand {k + 1}</Button>
                        {/each}
                      </div>
                    </div>
                  {/if}
                {/snippet}
              </RangeGrid>
            </div>
            <dl class="hand-facts">
              {#if topWin && topWin.net_bb > 0}
                <div><dt>Am meisten gebracht</dt><dd><b class="num">{topWin.class}</b> <span class="num pos">{bb(topWin.net_bb, { sign: true })}</span></dd></div>
              {/if}
              {#if topLoss && topLoss.net_bb < 0}
                <div><dt>Am meisten gekostet</dt><dd><b class="num">{topLoss.class}</b> <span class="num neg">{bb(topLoss.net_bb, { sign: true })}</span></dd></div>
              {/if}
              <div><dt>Gespielt</dt><dd><span class="num">{int(report.starting.length)}</span> verschiedene Starthände</dd></div>
            </dl>
          </div>
        {:else}
          <EmptyState compact title="Keine Karten bekannt." />
        {/if}
      </Panel>

      <Panel title="Verlauf im Turnier" sub="Deine Chips am Anfang jeder Hand. Die Zahlen markieren die teuersten Fehler.">
        <StackChart points={report.timeline} />
      </Panel>
    </div>

    <div class="pair">
      <Panel title="Gegen deine letzten Turniere" sub="Was dich je 100 Entscheidungen im Schnitt kostet – kürzer ist besser.">
        {#if report.history.length && s.per100 !== null}
          <p class="lead">
            {#if trend}
              {trend.beaten === trend.of
                ? `Besser als jedes deiner letzten ${trend.of} Turniere.`
                : trend.beaten === 0
                  ? `Schwächer als deine letzten ${trend.of} Turniere.`
                  : `Besser als ${trend.beaten} deiner letzten ${trend.of} Turniere.`}
            {/if}
          </p>
          <ul class="history">
            <li class="this">
              <span class="h-name">Dieses Turnier</span>
              <span class="g-bar" aria-hidden="true"><i style:width="{((s.per100 ?? 0) / maxPer100) * 100}%"></i></span>
              <span class="num">{bb(s.per100 ?? 0)}</span>
            </li>
            {#each report.history as h (h.tournament_id)}
              <li>
                <span class="h-name" title={h.name ?? undefined}>{dayShort(h.started_at)} · {h.name ?? `#${h.tournament_id}`}</span>
                <span class="g-bar quiet" aria-hidden="true"><i style:width="{((h.per100 ?? 0) / maxPer100) * 100}%"></i></span>
                <span class="num">{h.per100 === null ? "–" : bb(h.per100)}</span>
              </li>
            {/each}
          </ul>
        {:else}
          <EmptyState compact title="Noch kein früheres Turnier zum Vergleichen." text="Nach dem nächsten bewerteten Turnier steht hier, ob du besser wirst." />
        {/if}
      </Panel>

      <Panel title="Was als Nächstes?" sub="Aus deiner einen Rangliste über alle Hände – hier, was in diesem Turnier vorkam.">
        {#if report.next.length}
          <NextSteps steps={report.next} mode="first" {onLearn} onHands={onStepHands} note={here} />
        {:else}
          <EmptyState compact title="Nichts Dringendes." text="Keine Spot-Art hat in diesem Turnier etwas gekostet." />
        {/if}
      </Panel>
    </div>

    <Panel>
      <div class="coach">
        <AskCoach
          about={{ kind: "tournament", tournament_id: t.id }}
          presets={["Was sagt mein Score?", "Warum war mein teuerster Fehler falsch?", "Was soll ich als Nächstes üben?"]}
          onSetup={() => onCoach("setup")}
        />
        {#if report.coach_review}
          <Button size="sm" variant="quiet" onclick={() => onCoach("review")}>Frühere Coach-Besprechung öffnen</Button>
        {:else if notAnalyzable(t) === null}
          <Button size="sm" variant="quiet" title="Der Coach bespricht ausgewählte Hände ausführlich und gibt dir Übungen. Vorher siehst du die Kosten." onclick={() => onCoach("analyze")}
            >Ausführlich besprechen lassen</Button
          >
        {/if}
      </div>
    </Panel>
  {/if}
</div>

<style>
  .report {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    padding-top: var(--sp-5);
  }
  .top {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2) var(--sp-3);
  }
  .ico {
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .name {
    min-width: 0;
    font-size: var(--fs-lg);
    font-weight: var(--fw-semibold);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .date {
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .spacer {
    flex: 1;
  }
  .note {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2);
    margin: 0;
    padding: var(--sp-2) var(--sp-3);
    border: 1px dashed var(--line-2);
    border-radius: var(--r-md);
    font-size: var(--fs-md);
    color: var(--text-2);
  }

  /* The answer first: result, luck, decisions. */
  .answer {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    border: 1px solid var(--line);
    border-radius: var(--r-lg);
    background: var(--surface);
  }
  .cell {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    min-width: 0;
    padding: var(--sp-5) var(--sp-5) var(--sp-4);
  }
  .cell + .cell {
    border-left: 1px solid var(--line);
  }
  .eyebrow {
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .big {
    font-size: var(--fs-2xl);
    font-weight: var(--fw-semibold);
    line-height: var(--lh-tight);
  }
  .say {
    font-size: var(--fs-md);
    line-height: var(--lh-base);
    color: var(--text-2);
  }
  .say b {
    color: var(--text);
  }
  .luck {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: var(--sp-2) var(--sp-3);
  }
  .luck-label {
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .luck-bar {
    position: relative;
    display: flex;
    height: 10px;
    border-radius: var(--r-xs);
    background: linear-gradient(to right, transparent calc(50% - 1px), var(--line-2) calc(50% - 1px), var(--line-2) calc(50% + 1px), transparent calc(50% + 1px)),
      var(--surface-2);
  }
  .luck-bar i {
    display: block;
    height: 100%;
    border-radius: var(--r-xs);
    background: var(--pos);
    transform-origin: left;
    animation: grow-x var(--dur-slow) var(--ease-enter) both;
  }
  .luck-bar i.neg-bar {
    background: var(--neg);
  }
  .luck b {
    font-size: var(--fs-md);
    text-align: right;
  }

  .groups,
  .history {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }
  .groups li {
    display: grid;
    grid-template-columns: minmax(150px, 1.2fr) minmax(80px, 2fr) 76px;
    grid-template-rows: auto auto;
    align-items: center;
    column-gap: var(--sp-3);
  }
  .g-title {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-2);
    min-width: 0;
    font-size: var(--fs-base);
  }
  .g-bar {
    display: block;
    height: 8px;
    border-radius: var(--r-xs);
    background: var(--surface-2);
    overflow: hidden;
  }
  .g-bar i {
    display: block;
    height: 100%;
    background: var(--grade-mistake);
    transform-origin: left;
    animation: grow-x var(--dur-slow) var(--ease-enter) both;
  }
  .g-bar.quiet i {
    background: var(--text-3);
  }
  .g-cost {
    text-align: right;
    font-weight: var(--fw-semibold);
  }
  .g-meta {
    grid-column: 1 / -1;
    padding-left: 22px;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .unsure {
    color: var(--amber);
    cursor: help;
  }

  .pair {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--sp-4);
    align-items: start;
  }
  .moments {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }
  .moment {
    display: flex;
    flex-direction: column;
    gap: var(--sp-1);
    padding: var(--sp-3) var(--sp-4);
    border: 1px solid var(--line);
    border-left: 3px solid var(--grade-best);
    border-radius: var(--r-md);
    background: var(--surface-sunken);
  }
  .moment.bad {
    border-left-color: var(--grade-blunder);
  }
  .moment header {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2);
  }
  .rank {
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    background: var(--grade-blunder);
    color: var(--text-on-light);
    font-size: var(--fs-xs);
    font-weight: var(--fw-bold);
  }
  .cards {
    display: inline-flex;
    gap: 2px;
    font-size: var(--fs-base);
  }
  .where,
  .situation,
  .src {
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .moment p {
    margin: 0;
    font-size: var(--fs-md);
    line-height: var(--lh-base);
  }
  .situation {
    color: var(--text);
  }
  .moment .why {
    color: var(--text-2);
  }
  .cost {
    margin-left: var(--sp-1);
    color: var(--neg);
    font-weight: var(--fw-semibold);
  }
  .moment footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-2);
    margin-top: var(--sp-1);
  }

  .hands {
    display: grid;
    grid-template-columns: minmax(0, 300px) minmax(0, 1fr);
    gap: var(--sp-4);
    align-items: start;
  }
  .grid-box {
    min-width: 0;
    max-width: 420px;
  }
  .pin {
    display: flex;
    flex-direction: column;
    gap: var(--sp-1);
  }
  .pin p {
    margin: 0;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .pin-hands {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-1);
  }
  .hand-facts {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    margin: 0;
  }
  .hand-facts dt {
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .hand-facts dd {
    margin: 0;
    font-size: var(--fs-base);
  }

  .lead {
    margin: 0 0 var(--sp-3);
    font-weight: var(--fw-medium);
  }
  .history li {
    display: grid;
    grid-template-columns: minmax(0, 1.4fr) minmax(60px, 1fr) 64px;
    align-items: center;
    gap: var(--sp-3);
    font-size: var(--fs-md);
  }
  .history li > :last-child {
    text-align: right;
  }
  .history .this {
    font-weight: var(--fw-semibold);
  }
  .h-name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .here {
    margin: 0;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .coach {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--sp-3);
  }
  .coach > :global(section) {
    align-self: stretch;
  }

  @container (max-width: 1100px) {
    .pair {
      grid-template-columns: minmax(0, 1fr);
    }
  }
  @container (max-width: 900px) {
    .answer {
      grid-template-columns: minmax(0, 1fr);
    }
    .cell + .cell {
      border-left: 0;
      border-top: 1px solid var(--line);
    }
    .hands {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
