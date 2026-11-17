<script lang="ts">
  import { onMount } from "svelte";
  import ClientLock from "../app/ClientLock.svelte";
  import { dateTime } from "../app/format";
  import type { TournamentRow } from "../types";
  import { coachApi, cost, errorText } from "./api";
  import ReportView from "./ReportView.svelte";
  import RunPanel from "./RunPanel.svelte";
  import type { CoachReport, CoachStatus, ReportListItem } from "./types";

  let {
    analyze,
    tournaments,
    onAnalyzeHandled,
    onHand,
    onTraining,
    onSettings,
  }: {
    /** A tournament to analyse, set from the tournaments page. */
    analyze: number | null;
    tournaments: TournamentRow[];
    onAnalyzeHandled: () => void;
    onHand: (tournament: number, hand: number) => void;
    onTraining: () => void;
    onSettings: () => void;
  } = $props();

  let status = $state<CoachStatus | null>(null);
  let list = $state<ReportListItem[] | null>(null);
  let selected = $state<number | null>(null);
  let report = $state<CoachReport | null>(null);
  let runFor = $state<number | null>(null);
  let failed = $state<string | null>(null);

  const titleOf = (id: number, item?: { name: string | null; game: string }) => {
    const t = tournaments.find((x) => x.id === id);
    return item?.name || t?.name || item?.game || t?.game || `Turnier ${id}`;
  };

  async function loadList(select: number | null = selected) {
    try {
      list = await coachApi.reports(tournaments);
      failed = null;
      selected = select !== null && list.some((r) => r.tournament_id === select) ? select : (list[0]?.tournament_id ?? null);
    } catch (e) {
      failed = errorText(e);
    }
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
    onAnalyzeHandled();
  });

  onMount(() => {
    coachApi.status().then((s) => (status = s));
    loadList();
  });

  function done(r: CoachReport) {
    runFor = null;
    report = r;
    loadList(r.tournament_id);
  }

  async function remove(id: number) {
    await coachApi.deleteReport(id);
    selected = null;
    loadList(null);
  }
</script>

<ClientLock feature="Der Coach">
  <div class="page">
    {#if status && !status.has_key}
      <section class="panel banner">
        <p>
          Für den Coach brauchst du einen API-Key von OpenRouter. Damit zahlst du die Analysen direkt beim Anbieter, meist
          wenige Cent je Turnier.
        </p>
        <button class="btn primary" onclick={onSettings}>Key in den Einstellungen hinterlegen</button>
      </section>
    {/if}

    {#if runFor !== null}
      {#key runFor}
        <RunPanel
          tournamentId={runFor}
          title={titleOf(runFor)}
          onDone={done}
          onClose={() => (runFor = null)}
          {onSettings}
        />
      {/key}
    {/if}

    {#if failed}
      <div class="panel empty-state">{failed}</div>
    {:else if list && list.length === 0}
      <section class="panel intro">
        <h2>So arbeitet der Coach</h2>
        <ol>
          <li>Wähl auf der Seite <b>Turniere</b> ein abgeschlossenes Turnier und klick auf <b>Analysieren</b>.</li>
          <li>
            Ringside sucht die Hände heraus, in denen du eine echte Entscheidung hattest, rechnet Push/Fold- und
            All-in-Spots vor und zeigt dir vorher, was die Analyse ungefähr kostet.
          </li>
          <li>
            Erst nach deiner Bestätigung gehen die Hände ohne Namen an das Sprachmodell. Es rechnet nichts selbst, sondern
            fragt jede Zahl bei Ringside ab.
          </li>
          <li>Der Bericht bleibt hier gespeichert und zeigt dir, was du üben solltest.</li>
        </ol>
      </section>
    {:else if list}
      <div class="layout">
        <nav class="panel reports" aria-label="Gespeicherte Berichte">
          <div class="panel-head"><h2 class="panel-title">Berichte</h2></div>
          <ul>
            {#each list as r (r.tournament_id)}
              <li>
                <button class:on={r.tournament_id === selected} onclick={() => (selected = r.tournament_id)}>
                  <span class="name">{titleOf(r.tournament_id, r)}</span>
                  <span class="sub">
                    {r.started_at ? dateTime(r.started_at) : "–"} · {r.spots} Spots
                    {#if r.mistakes}· <span class="warn">{r.mistakes} zum Üben</span>{/if}
                  </span>
                  <span class="sub num">{cost(r.cost_usd)}</span>
                </button>
              </li>
            {/each}
          </ul>
        </nav>

        <div class="main">
          {#if report && report.tournament_id === selected}
            <ReportView
              {report}
              title={titleOf(report.tournament_id, list.find((r) => r.tournament_id === report?.tournament_id))}
              busy={runFor !== null}
              onHand={(id) => report && onHand(report.tournament_id, id)}
              {onTraining}
              onReanalyze={() => (runFor = report?.tournament_id ?? null)}
              onDelete={() => report && remove(report.tournament_id)}
            />
          {/if}
        </div>
      </div>
    {/if}
  </div>
</ClientLock>

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding-top: 20px;
  }
  .banner {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 12px 20px;
    padding: 14px 18px;
  }
  .banner p {
    margin: 0;
    max-width: 640px;
    line-height: 1.5;
  }
  .intro {
    max-width: 760px;
    padding: 18px 22px 20px;
  }
  .intro h2 {
    margin: 0 0 10px;
    font-size: 16px;
  }
  .intro ol {
    margin: 0;
    padding-left: 20px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    color: var(--text-2);
    line-height: 1.5;
  }
  .intro b {
    color: var(--text);
  }
  .layout {
    display: grid;
    grid-template-columns: 250px minmax(0, 1fr);
    gap: 14px;
    align-items: start;
  }
  .reports {
    position: sticky;
    top: 0;
    overflow: hidden;
  }
  .reports ul {
    list-style: none;
    margin: 8px 0 0;
    padding: 0 6px 6px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .reports button {
    display: flex;
    flex-direction: column;
    gap: 2px;
    width: 100%;
    padding: 8px 10px;
    border: 0;
    border-radius: var(--r-sm);
    background: transparent;
    text-align: left;
    cursor: pointer;
  }
  .reports button:hover {
    background: var(--surface-2);
  }
  .reports button.on {
    background: var(--surface-3);
  }
  .name {
    font-size: 13px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sub {
    font-size: 12px;
    color: var(--text-3);
  }
  .warn {
    color: var(--amber);
  }
  .main {
    min-width: 0;
  }

  @container (max-width: 860px) {
    .layout {
      grid-template-columns: minmax(0, 1fr);
    }
    .reports {
      position: static;
    }
    .reports ul {
      flex-direction: row;
      overflow-x: auto;
    }
    .reports li {
      flex: 0 0 210px;
    }
  }
</style>
