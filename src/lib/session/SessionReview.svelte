<script lang="ts">
  import { go } from "../nav/go";
  import { GRADE_OF } from "../analysis/labels";
  import { api } from "../api";
  import type { LearnTarget } from "../app/Leaks.svelte";
  import NextSteps from "../next/NextSteps.svelte";
  import type { NextStep, StepHands } from "../next/types";
  import type { Filter } from "../types";
  import { dayShort, time } from "../app/format";
  import type { RangeFilter } from "../ranges/types";
  import { NO_RANGE } from "../ranges/types";
  import { tournamentName } from "../stats";
  import Badge from "../ui/Badge.svelte";
  import Button from "../ui/Button.svelte";
  import Card from "../ui/Card.svelte";
  import EmptyState from "../ui/EmptyState.svelte";
  import { bb, cash, int, num } from "../ui/format";
  import GradeBadge from "../ui/GradeBadge.svelte";
  import GradeIcon from "../ui/GradeIcon.svelte";
  import MiniCard from "../ui/MiniCard.svelte";
  import { usePageHead } from "../ui/page.svelte";
  import ProgressRing from "../ui/ProgressRing.svelte";
  import RangeGrid, { type RangeCell } from "../ui/RangeGrid.svelte";
  import SectionHead from "../ui/SectionHead.svelte";
  import SourceNote from "../ui/SourceNote.svelte";
  import Term from "../ui/Term.svelte";
  import SiteMark from "../ui/SiteMark.svelte";
  import { severalSites } from "../app/sites.svelte";
  import Luck from "./Luck.svelte";
  import { delta, points } from "../skill/labels";
  import { WORDS } from "../ui/words";
  import { cardsOf, classCards, classWords, luckLine, swingLine } from "./labels";
  import type { ClassGap, RangePreset, SessionPage, SessionScope, SessionSpan } from "./types";

  let {
    refresh,
    filter,
    onReport,
    onHand,
    onRanges,
    onHands,
    onLearn,
    onStepHands,
    onSkill,
  }: {
    refresh: number;
    /** Opens the Spielwertung. */
    onSkill: () => void;
    /** The app's game and period: „Als Nächstes üben“ filters the overview's list to the session. */
    filter: Filter;
    onReport: (tournamentId: number) => void;
    /** Opens a hand of the session in the replayer at a decision. */
    onHand: (scope: SessionScope, handId: number, actionIndex: number) => void;
    onRanges: (scope: SessionScope, preset: RangePreset | null) => void;
    onHands: (scope: SessionScope, range: RangeFilter, label: string) => void;
    onLearn: (target: LearnTarget) => void;
    /** „Hände ansehen“ of a step of „Was als Nächstes?“. */
    onStepHands?: (hands: StepHands) => void;
  } = $props();

  const query = new URLSearchParams(window.location.search);
  let start = $state<string | null>(null);
  let page = $state<SessionPage | null>(null);
  let failed = $state(false);
  let retry = $state(0);
  /** The page of all luck examples ("Alle ansehen"). */
  let detail = $state(query.get("sub") === "luck");

  $effect(() => {
    void refresh;
    void retry;
    const s = start;
    let live = true;
    api
      .sessionReview(s)
      .then((p) => {
        if (!live) return;
        page = p;
        failed = false;
      })
      .catch(() => live && (failed = true));
    return () => (live = false);
  });

  const review = $derived(page?.review ?? null);
  const span = $derived(review?.span ?? null);
  const pre = $derived(review?.preflop ?? null);
  const rep = $derived(review?.report ?? null);

  const TODAY = new Date();
  const ymd = (d: Date) => `${d.getFullYear()}/${String(d.getMonth() + 1).padStart(2, "0")}/${String(d.getDate()).padStart(2, "0")}`;
  const YESTERDAY = new Date(TODAY.getTime() - 86_400_000);
  function dayName(ts: string): string {
    if (ts.startsWith(ymd(TODAY))) return "Heute";
    if (ts.startsWith(ymd(YESTERDAY))) return "Gestern";
    return dayShort(ts);
  }
  const several = $derived(severalSites());
  const sessionLabel = (s: SessionSpan) => `${dayName(s.start)}, ${time(s.start)}`;
  function spanLine(s: SessionSpan): string {
    const parts = [`${dayName(s.start)} · ${time(s.start)}–${time(s.end)}`];
    if (s.tournaments.length) parts.push(s.tournaments.length === 1 ? "1 Turnier" : `${s.tournaments.length} Turniere`);
    if (s.cash_hands) parts.push(`${int(s.cash_hands)} Cash-Hände`);
    parts.push(`${int(s.hands)} Hände`);
    if (s.sites?.length > 1) parts.push("PokerStars und GGPoker");
    return parts.join(" · ");
  }
  const scope = $derived<SessionScope | null>(span ? { since: span.start, until: span.end, label: `Session ${dayName(span.start)}, ${time(span.start)}–${time(span.end)}` } : null);

  usePageHead(() => ({
    sub: span ? `${start === null || start === page?.sessions[0]?.start ? "Deine letzte Session" : "Frühere Session"} · ${spanLine(span)}` : undefined,
    tabs: (page?.sessions.length ?? 0) > 1 ? page!.sessions.map((s) => ({ id: s.start, label: sessionLabel(s), hint: spanLine(s) })) : undefined,
    value: span?.start,
    label: "Sessions",
    onchange: (id) => {
      start = id;
      detail = false;
    },
    end: review ? analysisState : undefined,
  }));

  // Result in money: tournaments with a known result, summed per currency.
  const money = $derived.by(() => {
    const by = new Map<string, number>();
    let open = 0;
    for (const t of page?.tournaments ?? []) {
      if (t.profit === null) {
        open++;
        continue;
      }
      const c = t.currency ?? "";
      by.set(c, (by.get(c) ?? 0) + t.profit);
    }
    return { sums: [...by.entries()], open };
  });
  const signedCash = (v: number, c: string) => `${v > 0 ? "+" : ""}${cash(v, c || null)}`;

  // Preflop: the track from 0 to a round share above both values.
  const scaleMax = $derived(Math.max(30, Math.ceil((Math.max(pre?.you_pct ?? 0, pre?.model_pct ?? 0) + 8) / 10) * 10));
  const at = (v: number) => `${(100 * v) / scaleMax}%`;
  const diff = $derived(pre?.you_pct != null && pre.model_pct != null ? pre.you_pct - pre.model_pct : 0);
  const byClass = $derived(new Map((pre?.classes ?? []).map((c) => [c.class, c])));
  const gapMax = $derived(Math.max(1, ...(pre?.classes ?? []).map((c) => Math.abs(c.played - c.model))));
  const cells = $derived<RangeCell[]>(
    Array.from({ length: 169 }, (_, i) => {
      const c = byClass.get(i);
      if (!c) return null;
      const g = c.played - c.model;
      if (Math.abs(g) < 0.5) return { color: "var(--surface-3)", note: note(c) };
      const share = Math.round(30 + 70 * Math.min(1, Math.abs(g) / gapMax));
      return { color: `color-mix(in srgb, var(${g > 0 ? "--range-wide" : "--range-tight"}) ${share}%, var(--surface-2))`, note: note(c) };
    }),
  );
  function note(c: ClassGap): string {
    const times = (n: number) => (n === 1 ? "1 Mal" : `${int(n)} Mal`);
    return `${c.name}: ${times(c.hands)} bekommen, ${times(c.played)} gespielt – das Modell etwa ${times(Math.round(c.model))}`;
  }
  const VERDICT_TONE = ["good", "good", "warn", "bad"] as const;

  const sessionRange = (preset: RangePreset | null): RangeFilter => ({
    ...NO_RANGE,
    history: preset?.history ?? null,
    position: preset?.position ?? null,
    vs_position: preset?.vs ?? null,
  });

  /** „Was als Nächstes?“, filtered to this session: the overview's first step when it came up here. */
  let steps = $state<NextStep[]>([]);
  $effect(() => {
    void refresh;
    const f = { ...filter, tournament_id: null };
    const s = span;
    if (!s || review?.locked) {
      steps = [];
      return;
    }
    let live = true;
    api.nextSteps(f, { game: null, since: s.start, until: s.end, tournament_id: null }).then(
      (x) => live && (steps = x),
      () => live && (steps = []),
    );
    return () => (live = false);
  });
  /** Without an entry from the list, the session's most striking preflop pattern. */
  const pattern = $derived(pre?.patterns.find((x) => x.practice) ?? null);
  const analysing = $derived(!!rep && rep.unanalyzed > 0);
  const preflopReady = $derived(!!rep && rep.unanalyzed === 0);
  const STREET = { preflop: "Preflop", flop: "Flop", turn: "Turn", river: "River" } as const;
</script>

{#snippet analysisState()}
      <span class="sr-status" role="status">
        {#if review?.locked}
          <SourceNote kind="locked" />
        {:else if rep}
          <span class="sr-state">
            <ProgressRing size={18} stroke={3} value={preflopReady ? 1 : 1 - rep!.unanalyzed / Math.max(1, rep!.hands)} tone={preflopReady ? "good" : "info"} label="Analyse vor dem Flop" />
            {preflopReady ? "Vor dem Flop fertig" : `Analyse läuft: noch ${int(rep!.unanalyzed)} Hände`}
          </span>
          {#if rep!.pending > 0}
            <span class="sr-state" title="Flop-Spots rechnet Ringside im Hintergrund nach. Bis dahin sind die Zahlen nach dem Flop vorläufig.">
              <ProgressRing size={18} stroke={3} value={null} tone="info" label="Flop-Spots" />
              {rep!.pending === 1 ? "1 Flop-Spot wird noch gerechnet" : `${int(rep!.pending)} Flop-Spots werden noch gerechnet`}
            </span>
          {/if}
        {/if}
      </span>
{/snippet}

{#snippet nextTile()}
        <section class="sr-tile sr-next" aria-labelledby="sr-next-title">
          <SectionHead id="sr-next-title" title="Als Nächstes üben" level={3} />
          {#if steps.length}
            <NextSteps {steps} mode="first" label="" {onLearn} onHands={onStepHands} />
          {:else if pattern?.practice}
            {@const key = pattern.practice}
            <p class="sr-next-what"><b>Dein auffälligstes Muster</b></p>
            <p class="sr-say">{pattern.text}</p>
            <span class="sr-more"><Button variant="primary" onclick={() => onLearn({ spot: key })}>Üben</Button></span>
          {:else}
            <p class="sr-say">Noch nichts Bestimmtes – in dieser Session hat dich keine Spot-Art spürbar Chips gekostet.</p>
          {/if}
        </section>
{/snippet}

{#snippet classRow(classes: number[], title: string, color: string)}
  {#if classes.length}
    <div class="sr-keys">
      <span class="sr-keys-head"><i class="sr-dot" style="background:var({color})"></i>{title}</span>
      <span class="sr-key-list">
        {#each classes.slice(0, 3) as i (i)}
          {@const c = byClass.get(i)}
          {#if c}<span class="sr-key" title="{classWords(c.name)}. {note(c)}">{#each classCards(c.name) as card (card)}<MiniCard {card} />{/each}</span>{/if}
        {/each}
      </span>
    </div>
  {/if}
{/snippet}

<div class="page sr">
  {#if failed}
    <EmptyState page tone="error" title="Der Rückblick konnte nicht geladen werden" text="Deine Hände sind sicher gespeichert. Versuch es noch einmal; bleibt der Fehler, starte Ringside neu.">
      {#snippet action()}<Button variant="primary" onclick={() => retry++}>Erneut laden</Button>{/snippet}
    </EmptyState>
  {:else if !page}
    <EmptyState page tone="loading" title="Rückblick wird geladen …" />
  {:else if !review || !span || !pre || !rep}
    <EmptyState page title="Noch keine Session" text="Spiel ein paar Hände bei PokerStars oder importier deine GGPoker-Hände in den Einstellungen. Danach steht hier, wie deine letzte Session lief und was du als Nächstes üben kannst.">
      {#snippet action()}<Button onclick={() => go("play")}>Solange: Hände gegen Bots spielen</Button>{/snippet}
    </EmptyState>
  {:else if detail && !review.locked}
    <Luck {review} onBack={() => (detail = false)} onHand={(id, at) => scope && onHand(scope, id, at)} />
  {:else}
    <div class="sr-strip" aria-label="Turniere dieser Session">
      {#each page.tournaments as t (t.id)}
        <Card pad="sm" onclick={() => onReport(t.id)} title="{tournamentName(t)} – Turnier-Bericht öffnen">
          <span class="sr-t">
            <span class="sr-t-name">{#if several}<SiteMark site={t.site} />{" "}{/if}{tournamentName(t)}</span>
            <span class="sr-t-meta">
              {#if t.profit === null}<Badge size="sm" dot tone="info">läuft</Badge>{:else if t.place !== null}Platz {int(t.place)}{t.entrants ? ` von ${int(t.entrants)}` : ""}{:else}beendet{/if}
              {#if t.profit !== null}<b class="sr-money" class:sr-up={t.profit > 0} class:sr-down={t.profit < 0}>{signedCash(t.profit, t.currency ?? "")}</b>{/if}
            </span>
          </span>
        </Card>
      {/each}
      {#if span.cash_hands}
        <Card pad="sm"><span class="sr-t"><span class="sr-t-name">Cash</span><span class="sr-t-meta">{int(span.cash_hands)} Hände</span></span></Card>
      {/if}
      {#if page.skill}
        {@const s = page.skill}
        <Card pad="sm" onclick={onSkill} title="Deine Spielwertung öffnen">
          <span class="sr-t" data-session-skill>
            <span class="sr-t-name">{WORDS.skill_rating.word}</span>
            <span class="sr-t-meta">
              {#if s.delta !== null}<b class="sr-money" class:sr-up={s.delta > 0} class:sr-down={s.delta < 0}>{delta(s.delta)}</b>{/if}
              <span>{s.delta !== null ? "jetzt " : ""}<b class="sr-money">{points(s.rating)}</b>{` · ${s.tier}`}</span>
            </span>
          </span>
        </Card>
      {/if}
    </div>

    {#if review.locked}
      <section class="sr-locked">
        <svg viewBox="0 0 24 24" width="28" height="28" aria-hidden="true"><rect x="5" y="10.5" width="14" height="9.5" rx="2" /><path d="M8.5 10.5V8a3.5 3.5 0 0 1 7 0v2.5M12 14.5v2" /></svg>
        <h2 class="sr-locked-title">Am Tisch – dein Rückblick kommt, sobald du den Poker-Client schließt.</h2>
        <p class="sr-locked-text">Bewertungen, Vergleiche und Tipps sind gesperrt, solange ein Poker-Client läuft. So verlangen es die Regeln von PokerStars und GGPoker.</p>
        <dl class="sr-facts">
          <div><dt>Hände</dt><dd>{int(rep.hands)}</dd></div>
          {#if pre.you_pct !== null}<div><dt>Gespielt</dt><dd>{num(pre.you_pct, 0)} %</dd></div>{/if}
          <div><dt>Ergebnis</dt><dd class:sr-up={rep.luck.net_bb > 0} class:sr-down={rep.luck.net_bb < 0}>{bb(rep.luck.net_bb, { sign: true })}</dd></div>
          {#each money.sums as [c, v] (c)}<div><dt>Geld</dt><dd class:sr-up={v > 0} class:sr-down={v < 0}>{signedCash(v, c)}</dd></div>{/each}
        </dl>
      </section>
    {:else}
      <div class="sr-grid">
        <div class="sr-col">
        <section class="sr-tile sr-pre" aria-labelledby="sr-pre-title">
          <SectionHead id="sr-pre-title" title="Vor dem Flop" level={3}>
            {#snippet actions()}
              {#if pre.verdict}<Badge tone={VERDICT_TONE[pre.verdict.level]}>{pre.verdict.title}</Badge>{/if}
              {#if pre.sample === "rough"}<Badge dashed size="sm" title="Je mehr Hände, desto sicherer die Muster.">bei {int(pre.compared)} Händen noch grob</Badge>{/if}
            {/snippet}
          </SectionHead>

          {#if pre.compared === 0}
            <p class="sr-lead">{analysing ? "Die Analyse läuft – in ein paar Sekunden steht hier der Vergleich mit dem Modell." : "In dieser Session gab es keine Hand, die das Modell vergleichen kann."}</p>
          {:else}
            <p class="sr-lead">
              Du hast <b>{num(pre.you_pct ?? 0, 0)} %</b> deiner Hände gespielt – das Modell würde etwa <b>{num(pre.model_pct ?? 0, 0)} %</b> spielen.
            </p>
            <div class="sr-track" role="img" aria-label="Du {num(pre.you_pct ?? 0, 0)} %, Modell {num(pre.model_pct ?? 0, 0)} %">
              <span class="sr-rail"></span>
              <span class="sr-you" style="width:{at(pre.you_pct ?? 0)}"></span>
              <span
                class="sr-gap"
                class:sr-gap-wide={diff > 0}
                style="left:{at(Math.min(pre.you_pct ?? 0, pre.model_pct ?? 0))}; width:{at(Math.abs(diff))}"
              ></span>
              <span class="sr-mark sr-mark-you" style="left:{at(pre.you_pct ?? 0)}"><b>Du</b> {num(pre.you_pct ?? 0, 0)} %</span>
              <span class="sr-scale sr-scale-start" aria-hidden="true">0 %</span>
              <span class="sr-scale sr-scale-end" aria-hidden="true">{scaleMax} %</span>
              <span class="sr-mark sr-mark-model" style="left:{at(pre.model_pct ?? 0)}"><Term tip="Ringsides Preflop-Modell und bei kurzen Stacks das Push/Fold-Gleichgewicht – für genau deine Plätze, Stacks und Situationen, mit deinen Karten.">Modell</Term> {num(pre.model_pct ?? 0, 0)} %</span>
            </div>
            {#if pre.verdict}<p class="sr-say">{pre.verdict.line}</p>{/if}

            {#if pre.sample === "few"}
              <p class="sr-hint">Noch zu wenige Hände für ein Urteil: {int(pre.compared)} verglichen. Ab 30 nennt Ringside, welche Hände und Situationen auffallen.</p>
            {:else}
              {#if pre.patterns.length}
                <ul class="sr-patterns">
                  {#each pre.patterns as p (p.text)}
                    <li>
                      <span class="sr-arrow" class:sr-arrow-wide={p.wide} aria-label={p.wide ? "zu oft" : "zu selten"}>{p.wide ? "↑" : "↓"}</span>
                      <span class="sr-pattern-text">{p.text}</span>
                      <span class="sr-pattern-do">
                        {#if scope}<Button size="sm" variant="quiet" onclick={() => onRanges(scope, { history: p.history, position: p.position, vs: p.vs_position })}>Ansehen</Button>{/if}
                        {#if p.practice}{@const key = p.practice}<Button size="sm" variant="secondary" onclick={() => onLearn({ spot: key })}>Üben</Button>{/if}
                      </span>
                    </li>
                  {/each}
                </ul>
              {/if}
              <div class="sr-hands">
                <div class="sr-mini">
                  <RangeGrid {cells} size="sm" label="Deine Hände dieser Session gegen das Modell" />
                  <span class="sr-legend"><i class="sr-dot sr-dot-same"></i>wie das Modell · leer: nicht bekommen · s = gleiche Farbe, o = verschiedene Farben</span>
                </div>
                <div class="sr-keycol">
                  {@render classRow(pre.tight, "Laut Modell öfter spielen", "--range-tight")}
                  {@render classRow(pre.wide, "Laut Modell eher folden", "--range-wide")}
                  {#if !pre.tight.length && !pre.wide.length}<p class="sr-say">Keine Hand weicht deutlich ab.</p>{/if}
                </div>
              </div>

            {/if}
          {/if}

          {#if pre.compared > 0 && scope}
            <footer class="sr-foot">
              <Button size="sm" onclick={() => onRanges(scope, null)}>Deine gespielten Starthände</Button>
              <Button size="sm" variant="quiet" onclick={() => onHands(scope, sessionRange(null), "Preflop-Entscheidungen dieser Session")}>Die Hände dazu</Button>
              <span class="sr-src"><SourceNote kind={pre.equilibrium === pre.compared ? "equilibrium" : "model"} hint="Bei kurzen Stacks vergleicht Ringside mit dem Push/Fold-Gleichgewicht, sonst mit dem Preflop-Modell." /></span>
            </footer>
          {/if}
        </section>

        </div>
        <div class="sr-col">
        <section class="sr-tile sr-res" aria-labelledby="sr-res-title">
          <SectionHead id="sr-res-title" title="Ergebnis und Glück" level={3} />
          <div class="sr-result">
            {#each money.sums as [c, v] (c)}
              <strong class="sr-big" class:sr-up={v > 0} class:sr-down={v < 0}>{signedCash(v, c)}</strong>
            {:else}
              <strong class="sr-big" class:sr-up={rep.luck.net_bb > 0} class:sr-down={rep.luck.net_bb < 0}>{bb(rep.luck.net_bb, { sign: true })}</strong>
            {/each}
            <span class="sr-result-sub">
              {#if money.sums.length}{bb(rep.luck.net_bb, { sign: true })} in Chips{/if}{#if money.open}{money.sums.length ? " · " : ""}{money.open === 1 ? "1 Turnier läuft noch" : `${money.open} Turniere laufen noch`}{/if}
            </span>
          </div>
          <p class="sr-say">{luckLine(rep.luck)}</p>
        </section>

        <!-- Right under the result, so the one thing to do next is in view without scrolling. -->
        {@render nextTile()}

        {#if review.bad_luck.length || review.good_luck.length}
          <section class="sr-tile sr-luck" aria-labelledby="sr-luck-title">
            <SectionHead id="sr-luck-title" title="Glück oder gut gespielt?" level={3} />
            <div class="sr-swings">
              {#if review.bad_luck[0]}
                {@const s = review.bad_luck[0]}
                <article class="sr-swing" style="--sr-grade:var(--grade-{s.rating ? GRADE_OF[s.rating] : 'best'})">
                  <span class="sr-swing-mark">{#if s.rating}<GradeIcon grade={GRADE_OF[s.rating]} size={18} />{/if}</span>
                  <div class="sr-swing-body">
                    <span class="sr-swing-kind">Gut entschieden, Pech gehabt</span>
                    <span class="sr-swing-hand">{#each cardsOf(s.hero_cards) as c (c)}<MiniCard card={c} />{/each}<b>{s.played}</b><span class="sr-swing-net sr-down" title="Ergebnis der Hand">Hand {bb(s.net_bb, { sign: true })}</span></span>
                    <span class="sr-swing-line">{swingLine(s, true)}</span>
                  </div>
                </article>
              {/if}
              {#if review.good_luck[0]}
                {@const s = review.good_luck[0]}
                <article class="sr-swing" style="--sr-grade:var(--grade-{s.rating ? GRADE_OF[s.rating] : 'mistake'})">
                  <span class="sr-swing-mark">{#if s.rating}<GradeIcon grade={GRADE_OF[s.rating]} size={18} />{/if}</span>
                  <div class="sr-swing-body">
                    <span class="sr-swing-kind">Glück gehabt</span>
                    <span class="sr-swing-hand">{#each cardsOf(s.hero_cards) as c (c)}<MiniCard card={c} />{/each}<b>{s.played}</b><span class="sr-swing-net sr-up" title="Ergebnis der Hand">Hand {bb(s.net_bb, { sign: true })}</span></span>
                    <span class="sr-swing-line">{swingLine(s, false)}</span>
                  </div>
                </article>
              {/if}
            </div>
            <span class="sr-more"><Button size="sm" variant="quiet" onclick={() => (detail = true)}>Alle Beispiele ansehen ({review.bad_luck.length + review.good_luck.length})</Button></span>
          </section>
        {/if}

        <section class="sr-tile sr-worst" aria-labelledby="sr-worst-title">
          <SectionHead id="sr-worst-title" title="Deine teuersten Entscheidungen" level={3} />
          {#if rep.worst.length}<p class="sr-note">Die Beträge sagen, was dich die Entscheidung im Schnitt kostet – nicht, wie die Hand ausging.</p>{/if}
          {#if rep.worst.length}
            <ol class="sr-moments">
              {#each rep.worst as m (m.hand_id + ":" + m.action_index)}
                <li>
                  <span class="sr-cards">{#each cardsOf(m.hero_cards) as c (c)}<MiniCard card={c} />{/each}</span>
                  <span class="sr-moment-text">
                    <span class="sr-moment-what">{m.played} statt <b>{m.best ?? "–"}</b></span>
                    <span class="sr-moment-where">
                      {#if m.rating}<GradeBadge grade={GRADE_OF[m.rating]} cost={m.loss_bb} size="sm" />{/if}
                      <span class="sr-moment-place">{m.position ?? "?"} · {STREET[m.street]}{m.situation !== STREET[m.street] ? ` · ${m.situation}` : ""}</span>
                    </span>
                  </span>
                  {#if scope}<Button size="sm" onclick={() => onHand(scope, m.hand_id, m.action_index)}>Abspielen</Button>{/if}
                </li>
              {/each}
            </ol>
          {:else}
            <p class="sr-say">{analysing ? "Die Analyse läuft noch." : "Kein teurer Fehler in dieser Session – stark."}</p>
          {/if}
        </section>

        </div>
      </div>
    {/if}
  {/if}
</div>

<style>
  .sr {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
  }

  /* Tournaments and analysis state in one quiet row. */
  .sr-strip {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
    gap: var(--sp-2);
  }
  .sr-t {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .sr-t-name {
    display: -webkit-box;
    overflow: hidden;
    font-size: var(--fs-md);
    font-weight: var(--fw-semibold);
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
  }
  .sr-t-meta {
    display: flex;
    gap: var(--sp-2);
    align-items: center;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .sr-status {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-1) var(--sp-4);
    align-items: center;
    justify-content: flex-end;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .sr-state {
    display: inline-flex;
    gap: var(--sp-2);
    align-items: center;
  }

  .sr-up {
    color: var(--pos);
  }
  .sr-down {
    color: var(--neg);
  }
  .sr-money {
    font-family: var(--font-num);
    font-weight: var(--fw-semibold);
  }

  /* Preflop takes the left column; result, mistakes and practice stack on the right. */
  /* Preflop and the next step on the left, result and mistakes on the right; one column when narrow. */
  .sr-grid {
    display: grid;
    grid-template-columns: minmax(0, 1.25fr) minmax(0, 1fr);
    gap: var(--sp-4);
    align-items: start;
  }
  .sr-col {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    min-width: 0;
  }
  @media (max-width: 1180px) {
    .sr-grid {
      grid-template-columns: minmax(0, 1fr);
    }
  }

  .sr-tile {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    min-width: 0;
    padding: var(--sp-5);
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: var(--r-lg);
  }
  .sr-lead {
    margin: 0;
    font-size: var(--fs-2xl);
    line-height: var(--lh-tight);
    font-weight: var(--fw-medium);
    text-wrap: balance;
  }
  .sr-lead b {
    font-family: var(--font-num);
    font-weight: var(--fw-bold);
  }
  .sr-say {
    margin: 0;
    font-size: var(--fs-base);
    line-height: var(--lh-base);
    color: var(--text-2);
  }
  .sr-note {
    margin: calc(-1 * var(--sp-2)) 0 0;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .sr-mini {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }
  .sr-legend {
    display: flex;
    gap: var(--sp-2);
    align-items: center;
    font-size: var(--fs-xs);
    color: var(--text-2);
  }
  .sr-dot-same {
    background: var(--surface-3);
    box-shadow: inset 0 0 0 1px var(--line-2);
  }
  .sr-key {
    display: inline-flex;
    gap: 2px;
    cursor: help;
  }
  .sr-pattern-do {
    display: inline-flex;
    gap: var(--sp-1);
  }
  .sr-hint {
    margin: 0;
    padding: var(--sp-3) var(--sp-4);
    font-size: var(--fs-md);
    color: var(--text-2);
    background: var(--surface-sunken);
    border: 1px dashed var(--line-2);
    border-radius: var(--r-md);
  }

  /* You against the model on one track: the gap between the marks carries the direction's colour. */
  .sr-track {
    position: relative;
    height: 52px;
    margin: var(--sp-1) 0 var(--sp-2);
  }
  .sr-rail,
  .sr-you,
  .sr-gap {
    position: absolute;
    top: 22px;
    height: 8px;
    border-radius: var(--r-pill);
  }
  .sr-rail {
    right: 0;
    left: 0;
    background: var(--surface-3);
  }
  .sr-you {
    left: 0;
    background: var(--text);
    transition: width var(--dur-slow) var(--ease-standard);
  }
  .sr-gap {
    background: var(--range-tight);
    transition:
      left var(--dur-slow) var(--ease-standard),
      width var(--dur-slow) var(--ease-standard);
  }
  .sr-gap.sr-gap-wide {
    background: var(--range-wide);
  }
  .sr-scale {
    position: absolute;
    top: 36px;
    font-family: var(--font-num);
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .sr-scale-start {
    left: 0;
  }
  .sr-scale-end {
    right: 0;
  }
  .sr-mark {
    position: absolute;
    font-size: var(--fs-sm);
    font-family: var(--font-num);
    white-space: nowrap;
    transform: translateX(-50%);
  }
  .sr-mark::after {
    position: absolute;
    left: 50%;
    width: 2px;
    height: 14px;
    content: "";
    background: currentColor;
  }
  .sr-mark-you {
    top: 0;
    color: var(--text);
  }
  .sr-mark-you::after {
    top: 17px;
  }
  .sr-mark-model {
    top: 36px;
    color: var(--text-2);
  }
  .sr-mark-model::after {
    top: -8px;
    height: 7px;
  }

  .sr-hands {
    display: grid;
    grid-template-columns: minmax(220px, 460px) minmax(0, 1fr);
    gap: var(--sp-5);
    align-items: center;
    padding-top: var(--sp-3);
    border-top: 1px solid var(--line);
  }
  .sr-keycol {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
  }
  .sr-keys {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }
  .sr-keys-head {
    display: flex;
    gap: var(--sp-2);
    align-items: center;
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .sr-dot {
    width: 10px;
    height: 10px;
    border-radius: var(--r-xs);
  }
  .sr-key-list {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-3);
    font-size: var(--fs-lg);
  }

  .sr-patterns {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    margin: 0;
    padding: var(--sp-3) 0 0;
    list-style: none;
    border-top: 1px solid var(--line);
  }
  .sr-patterns li {
    display: grid;
    grid-template-columns: 20px minmax(0, 1fr) auto;
    gap: var(--sp-2);
    align-items: baseline;
    font-size: var(--fs-base);
    line-height: var(--lh-base);
  }
  .sr-arrow {
    font-weight: var(--fw-bold);
    color: var(--range-tight);
  }
  .sr-arrow.sr-arrow-wide {
    color: var(--range-wide);
  }
  .sr-foot {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-2);
    align-items: center;
    padding-top: var(--sp-3);
    border-top: 1px solid var(--line);
  }
  .sr-src {
    margin-left: auto;
  }

  .sr-result {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-1) var(--sp-3);
    align-items: baseline;
  }
  .sr-big {
    font-family: var(--font-num);
    font-size: var(--fs-4xl);
    font-weight: var(--fw-bold);
    line-height: var(--lh-tight);
  }
  .sr-result-sub {
    font-size: var(--fs-md);
    color: var(--text-2);
  }

  /* One example per side: the mark says whether the decision was smart, the number what happened. */
  .sr-swings {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: var(--sp-2);
  }
  .sr-swing {
    display: flex;
    gap: var(--sp-3);
    padding: var(--sp-3);
    background: var(--surface-2);
    border-radius: var(--r-md);
    box-shadow: inset 3px 0 0 var(--sr-grade);
  }
  .sr-swing-mark {
    display: grid;
    flex: none;
    place-items: center;
    width: 18px;
    height: 20px;
  }
  .sr-swing-body {
    display: flex;
    flex-direction: column;
    flex: 1;
    gap: var(--sp-1);
    min-width: 0;
  }
  .sr-swing-kind {
    font-size: var(--fs-sm);
    font-weight: var(--fw-semibold);
    color: var(--text-2);
  }
  .sr-swing-net {
    margin-left: auto;
    font-family: var(--font-num);
    font-size: var(--fs-sm);
    white-space: nowrap;
  }
  .sr-swing-hand {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-1) var(--sp-2);
    align-items: center;
    font-size: var(--fs-md);
  }
  .sr-swing-line {
    font-size: var(--fs-md);
    line-height: var(--lh-base);
    color: var(--text-2);
  }

  .sr-moments {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .sr-moments li {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    gap: var(--sp-3);
    align-items: center;
    padding: var(--sp-2) 0;
    border-top: 1px solid var(--line);
  }
  .sr-moments li:first-child {
    border-top: 0;
  }
  .sr-cards {
    display: flex;
    gap: 2px;
  }
  .sr-moment-text {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .sr-moment-what {
    font-size: var(--fs-base);
  }
  .sr-moment-text {
    gap: var(--sp-1);
  }
  .sr-moment-where {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-1) var(--sp-2);
    align-items: center;
    min-width: 0;
  }
  .sr-moment-place {
    font-size: var(--fs-sm);
    color: var(--text-2);
  }

  .sr-more {
    align-self: flex-start;
  }
  .sr-next-what {
    margin: 0;
    font-size: var(--fs-lg);
  }

  .sr-locked {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    align-items: center;
    padding: var(--sp-10) var(--sp-6);
    text-align: center;
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: var(--r-lg);
  }
  .sr-locked svg {
    fill: none;
    stroke: var(--text-2);
    stroke-width: 1.5;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .sr-locked-title {
    margin: 0;
    font-size: var(--fs-xl);
    font-weight: var(--fw-semibold);
  }
  .sr-locked-text {
    max-width: 520px;
    margin: 0;
    font-size: var(--fs-base);
    color: var(--text-2);
  }
  .sr-facts {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-8);
    justify-content: center;
    margin: var(--sp-4) 0 0;
  }
  .sr-facts dt {
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .sr-facts dd {
    margin: 0;
    font-family: var(--font-num);
    font-size: var(--fs-2xl);
    font-weight: var(--fw-semibold);
  }

  @media (prefers-reduced-motion: reduce) {
    .sr-you,
    .sr-gap {
      transition: none;
    }
  }
</style>
