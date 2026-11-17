<script lang="ts">
  import { cost, tokens } from "./api";
  import SpotCard from "./SpotCard.svelte";
  import type { CoachReport, Verdict } from "./types";

  let {
    report,
    title,
    busy,
    onHand,
    onTraining,
    onReanalyze,
    onDelete,
  }: {
    report: CoachReport;
    title: string;
    busy: boolean;
    onHand: (id: number) => void;
    onTraining: () => void;
    onReanalyze: () => void;
    onDelete: () => void;
  } = $props();

  const ORDER: [Verdict, string][] = [
    ["fehler", "Fehler"],
    ["fraglich", "Fraglich"],
    ["ok", "Okay"],
    ["stark", "Stark"],
  ];
  const counts = $derived(ORDER.map(([v, label]) => ({ v, label, n: report.spots.filter((s) => s.verdict === v).length })));
  const created = $derived(
    new Date(report.meta.created_at * 1000).toLocaleString("de-DE", { dateStyle: "medium", timeStyle: "short" }),
  );
  const minutes = $derived(Math.max(1, Math.round(report.meta.seconds / 60)));
  let confirmDelete = $state(false);
</script>

<div class="report">
  <section class="panel head">
    <div class="titles">
      <h2>{title}</h2>
      <p class="meta">
        Analysiert am {created} · <span class="num">{report.meta.model}</span> ·
        <span class="num">{tokens(report.meta.prompt_tokens + report.meta.completion_tokens)}</span> Tokens ·
        <span class="num">{cost(report.meta.cost_usd)}</span> · {minutes} Min.
      </p>
      <p class="meta">
        {report.meta.hands_sent} von {report.meta.hands_total} Händen gesendet · {report.meta.tool_calls} Rechnungen von Ringside
        {#if report.meta.downgraded}
          · <span title="Das Modell hat diese Zahlen als berechnet ausgegeben, kein Werkzeug hat sie geliefert.">
            {report.meta.downgraded} angebliche Rechnungen als Einschätzung markiert</span
          >
        {/if}
      </p>
    </div>
    <div class="buttons">
      <button class="btn" onclick={onReanalyze} disabled={busy}>Neu analysieren</button>
      {#if confirmDelete}
        <button class="btn danger" onclick={onDelete}>Wirklich löschen</button>
        <button class="btn ghost" onclick={() => (confirmDelete = false)}>Behalten</button>
      {:else}
        <button class="btn ghost" onclick={() => (confirmDelete = true)} disabled={busy}>Löschen</button>
      {/if}
    </div>
  </section>

  <div class="overview">
    <section class="panel summary">
      <h3>Gesamteindruck</h3>
      <p>{report.summary}</p>
      <div class="tally" aria-label="Urteile">
        {#each counts as c (c.v)}
          <span class="t v-{c.v}" class:zero={c.n === 0}><b class="num">{c.n}</b> {c.label}</span>
        {/each}
      </div>
    </section>
    <section class="panel focus">
      <h3>Schwerpunkte</h3>
      <ol>
        {#each report.focus as f, i (i)}<li>{f}</li>{/each}
      </ol>
      <button class="btn primary" onclick={onTraining}>Üben</button>
    </section>
  </div>

  <div class="spots">
    {#each report.spots as spot, i (spot.hand_id + ":" + i)}
      <SpotCard {spot} index={i + 1} {onHand} {onTraining} />
    {/each}
  </div>

  <p class="foot faint">
    Zahlen mit „berechnet“ stammen aus Ringsides Rechenkern (Equity, Pot Odds, Push/Fold-Nash, ICM, Outs). Alles mit
    „Einschätzung“ ist die Meinung des Sprachmodells, etwa über die Ranges deiner Gegner.
  </p>
</div>

<style>
  .report {
    display: flex;
    flex-direction: column;
    gap: 14px;
    min-width: 0;
  }
  .head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 12px 20px;
    padding: 16px 18px;
  }
  .titles {
    min-width: 0;
  }
  h2 {
    margin: 0 0 4px;
    font-family: var(--font-num);
    font-size: 19px;
    font-weight: 600;
  }
  .meta {
    margin: 0;
    font-size: 12.5px;
    color: var(--text-2);
    line-height: 1.6;
  }
  .buttons {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  .overview {
    display: grid;
    grid-template-columns: minmax(0, 1.6fr) minmax(0, 1fr);
    gap: 14px;
  }
  .summary,
  .focus {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 16px 18px;
  }
  h3 {
    margin: 0;
    font-size: 13px;
    font-weight: 600;
    color: var(--text-2);
  }
  .summary p {
    margin: 0;
    font-size: 15px;
    line-height: 1.55;
    text-wrap: pretty;
  }
  .tally {
    display: flex;
    flex-wrap: wrap;
    gap: 14px;
    margin-top: auto;
    padding-top: 4px;
    font-size: 13px;
    color: var(--text-2);
  }
  .t b {
    font-size: 17px;
    margin-right: 2px;
  }
  .v-fehler b {
    color: var(--neg);
  }
  .v-fraglich b {
    color: var(--amber);
  }
  .v-stark b {
    color: var(--pos);
  }
  .t.zero {
    opacity: 0.5;
  }
  .t.zero b {
    color: var(--text-3);
  }
  ol {
    margin: 0;
    padding-left: 20px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    line-height: 1.45;
  }
  .focus .btn {
    align-self: flex-start;
    margin-top: auto;
  }
  .spots {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .foot {
    margin: 4px 2px 0;
    font-size: 12.5px;
    line-height: 1.5;
  }

  @container (max-width: 760px) {
    .overview {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
