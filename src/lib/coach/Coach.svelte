<script lang="ts">
  import { onMount } from "svelte";
  import { api, inTauri } from "../api";
  import ClientLock from "../app/ClientLock.svelte";
  import type { LearnTarget } from "../app/Leaks.svelte";
  import { dateTime, dayShort } from "../app/format";
  import { nextLevel, trainableKind } from "../learn/catalog";
  import { ARTICLE_BY_ID } from "../learn/wissen/articles";
  import type { LearnProgress, TournamentRow } from "../types";
  import { coachApi, effortName, errorText, preview } from "./api";
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
  let practised = $state<Record<string, { answer: Choice; right: boolean | null }>>({});

  function openSpot(mode: "drill" | "read", n: number) {
    focus = { mode, n };
    returnTo = null;
    window.scrollTo?.({ top: 0 });
    document.querySelector(".layout")?.scrollIntoView({ block: "start" });
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

<ClientLock feature="Der Coach">
  <div class="page">
    {#if list}
      <div class="layout">
        <aside class="rail">
          <section class="panel start" aria-labelledby="start-title">
            <h2 id="start-title" class="panel-title">Turnier besprechen</h2>
            {#if status && !status.has_key}
              <p class="sub pad">Erst den Key von OpenRouter hinterlegen, dann kannst du hier ein Turnier besprechen lassen.</p>
            {/if}
            {#if suggestions.length}
              <ul>
                {#each suggestions as t (t.id)}
                  <li>
                    <span class="what">
                      <span class="name" title={titleOf(t.id)}>{titleOf(t.id)}</span>
                      <span class="sub">{dayShort(t.started_at)} · <span class="num">{t.hands}</span> Hände</span>
                    </span>
                    <button
                      class="btn go"
                      disabled={running || !status?.has_key}
                      onclick={() => start(t.id)}
                      title={status?.has_key
                        ? "Vorher siehst du, was gesendet wird und was es kostet."
                        : "Erst den Key von OpenRouter hinterlegen"}>Analysieren</button
                    >
                  </li>
                {/each}
              </ul>
            {:else}
              <p class="sub pad">Alle fertigen Hold'em-Turniere sind besprochen. Neue findest du nach dem nächsten Turnier hier.</p>
            {/if}
          </section>

          {#if list.length}
            <nav class="panel reports" aria-labelledby="reports-title">
              <h2 id="reports-title" class="panel-title">Deine Berichte</h2>
              <ul>
                {#each list as r (r.tournament_id)}
                  <li>
                    <button
                      class="item"
                      class:on={r.tournament_id === selected && runFor === null}
                      aria-current={r.tournament_id === selected ? "true" : undefined}
                      onclick={() => {
                        selected = r.tournament_id;
                        runFor = null;
                        setupOpen = false;
                        focus = null;
                        returnTo = null;
                      }}
                    >
                      <span class="name">{titleOf(r.tournament_id, r)}</span>
                      <span class="sub">{r.started_at ? dateTime(r.started_at) : "–"}</span>
                      <span class="score">
                        {@render bar(r.spots, r.mistakes)}
                        <span class="sub">
                          {#if r.mistakes}<b class="warn num">{r.mistakes}</b> von <span class="num">{r.spots}</span> zum Üben
                          {:else}alle <span class="num">{r.spots}</span> gut gespielt{/if}
                        </span>
                      </span>
                    </button>
                  </li>
                {/each}
              </ul>
            </nav>
          {/if}

          {#if status?.has_key}
            <button
              class="panel setup-card"
              class:on={setupOpen}
              aria-expanded={setupOpen}
              onclick={() => (setupOpen = !setupOpen)}
              title="Modell, Gründlichkeit und Key ändern"
            >
              <span class="dot" class:busy={running} aria-hidden="true"></span>
              <span class="what">
                <span class="name">{running ? "Coach analysiert" : "Coach eingerichtet"}</span>
                <span class="sub"><span class="num">{shortModel(status.model)}</span> · {effortName(status.effort)}</span>
              </span>
              <svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true"
                ><path d="M4 7h10M18 7h2M4 17h4M12 17h8M14 4.5v5M8 14.5v5" /></svg
              >
            </button>
          {/if}
        </aside>

        <div class="main">
          {#if status && !status.has_key}
            <section class="panel onboard" aria-labelledby="key-title">
              <div class="onboard-head">
                <h2 id="key-title">Für den Coach brauchst du einen API-Key von OpenRouter</h2>
                <p>
                  Über OpenRouter zahlst du die Analysen direkt beim Anbieter, meist wenige Cent je Turnier. Ringside
                  verdient daran nichts und schickt nie etwas ohne deine Bestätigung.
                </p>
              </div>
              <ol class="steps">
                <li>
                  <span class="n num">1</span>
                  <div>
                    <b>Key bei OpenRouter erstellen</b>
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
            </section>
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
            <div class="panel failed" role="alert">
              <b>Die Berichte lassen sich gerade nicht laden.</b>
              <span>{failed}</span>
              <button class="btn" onclick={() => loadList()}>Erneut laden</button>
            </div>
          {:else if list.length === 0 && status?.has_key}
            <section class="panel intro" aria-labelledby="intro-title">
              <h2 id="intro-title">Lass dein letztes Turnier besprechen</h2>
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
                  <button class="btn primary" onclick={() => start(suggestions[0].id)}>
                    {titleOf(suggestions[0].id)} analysieren
                  </button>
                  <span class="sub">vom {dateTime(suggestions[0].started_at)}, {suggestions[0].hands} Hände</span>
                </div>
              {/if}
            </section>
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
      <div class="panel failed" role="alert">
        <b>Die Berichte lassen sich gerade nicht laden.</b>
        <span>{failed}</span>
        <button class="btn" onclick={() => loadList()}>Erneut laden</button>
      </div>
    {/if}
  </div>
</ClientLock>

<style>
  .page {
    max-width: 1880px;
    padding-top: 20px;
  }
  .layout {
    display: grid;
    grid-template-columns: 264px minmax(0, 1fr);
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
  .rail .panel-title {
    padding: 14px 16px 6px;
  }
  .rail ul {
    list-style: none;
    margin: 0;
    padding: 0 6px 6px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .name {
    font-size: 13.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sub {
    font-size: 12.5px;
    color: var(--text-2);
  }
  .pad {
    margin: 0;
    padding: 0 16px 14px;
    line-height: 1.5;
  }
  .what {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }

  .start li {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 10px;
    border-radius: var(--r-sm);
  }
  .start li {
    transition:
      background var(--dur-fast, 120ms) var(--ease-standard, cubic-bezier(0.2, 0, 0, 1));
  }
  .start li:hover {
    background: var(--surface-2);
  }
  .start .what {
    flex: 1;
  }
  .go {
    height: 28px;
    padding: 0 10px;
    font-size: 12.5px;
  }

  .reports ul {
    max-height: min(52vh, 560px);
    overflow-y: auto;
    overscroll-behavior: contain;
    mask-image: linear-gradient(to bottom, #000 calc(100% - 36px), transparent);
    padding-bottom: 24px;
  }
  .item {
    display: flex;
    flex-direction: column;
    gap: 2px;
    width: 100%;
    padding: 9px 10px 10px;
    border: 0;
    border-radius: var(--r-sm);
    background: transparent;
    text-align: left;
    cursor: pointer;
  }
  .item {
    transition:
      background var(--dur-fast, 120ms) var(--ease-standard, cubic-bezier(0.2, 0, 0, 1)),
      box-shadow var(--dur-fast, 120ms) var(--ease-standard, cubic-bezier(0.2, 0, 0, 1));
  }
  .item:hover {
    background: var(--surface-2);
  }
  .item.on {
    background: var(--surface-3);
    box-shadow: inset 2px 0 0 var(--text);
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
    font-weight: 600;
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

  .setup-card {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 11px 14px;
    text-align: left;
    cursor: pointer;
    color: var(--text);
  }
  .setup-card {
    transition:
      background var(--dur-fast, 120ms) var(--ease-standard, cubic-bezier(0.2, 0, 0, 1)),
      border-color var(--dur-fast, 120ms) var(--ease-standard, cubic-bezier(0.2, 0, 0, 1));
  }
  .setup-card:hover,
  .setup-card.on {
    background: var(--surface-2);
    border-color: var(--line-2);
  }
  .setup-card .what {
    flex: 1;
  }
  .setup-card svg {
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
    animation: breathe 1.6s var(--ease-standard, cubic-bezier(0.2, 0, 0, 1)) infinite;
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
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-width: 0;
  }

  .onboard {
    padding: 22px 24px 24px;
    border-color: var(--line-2);
  }
  .onboard-head {
    max-width: 720px;
  }
  .onboard h2,
  .intro h2 {
    margin: 0 0 6px;
    font-family: var(--font-num);
    font-size: 20px;
    font-weight: 600;
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
    grid-template-columns: minmax(0, 1fr) minmax(0, 1.6fr) minmax(0, 1fr);
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
    font-size: 13px;
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
    font-size: 13px;
    color: var(--text-2);
  }
  .form {
    margin-top: 12px;
  }

  .intro {
    padding: 24px 26px 26px;
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
    font-size: 13px;
    color: var(--text-2);
  }
  .flow span {
    font-size: 13px;
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
  .cta .btn {
    height: 36px;
    padding: 0 16px;
  }

  .failed {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 14px;
    padding: 16px 18px;
    border-color: rgba(229, 72, 59, 0.35);
  }
  .failed span {
    flex: 1 1 260px;
    color: var(--text-2);
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
    .main {
      order: -1;
    }
    .rail {
      position: static;
      display: grid;
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
    .reports {
      grid-row: span 2;
    }
  }
</style>
