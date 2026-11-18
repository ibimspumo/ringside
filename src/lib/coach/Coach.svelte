<script lang="ts">
  import { onMount, tick } from "svelte";
  import { api, inTauri } from "../api";
  import ClientLock from "../app/ClientLock.svelte";
  import type { LearnTarget } from "../app/Leaks.svelte";
  import { dateTime, dayShort } from "../app/format";
  import { nextLevel, trainableKind } from "../learn/catalog";
  import { ARTICLE_BY_ID } from "../learn/wissen/articles";
  import type { LearnProgress, TournamentRow } from "../types";
  import { coachApi, effortName, errorText, preview } from "./api";
  import Button from "../ui/Button.svelte";
  import Card from "../ui/Card.svelte";
  import EmptyState from "../ui/EmptyState.svelte";
  import CoachSettings from "./CoachSettings.svelte";
  import KeyForm from "./KeyForm.svelte";
  import { notAnalyzable, shortModel } from "./meaning";
  import { appear, leave, rise } from "./motion";
  import ReportView, { type Practice } from "./ReportView.svelte";
  import RunPanel from "./RunPanel.svelte";
  import SpotDrill from "./SpotDrill.svelte";
  import SpotRead from "./SpotRead.svelte";
  import type { Choice } from "./drill";
  import type { CoachReport, CoachStatus, ConceptCount, ReportListItem } from "./types";

  let {
    analyze,
    tournaments,
    onAnalyzeHandled,
    onHand,
    onTraining,
  }: {
    /** A tournament to analyse, set from the tournaments page. */
    analyze: number | null;
    tournaments: TournamentRow[];
    onAnalyzeHandled: () => void;
    onHand: (tournament: number, hand: number) => void;
    /** Opens the learning area, at an exercise or article when one fits. */
    onTraining: (target: LearnTarget | null) => void;
  } = $props();

  let status = $state<CoachStatus | null>(null);
  let list = $state<ReportListItem[] | null>(null);
  let selected = $state<number | null>(null);
  let report = $state<CoachReport | null>(null);
  let runFor = $state<number | null>(null);
  let running = $state(false);
  let failed = $state<string | null>(null);
  let setupOpen = $state(preview.setup);
  let concepts = $state<ConceptCount[]>([]);
  let progress = $state<LearnProgress | null>(null);
  /** A spot of the report opened to practise or to read up on, by its 1-based number. */
  let focus = $state<{ mode: "drill" | "read"; n: number } | null>(null);
  let returnTo = $state<number | null>(null);
  /** Spots practised in this visit, by "<tournament>:<n>": the answer and whether it matched. */
  let main = $state<HTMLElement>();
  let practised = $state<Record<string, { answer: Choice; right: boolean | null }>>({});

  function openSpot(mode: "drill" | "read", n: number) {
    focus = { mode, n };
    returnTo = null;
    showTask();
  }

  /** Brings the task just opened into view; in narrow windows it moves above the lists. */
  function showTask() {
    tick().then(() => main?.scrollIntoView({ block: "start" }));
  }

  function back() {
    returnTo = focus?.n ?? null;
    focus = null;
  }

  const titleOf = (id: number, item?: { name: string | null; game: string }) => {
    const t = tournaments.find((x) => x.id === id);
    return item?.name || t?.name || item?.game || t?.game || `Turnier ${id}`;
  };

  /** Newest finished tournaments the coach can analyse and has not yet. */
  const suggestions = $derived(
    list === null
      ? []
      : tournaments
          .filter((t) => notAnalyzable(t) === null && !list!.some((r) => r.tournament_id === t.id))
          .sort((a, b) => b.started_at.localeCompare(a.started_at))
          .slice(0, 3),
  );

  /** Where to practise or read up on a concept of the reports. */
  function practice(concept: string): Practice {
    const c = concepts.find((x) => x.id === concept);
    if (!c) return { train: null, articles: [] };
    const kind = trainableKind(c);
    return {
      train: kind ? { kind, level: nextLevel(progress?.skills.find((s) => s.id === kind)) } : null,
      articles: c.articles.flatMap((a) => (ARTICLE_BY_ID[a] ? [{ id: a, title: ARTICLE_BY_ID[a].title }] : [])),
    };
  }

  async function loadList(select: number | null = selected) {
    try {
      list = await coachApi.reports(tournaments);
      failed = null;
      selected = select !== null && list.some((r) => r.tournament_id === select) ? select : (list[0]?.tournament_id ?? null);
    } catch (e) {
      failed = errorText(e);
    }
    coachApi.concepts().then(
      (c) => (concepts = c),
      () => {},
    );
  }

  $effect(() => {
    const id = selected;
    if (id === null) {
      report = null;
      return;
    }
    let live = true;
    coachApi
      .report(id)
      .then((r) => live && (report = r))
      .catch((e) => live && (failed = errorText(e)));
    return () => (live = false);
  });

  $effect(() => {
    if (analyze === null) return;
    runFor = analyze;
    setupOpen = false;
    onAnalyzeHandled();
  });

  const loadStatus = () => coachApi.status().then((s) => (status = s));

  onMount(() => {
    loadStatus();
    loadList().then(() => {
      if (preview.run && list?.length) runFor = list[0].tournament_id;
    });
    api.learnProgress().then(
      (p) => (progress = p),
      () => {},
    );
  });

  function start(id: number) {
    setupOpen = false;
    focus = null;
    runFor = id;
    showTask();
  }

  function done(r: CoachReport) {
    runFor = null;
    running = false;
    report = r;
    loadList(r.tournament_id);
  }

  async function open(e: MouseEvent) {
    if (!inTauri) return;
    e.preventDefault();
    const { openUrl } = await import("@tauri-apps/plugin-opener");
    openUrl((e.currentTarget as HTMLAnchorElement).href);
  }

  async function remove(id: number) {
    await coachApi.deleteReport(id);
    selected = null;
    loadList(null);
  }
</script>

{#snippet bar(spots: number, mistakes: number)}
  <span class="mini" aria-hidden="true">
    <i class="bad" style="flex-grow:{mistakes}"></i><i class="good" style="flex-grow:{spots - mistakes}"></i>
  </span>
{/snippet}

{#snippet loadFailed(message: string)}
  <EmptyState tone="error" title="Die Berichte lassen sich gerade nicht laden." text={message}>
    {#snippet action()}<Button onclick={() => loadList()}>Erneut laden</Button>{/snippet}
  </EmptyState>
{/snippet}

<ClientLock feature="Der Coach">
  <div class="page wide">
    {#if list}
      <div class="layout" class:busy={runFor !== null || focus !== null || setupOpen || !status?.has_key}
        class:drilling={focus?.mode === "drill"}>
        <aside class="rail">
          <Card pad="sm" aria-labelledby="start-title">
            <h2 id="start-title" class="rail-title">Turnier besprechen</h2>
            {#if status && !status.has_key}
              <p class="sub pad">Erst den Key von OpenRouter hinterlegen, dann kannst du hier ein Turnier besprechen lassen.</p>
            {/if}
            {#if suggestions.length}
              <ul>
                {#each suggestions as t (t.id)}
                  <li class="start">
                    <span class="what">
                      <span class="name" title={titleOf(t.id)}>{titleOf(t.id)}</span>
                      <span class="sub nowrap">{dayShort(t.started_at)} · <span class="num">{t.hands}</span> Hände</span>
                    </span>
                    <Button
                      size="sm"
                      disabled={running || !status?.has_key}
                      onclick={() => start(t.id)}
                      title={status?.has_key
                        ? "Vorher siehst du, was gesendet wird und was es kostet."
                        : "Erst den Key von OpenRouter hinterlegen"}>Analysieren</Button
                    >
                  </li>
                {/each}
              </ul>
            {:else}
              <p class="sub pad">Alle fertigen Hold'em-Turniere sind besprochen. Neue findest du nach dem nächsten Turnier hier.</p>
            {/if}
          </Card>

          {#if list.length}
            <nav class="reports" aria-labelledby="reports-title">
              <h2 id="reports-title" class="rail-title">Deine Berichte <span class="count num">{list.length}</span></h2>
              <ul>
                {#each list as r (r.tournament_id)}
                  <li>
                    <Card
                      pad="sm"
                      selected={r.tournament_id === selected && runFor === null && !setupOpen}
                      aria-current={r.tournament_id === selected ? "true" : undefined}
                      onclick={() => {
                        selected = r.tournament_id;
                        runFor = null;
                        setupOpen = false;
                        focus = null;
                        returnTo = null;
                      }}
                    >
                      <span class="item">
                      <span class="name">{titleOf(r.tournament_id, r)}</span>
                      <span class="sub">{r.started_at ? dateTime(r.started_at) : "–"}</span>
                      <span class="score">
                        {@render bar(r.spots, r.mistakes)}
                        <span class="sub">
                          {#if r.mistakes}<b class="warn num">{r.mistakes}</b> von <span class="num">{r.spots}</span> zum Üben
                          {:else}alle <span class="num">{r.spots}</span> gut gespielt{/if}
                        </span>
                      </span>
                      </span>
                    </Card>
                  </li>
                {/each}
              </ul>
            </nav>
          {/if}

          {#if status?.has_key}
            <Card
              pad="sm"
              selected={setupOpen}
              aria-expanded={setupOpen}
              onclick={() => (setupOpen = !setupOpen)}
              title="Modell, Gründlichkeit und Key ändern"
            >
              <span class="setup">
              <span class="dot" class:busy={running} aria-hidden="true"></span>
              <span class="what">
                <span class="name">{running ? "Coach analysiert" : "Coach eingerichtet"}</span>
                <span class="sub"><span class="num">{shortModel(status.model)}</span> · {effortName(status.effort)}</span>
              </span>
              <svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true"
                ><path d="M4 7h10M18 7h2M4 17h4M12 17h8M14 4.5v5M8 14.5v5" /></svg
              >
              </span>
            </Card>
          {/if}
        </aside>

        <div class="main" bind:this={main}>
          {#if status && !status.has_key}
            <Card pad="lg" tone="raised" aria-labelledby="key-title">
              <div class="onboard-head">
                <h2 id="key-title" class="big-title">Für den Coach brauchst du einen API-Key von OpenRouter</h2>
                <p>
                  Über OpenRouter zahlst du die Analysen direkt beim Anbieter, meist wenige Cent je Turnier. Ringside
                  verdient daran nichts und schickt nie etwas ohne deine Bestätigung.
                </p>
              </div>
              <ol class="steps">
                <li>
                  <span class="n num">1</span>
                  <div>
                    <b>Key erstellen</b>
                    <p>
                      Konto auf <a href="https://openrouter.ai/keys" target="_blank" rel="noreferrer" onclick={open}
                        >openrouter.ai/keys</a
                      > anlegen, ein paar Dollar Guthaben laden und einen Key erzeugen.
                    </p>
                  </div>
                </li>
                <li>
                  <span class="n num">2</span>
                  <div>
                    <b>Hier einfügen</b>
                    <p>Ringside prüft ihn sofort und legt ihn im Schlüsselspeicher von Windows ab.</p>
                    <div class="form"><KeyForm onSaved={loadStatus} /></div>
                  </div>
                </li>
                <li>
                  <span class="n num">3</span>
                  <div>
                    <b>Turnier besprechen</b>
                    <p>Unter „Turnier besprechen“ eins wählen. Du siehst die Kosten, bevor etwas gesendet wird.</p>
                  </div>
                </li>
              </ol>
            </Card>
          {/if}

          {#if setupOpen}
            <div in:rise out:leave>
            <CoachSettings
              onClose={() => (setupOpen = false)}
              onChange={(s) => {
                status = s;
                if (!s.has_key) setupOpen = false;
              }}
            />
            </div>
          {/if}

          {#if runFor !== null}
            {#key runFor}
              <RunPanel
                tournamentId={runFor}
                title={titleOf(runFor)}
                onDone={done}
                onClose={() => {
                  runFor = null;
                  running = false;
                }}
                onSettings={() => (setupOpen = true)}
                onRunning={(r) => (running = r)}
              />
            {/key}
          {:else if failed}
            {@render loadFailed(failed)}
          {:else if list.length === 0 && status?.has_key}
            <Card pad="lg" aria-labelledby="intro-title">
              <h2 id="intro-title" class="big-title">Lass dein letztes Turnier besprechen</h2>
              <p class="lead">
                Der Coach sucht die Hände heraus, in denen du eine echte Entscheidung hattest, und sagt dir, was gut war und
                was du üben solltest. Jede Zahl darin rechnet Ringside selbst.
              </p>
              <ol class="flow">
                <li><b>Turnier wählen</b><span>Unter „Turnier besprechen“ stehen deine letzten fertigen Turniere.</span></li>
                <li><b>Kosten sehen</b><span>Ringside schätzt vorher, was die Analyse kostet, meist wenige Cent.</span></li>
                <li><b>Bestätigen</b><span>Erst dann gehen deine Hände ohne Namen an das Sprachmodell.</span></li>
                <li><b>Bericht lesen und üben</b><span>Jede Entscheidung mit Urteil, Begründung und passender Übung.</span></li>
              </ol>
              {#if suggestions[0]}
                <div class="cta">
                  <Button variant="primary" size="lg" onclick={() => start(suggestions[0].id)}>
                    {titleOf(suggestions[0].id)} analysieren
                  </Button>
                  <span class="sub">vom {dateTime(suggestions[0].started_at)}, {suggestions[0].hands} Hände</span>
                </div>
              {/if}
            </Card>
          {:else if report && report.tournament_id === selected}
            {@const reportTitle = titleOf(report.tournament_id, list.find((r) => r.tournament_id === report?.tournament_id))}
            {@const spot = focus ? report.spots[focus.n - 1] : null}
            {#if focus && spot && focus.mode === "drill"}
              <div in:appear>
              <SpotDrill
                {spot}
                n={focus.n}
                total={report.spots.length}
                title={reportTitle}
                practice={practice(spot.concept)}
                onBack={back}
                {onTraining}
                onRead={() => focus && (focus = { mode: "read", n: focus.n })}
                onAnswer={(answer, right) => {
                  if (report && focus) practised[`${report.tournament_id}:${focus.n}`] = { answer, right };
                }}
              />
              </div>
            {:else if focus && spot}
              <div in:appear>
              <SpotRead
                {spot}
                n={focus.n}
                total={report.spots.length}
                title={reportTitle}
                practice={practice(spot.concept)}
                onBack={back}
                onDrill={() => focus && (focus = { mode: "drill", n: focus.n })}
              />
              </div>
            {:else}
              <div in:appear>
              <ReportView
                {report}
                title={reportTitle}
                {practice}
                {returnTo}
                practised={(n) => (report ? practised[`${report.tournament_id}:${n}`] : undefined)}
                onHand={(id) => report && onHand(report.tournament_id, id)}
                {onTraining}
                canAnalyze={!!status?.has_key && !running}
                onReanalyze={() => report && start(report.tournament_id)}
                onDelete={() => report && remove(report.tournament_id)}
                onDrill={(n) => openSpot("drill", n)}
                onRead={(n) => openSpot("read", n)}
              />
              </div>
            {/if}
          {/if}
        </div>
      </div>
    {:else if failed}
      {@render loadFailed(failed)}
    {/if}
  </div>
</ClientLock>

<style>
  .wide {
    max-width: 1880px;
    padding-top: 20px;
  }
  .layout {
    display: grid;
    grid-template-columns: 296px minmax(0, 1fr);
    gap: 16px;
    align-items: start;
  }
  .rail {
    position: sticky;
    top: 16px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-width: 0;
  }
  .rail-title {
    margin: 0;
    padding: var(--sp-1) var(--sp-2) var(--sp-2);
    font-size: var(--fs-md);
    font-weight: var(--fw-semibold);
  }
  .rail ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .name {
    font-size: var(--fs-base);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sub {
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .count {
    margin-left: var(--sp-1);
    font-weight: var(--fw-regular);
    color: var(--text-2);
  }
  .nowrap {
    white-space: nowrap;
  }
  .pad {
    margin: 0;
    padding: 0 var(--sp-2) var(--sp-2);
    line-height: 1.5;
  }
  .what {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }

  .start {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding: 7px var(--sp-2);
  }
  .start .what {
    flex: 1;
  }

  .reports ul {
    gap: var(--sp-2);
    max-height: max(280px, calc(100vh - 420px));
    overflow-y: auto;
    overscroll-behavior: contain;
    mask-image: linear-gradient(to bottom, var(--text) calc(100% - 36px), transparent);
    padding-bottom: 24px;
  }
  .item {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .score {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 5px;
  }
  .score .sub {
    color: var(--text-2);
  }
  .warn {
    color: var(--amber);
    font-weight: var(--fw-semibold);
  }
  .mini {
    display: flex;
    gap: 2px;
    width: 54px;
    height: 6px;
    flex-shrink: 0;
  }
  .mini i {
    flex-basis: 0;
    border-radius: 2px;
  }
  .mini .bad {
    background: var(--amber);
  }
  .mini .good {
    background: color-mix(in srgb, var(--pos) 70%, transparent);
  }

  .setup {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
  }
  .setup .what {
    flex: 1;
  }
  .setup svg {
    fill: none;
    stroke: var(--text-2);
    stroke-width: 1.7;
    stroke-linecap: round;
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--pos);
    flex-shrink: 0;
  }
  .dot.busy {
    background: var(--amber);
    animation: breathe var(--dur-loop) var(--ease-standard) infinite;
  }
  @keyframes breathe {
    50% {
      opacity: 0.35;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .dot.busy {
      animation: none;
    }
  }

  .main {
    scroll-margin-top: var(--sp-4);
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-width: 0;
  }

  .onboard-head {
    max-width: 720px;
  }
  .big-title {
    margin: 0 0 6px;
    font-family: var(--font-num);
    font-size: var(--fs-2xl);
    font-weight: var(--fw-semibold);
  }
  .onboard-head p,
  .lead {
    margin: 0;
    color: var(--text-2);
    line-height: 1.55;
    max-width: 720px;
  }
  .steps {
    list-style: none;
    margin: 20px 0 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 12px;
  }
  .steps li {
    display: flex;
    gap: 12px;
    padding: 14px 16px 16px;
    border-radius: var(--r-sm);
    background: var(--bg);
    border: 1px solid var(--line);
  }
  .steps li > div {
    min-width: 0;
  }
  .steps b {
    display: block;
    margin-bottom: 3px;
  }
  .steps p {
    margin: 0;
    font-size: var(--fs-md);
    line-height: 1.5;
    color: var(--text-2);
  }
  .steps a {
    color: var(--text);
    text-underline-offset: 2px;
  }
  .n {
    display: grid;
    place-items: center;
    flex-shrink: 0;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    border: 1px solid var(--line-2);
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .form {
    margin-top: 12px;
  }

  .flow {
    list-style: none;
    counter-reset: flow;
    margin: 22px 0 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 10px;
  }
  .flow li {
    counter-increment: flow;
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 14px 16px 16px;
    border-top: 2px solid var(--line-2);
    background: var(--bg);
    border-radius: 0 0 var(--r-sm) var(--r-sm);
  }
  .flow li:first-child {
    border-top-color: var(--text);
  }
  .flow li::before {
    content: counter(flow);
    font-family: var(--font-num);
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .flow span {
    font-size: var(--fs-md);
    color: var(--text-2);
    line-height: 1.5;
  }
  .cta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 14px;
    margin-top: 22px;
  }


  /* Practising needs the whole width for the table; the report row is one click away. */
  @container (max-width: 1640px) {
    .drilling {
      grid-template-columns: minmax(0, 1fr);
    }
    .drilling .rail {
      display: none;
    }
  }
  @container (max-width: 1040px) {
    .steps {
      grid-template-columns: minmax(0, 1fr);
    }
    .flow {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }
  @container (max-width: 860px) {
    .layout {
      grid-template-columns: minmax(0, 1fr);
    }
    /* Report row first, then the open report or task, then starting a new one and the setup. */
    .rail {
      display: contents;
    }
    .reports {
      order: -2;
    }
    .main {
      order: -1;
    }
    .reports ul {
      flex-direction: row;
      max-height: none;
      overflow-x: auto;
      overflow-y: hidden;
      mask-image: linear-gradient(to right, var(--text) calc(100% - 36px), transparent);
      padding: 0 var(--sp-8) var(--sp-1) 0;
    }
    .reports li {
      flex: 0 0 260px;
      min-width: 0;
      max-width: 260px;
    }
    /* While a task runs, it comes before the report row too. */
    .busy .main {
      order: -3;
    }
  }
</style>
