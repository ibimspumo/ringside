<script lang="ts">
  import { api } from "../api";
  import { buyInText, cash, tournamentName } from "../stats";
  import type { Filter, TournamentRow, Tournaments } from "../types";
  import { dateTime, int, sign, signed } from "./format";
  import ResultsChart, { type Series } from "./ResultsChart.svelte";

  let {
    filter,
    refresh,
    selected,
    onHands,
    onAnalyze,
  }: {
    filter: Filter;
    refresh: number;
    selected: number | null;
    onHands: (tournament: number) => void;
    onAnalyze: (tournament: number) => void;
  } = $props();

  /** Why the coach cannot analyse this tournament yet, or null. */
  function notAnalyzable(r: TournamentRow): string | null {
    if (r.prize === null) return "Erst wenn das Turnier für dich vorbei ist";
    if (r.game && !r.game.includes("Hold")) return "Der Coach analysiert bisher nur Hold'em";
    if (r.hands === 0) return "Keine Hände von diesem Turnier gespeichert";
    return null;
  }

  type Point = { n: number; played_at: string; profit: number; bounties: number };
  type Col = "date" | "game" | "buy_in" | "entrants" | "place" | "prize" | "bounties" | "profit" | "hands";
  const COLS: [Col, string][] = [
    ["date", "Datum"],
    ["game", "Turnier"],
    ["buy_in", "Buy-in"],
    ["entrants", "Teilnehmer"],
    ["place", "Platz"],
    ["prize", "Preis"],
    ["bounties", "Bounties"],
    ["profit", "Ergebnis"],
    ["hands", "Hände"],
  ];
  const NAMES: Record<string, string> = { PLAY: "Spielgeld", FPP: "FPP", SC: "StarsCoin" };

  let data = $state<Tournaments | null>(null);
  let failed = $state(false);
  let picked = $state<string | null>(null);
  let sortBy = $state<Col>("date");
  let asc = $state(false);

  $effect(() => {
    void refresh;
    const f = { ...filter };
    let live = true;
    api
      .tournaments(f)
      .then((d) => {
        if (!live) return;
        data = d;
        failed = false;
      })
      .catch(() => live && (failed = true));
    return () => (live = false);
  });

  /** Currencies with money to show; tournaments of unknown currency only add to the count. */
  const currencies = $derived(data?.totals.filter((t) => t.currency) ?? []);
  const unknown = $derived(data?.totals.find((t) => !t.currency)?.tournaments ?? 0);
  const totals = $derived(currencies.find((t) => t.currency === picked) ?? currencies[0] ?? null);
  const cur = $derived(totals?.currency ?? null);

  const signedCash = (amount: number, currency: string | null) => (amount > 0 ? "+" : "") + cash(amount, currency);
  const percent = (v: number | null, digits = 1) => (v === null ? "–" : `${signed(v, digits)} %`);

  const points = $derived.by<Point[]>(() => {
    if (!data || !cur) return [];
    let profit = 0;
    let bounties = 0;
    return data.rows
      .filter((r) => r.currency === cur && r.profit !== null)
      .sort((a, b) => a.started_at.localeCompare(b.started_at) || a.id - b.id)
      .map((r, i) => {
        profit += r.profit ?? 0;
        bounties += r.bounties;
        return { n: i + 1, played_at: r.started_at, profit: profit / 100, bounties: bounties / 100 };
      });
  });
  const series = $derived<Series<Point>[]>([
    { key: "profit", label: "Gewinn", color: "var(--text)", width: 2 },
    ...(totals?.bounties ? [{ key: "bounties" as const, label: "Davon Bounties", color: "var(--sd)", width: 1.5 }] : []),
  ]);

  function key(r: TournamentRow, col: Col): number | string | null {
    switch (col) {
      case "date":
        return r.started_at;
      case "game":
        return tournamentName(r);
      case "buy_in":
        return r.cost;
      case "entrants":
        return r.entrants;
      case "place":
        return r.place;
      case "prize":
        return r.prize;
      case "bounties":
        return r.bounties;
      case "profit":
        return r.profit;
      case "hands":
        return r.hands;
    }
  }

  const rows = $derived.by(() => {
    const dir = asc ? 1 : -1;
    return [...(data?.rows ?? [])].sort((a, b) => {
      const x = key(a, sortBy);
      const y = key(b, sortBy);
      if (x === null && y === null) return 0;
      if (x === null) return 1;
      if (y === null) return -1;
      return dir * (typeof x === "string" ? x.localeCompare(y as string, "de") : x - (y as number));
    });
  });

  function sortOn(col: Col) {
    if (sortBy === col) asc = !asc;
    else {
      sortBy = col;
      asc = col === "game" || col === "place";
    }
  }

  function buyInTitle(r: TournamentRow): string | undefined {
    const b = r.buy_in;
    if (!b) return undefined;
    const parts = [`${cash(b.prize, b.currency)} Preisgeld`];
    if (b.bounty) parts.push(`${cash(b.bounty, b.currency)} Bounty`);
    parts.push(`${cash(b.fee, b.currency)} Gebühr`);
    const extra = (r.cost ?? 0) - (b.prize + b.bounty + b.fee);
    if (extra > 0) parts.push(`${cash(extra, r.currency)} Re-Entrys`);
    return parts.join(" + ");
  }

  function resultTitle(r: TournamentRow): string {
    if (r.profit !== null) return `Preis + Bounties − Buy-in${r.has_summary ? "" : " (aus deinen Händen, ohne Zusammenfassung)"}`;
    const chips = r.hands ? ` Chips über ${int(r.hands)} Hände: ${signed(r.chips / 100, 0)}.` : "";
    return `Noch kein Ausscheiden in deinen Händen: Das Turnier läuft noch, oder die letzte Hand ist nicht importiert.${chips}`;
  }
</script>

<div class="page">
  {#if failed}
    <div class="panel empty">Die Turniere konnten nicht geladen werden. Starte Ringside neu, falls das so bleibt.</div>
  {:else if data && data.rows.length === 0}
    <div class="panel empty">
      Noch keine Turniere in diesem Zeitraum. Ringside liest dafür deine Turnier-Hände und die Zusammenfassungen aus dem
      Ordner „TournSummary“ neben „HandHistory“.
    </div>
  {:else if data}
    {#if currencies.length > 1}
      <div class="seg cur" role="group" aria-label="Währung">
        {#each currencies as t (t.currency)}
          <button class:on={t.currency === cur} onclick={() => (picked = t.currency)}>{NAMES[t.currency] ?? t.currency}</button>
        {/each}
      </div>
    {/if}

    {#if totals}
      <section class="panel kpis">
        <div class="kpi">
          <span class="label">Turniere</span>
          <strong class="num">{int(totals.tournaments)}</strong>
          <small
            >{totals.finished === totals.tournaments ? "alle mit Ergebnis" : `${int(totals.finished)} mit Ergebnis`}{unknown
              ? ` · ${int(unknown)} ohne Währung`
              : ""}</small
          >
        </div>
        <div class="kpi">
          <span class="label">Buy-ins gesamt</span>
          <strong class="num">{cash(totals.cost, cur)}</strong>
          <small>{totals.finished ? `Ø ${cash(Math.round(totals.cost / totals.finished), cur)}` : "–"}</small>
        </div>
        <div class="kpi">
          <span class="label">Preisgeld</span>
          <strong class="num">{cash(totals.prizes, cur)}</strong>
          <small>{int(totals.cashes)}× im Geld</small>
        </div>
        <div class="kpi">
          <span class="label">Bounties</span>
          <strong class="num">{totals.bounties ? cash(totals.bounties, cur) : "–"}</strong>
          <small>aus Knockouts</small>
        </div>
        <div class="kpi">
          <span class="label">Gewinn</span>
          <strong class="num {sign(totals.profit)}">{signedCash(totals.profit, cur)}</strong>
          <small>nach Buy-ins</small>
        </div>
        <div class="kpi split">
          <div class="pair" title="Return on Investment: Gewinn je eingesetztem Buy-in">
            <span>ROI</span>
            <b class="num {sign(totals.roi)}">{percent(totals.roi)}</b>
          </div>
          <div class="pair" title="In the Money: Anteil der Turniere mit Preisgeld">
            <span>ITM</span>
            <b class="num">{totals.itm === null ? "–" : `${int(totals.itm)} %`}</b>
          </div>
        </div>
      </section>

      <section class="panel">
        <div class="panel-head">
          <h2 class="panel-title">Gewinn-Verlauf <span class="faint">· {NAMES[totals.currency] ?? totals.currency}</span></h2>
          {#if series.length > 1}
            <div class="legend">
              {#each series as s (s.key)}<span><i class="sw" style="background:{s.color}"></i>{s.label}</span>{/each}
            </div>
          {/if}
        </div>
        <div class="chart">
          <ResultsChart
            {points}
            {series}
            height={260}
            unit={cur ?? ""}
            xLabel="Turniere"
            pointLabel="Turnier"
            format={(v) => signedCash(Math.round(v * 100), cur)}
            tick={(v) => (v === 0 ? "0" : signedCash(Math.round(v * 100), cur).replace(/,00$/, ""))}
            empty="Noch kein Turnier mit Ergebnis in dieser Währung. Ergebnisse kommen aus den Turnier-Zusammenfassungen."
          />
        </div>
      </section>
    {/if}

    <section class="panel list">
      <div class="scroll">
        <table class="table">
          <thead>
            <tr>
              {#each COLS as [col, label] (col)}
                <th
                  class="sortable"
                  class:sorted={sortBy === col}
                  class:asc={sortBy === col && asc}
                  aria-sort={sortBy === col ? (asc ? "ascending" : "descending") : "none"}
                  onclick={() => sortOn(col)}>{label}</th
                >
              {/each}
              <th><span class="sr">Coach</span></th>
            </tr>
          </thead>
          <tbody>
            {#each rows as r (r.id)}
              <tr
                class:on={r.id === selected}
                aria-selected={r.id === selected}
                onclick={() => onHands(r.id)}
                title="Nur dieses Turnier zeigen und seine Hände öffnen"
              >
                <td class="date">{r.started_at ? dateTime(r.started_at) : "–"}</td>
                <td class="game">
                  <span class="gcell">
                    <span class="gname" title={r.name ? `${r.name} · ${r.game}` : r.game}>{r.name || r.game || "Turnier"}</span>
                    <span class="faint">#{r.id}</span>
                  </span>
                </td>
                <td title={buyInTitle(r)}>{buyInText(r)}</td>
                <td>{r.entrants === null ? "–" : int(r.entrants)}</td>
                {#if r.place !== null}
                  <td>{int(r.place)}.</td>
                {:else if r.prize !== null}
                  <td title="Ausgeschieden ohne Preisgeld; den Platz nennt nur die Turnier-Zusammenfassung.">raus</td>
                {:else}
                  <td>–</td>
                {/if}
                <td class:pos={!!r.prize}>{r.prize ? cash(r.prize, r.currency) : "–"}</td>
                <td>{r.bounties ? cash(r.bounties, r.currency) : "–"}</td>
                <td class={sign(r.profit)} title={resultTitle(r)}>
                  {#if r.profit === null}<span class="unknown">offen</span>{:else}{signedCash(r.profit, r.currency)}{/if}
                </td>
                <td>{int(r.hands)}</td>
                <td class="coach">
                  {#if notAnalyzable(r) === null}
                    <button
                      class="btn analyze"
                      title="Vom KI-Coach analysieren lassen. Vorher siehst du, was gesendet wird und was es kostet."
                      onclick={(e) => {
                        e.stopPropagation();
                        onAnalyze(r.id);
                      }}>Analysieren</button
                    >
                  {:else}
                    <span class="na" title={notAnalyzable(r)}>–</span>
                  {/if}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    </section>
  {/if}
</div>

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding-top: 20px;
  }
  .cur {
    align-self: flex-start;
  }
  .kpis {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(132px, 1fr));
  }
  .kpi {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 14px 16px 15px;
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
    font-size: 24px;
    font-weight: 600;
    line-height: 1.15;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .kpi.split {
    justify-content: center;
    gap: 6px;
  }
  .pair {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 10px;
    font-size: 12.5px;
    color: var(--text-2);
    cursor: help;
  }
  .pair b {
    font-size: 19px;
    font-weight: 600;
    color: var(--text);
  }
  .pair b.pos {
    color: var(--pos);
  }
  .pair b.neg {
    color: var(--neg);
  }
  .kpi small {
    font-size: 12px;
    color: var(--text-3);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .panel-title .faint {
    font-weight: 400;
  }
  .legend {
    display: flex;
    gap: 16px;
    font-size: 12.5px;
    color: var(--text-2);
  }
  .sw {
    display: inline-block;
    width: 12px;
    height: 2px;
    margin-right: 7px;
    vertical-align: 4px;
  }
  .chart {
    padding: 10px 14px 10px 4px;
  }
  .list {
    overflow: hidden;
  }
  .scroll {
    overflow-x: auto;
  }
  .table td {
    padding: 0 10px;
    height: 32px;
    font-size: 13.5px;
    white-space: nowrap;
  }
  tbody tr {
    cursor: pointer;
  }
  tbody tr.on td {
    background: var(--surface-2);
  }
  tbody tr.on td:first-child {
    box-shadow: inset 2px 0 0 var(--text);
  }
  td.date {
    color: var(--text-2);
  }
  td.game {
    text-align: left;
    font-family: var(--font-ui);
    font-size: 13px;
  }
  .gcell {
    display: flex;
    align-items: baseline;
    gap: 8px;
    max-width: 340px;
  }
  .gname {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  th:nth-child(2) {
    text-align: left;
  }
  .game .faint {
    flex-shrink: 0;
    font-family: var(--font-num);
    font-size: 12px;
  }
  td.coach {
    width: 1%;
    padding-right: 12px;
  }
  .analyze {
    height: 24px;
    padding: 0 9px;
    font-size: 12px;
  }
  .na {
    color: var(--text-3);
    cursor: help;
  }
  .sr {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
  }
  .unknown {
    font-family: var(--font-ui);
    font-size: 12.5px;
    color: var(--text-3);
  }

  @container (max-width: 820px) {
    .legend {
      display: none;
    }
  }
</style>
