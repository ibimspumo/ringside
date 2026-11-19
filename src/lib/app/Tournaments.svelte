<script lang="ts">
  import { adviceLocked, watchPokerClient } from "./client.svelte";
  import { api } from "../api";
  import { buyInText, cash, tournamentName } from "../stats";
  import type { Filter, TournamentRow, Tournaments } from "../types";
  import { dateTime, int, sign, signed } from "./format";
  import ResultsChart, { type Series } from "./ResultsChart.svelte";
  import Button from "../ui/Button.svelte";
  import EmptyState from "../ui/EmptyState.svelte";
  import Panel from "../ui/Panel.svelte";
  import Term from "../ui/Term.svelte";

  let {
    filter,
    refresh,
    selected,
    onReport,
  }: {
    filter: Filter;
    refresh: number;
    selected: number | null;
    /** Opens the report of a tournament. */
    onReport: (tournament: number) => void;
  } = $props();

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
  let retry = $state(0);
  let picked = $state<string | null>(null);
  let sortBy = $state<Col>("date");
  let asc = $state(false);

  $effect(() => {
    void refresh;
    void retry;
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

  const ROI_HELP = "ROI (Return on Investment): Gewinn je eingesetztem Buy-in. +10 % heißt: Für 1 $ Buy-in kamen im Schnitt 1,10 $ zurück.";
  const ITM_HELP = "ITM (In the Money): Anteil der Turniere, in denen du Preisgeld gewonnen hast.";
  const BUYIN_HELP = "Buy-in: was du für die Teilnahme zahlst, mit Gebühr, Bounty-Anteil und Re-Entrys.";
  const BOUNTY_HELP = "Bounty: Geld, das du bekommst, wenn du in einem Knockout-Turnier einen Spieler rauswirfst.";
  /** Below this many tournaments the ROI is mostly variance. */
  const FEW = 200;

  const signedCash = (amount: number, currency: string | null) => (amount > 0 ? "+" : "") + cash(amount, currency);
  const percent = (v: number | null, digits = 1) => (v === null ? "–" : `${signed(v, digits)}\u00a0%`);

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
    ...(totals?.bounties ? [{ key: "bounties" as const, label: "Bounties (Summe)", color: "var(--sd)", width: 1.5 }] : []),
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

  /** Share of the field this finish beat, 0–1; null without a place or field size. */
  function depth(r: TournamentRow): number | null {
    if (r.place === null || !r.entrants) return null;
    return Math.min(1, Math.max(0, 1 - (r.place - 1) / r.entrants));
  }

  function resultTitle(r: TournamentRow): string {
    if (r.profit !== null) return `Preis + Bounties − Buy-in${r.has_summary ? "" : " (aus deinen Händen, ohne Zusammenfassung)"}`;
    const chips = r.hands ? ` Chips über ${int(r.hands)} Hände: ${signed(r.chips / 100, 0)}.` : "";
    return `Noch kein Ausscheiden in deinen Händen: Das Turnier läuft noch, oder die letzte Hand ist nicht importiert.${chips}`;
  }
  watchPokerClient();
</script>

<div class="page tours">
  {#if failed}
    <Panel>
      <EmptyState
        tone="error"
        title="Die Turniere konnten nicht geladen werden."
        text="Deine Hände sind sicher gespeichert. Versuch es noch einmal; bleibt der Fehler, starte Ringside neu."
      >
        {#snippet action()}<Button variant="primary" onclick={() => retry++}>Erneut laden</Button>{/snippet}
      </EmptyState>
    </Panel>
  {:else if data && data.rows.length === 0}
    <Panel>
      <EmptyState
        title="Noch keine Turniere in diesem Zeitraum"
        text="Ringside liest dafür deine Turnier-Hände und die Zusammenfassungen aus dem Ordner „TournSummary“ neben „HandHistory“. Hast du schon gespielt{filter.since !==
        null
          ? ', wähle oben „Alles“'
          : ', prüfe in den Einstellungen den Ordner der Hand-Historys'}."
      />
    </Panel>
  {:else if data}
    {#if totals}
      {@const back = totals.prizes + totals.bounties}
      {@const most = Math.max(totals.cost, back, 1)}
      <section class="panel verdict">
        <div class="answer">
          <div class="eyebrow">
            <span>Dein Turnier-Ergebnis</span>
            {#if currencies.length > 1}
              <div class="seg cur" role="group" aria-label="Währung">
                {#each currencies as t (t.currency)}
                  <button class:on={t.currency === cur} onclick={() => (picked = t.currency)}>{NAMES[t.currency] ?? t.currency}</button>
                {/each}
              </div>
            {/if}
          </div>
          <strong class="big num {sign(totals.profit)}">{signedCash(totals.profit, cur)}</strong>
          <p class="say">
            {totals.profit >= 0 ? "Du liegst im Plus" : "Du liegst im Minus"} nach {int(totals.tournaments)}
            {totals.tournaments === 1 ? "Turnier" : "Turnieren"}, Preisgeld und Bounties minus Buy-ins.
            {#if totals.tournaments < FEW}<span class="chance"
                >Turnierergebnisse schwanken stark – unter {FEW} Turnieren ist das vor allem ein Zwischenstand.</span
              >{/if}
          </p>
        </div>

        <div class="flow" aria-label="Eingezahlt und zurückbekommen">
          <div class="flow-row">
            <span class="flow-label"><Term tip={BUYIN_HELP}>Buy-ins gesamt</Term></span>
            <span class="flow-bar"><i class="paid" style="width:{(totals.cost / most) * 100}%"></i></span>
            <b class="num">{cash(totals.cost, cur)}</b>
          </div>
          <div class="flow-row">
            <span class="flow-label">Zurück</span>
            <span class="flow-bar">
              <i class="prize" style="width:{(totals.prizes / most) * 100}%"></i>
              <i class="bounty" style="width:{(totals.bounties / most) * 100}%"></i>
            </span>
            <b class="num">{cash(back, cur)}</b>
          </div>
          <div class="flow-key">
            <span><i class="prize"></i>Preisgeld <b class="num">{cash(totals.prizes, cur)}</b> · {int(totals.cashes)}× im Geld</span>
            <span
              ><i class="bounty"></i><Term tip={BOUNTY_HELP}>Bounties</Term>
              <b class="num">{totals.bounties ? cash(totals.bounties, cur) : "–"}</b> aus Knockouts</span
            >
          </div>
        </div>

        <dl class="facts">
          <div>
            <dt><Term tip={ROI_HELP}>ROI</Term></dt>
            <dd class="num {sign(totals.roi)}">{percent(totals.roi)}</dd>
            <small
              >{totals.roi === null
                ? "noch kein Buy-in mit Ergebnis"
                : cur && NAMES[cur]
                  ? "Gewinn je eingesetztem Buy-in."
                  : `Für ${cash(100, cur)} Buy-in kamen im Schnitt ${cash(Math.round(100 + totals.roi), cur)} zurück.`}</small
            >
          </div>
          <div>
            <dt><Term tip={ITM_HELP}>ITM</Term></dt>
            <dd class="num">{totals.itm === null ? "–" : `${int(totals.itm)}\u00a0%`}</dd>
            <small>{totals.itm === null ? "noch kein Turnier beendet" : `${int(totals.cashes)} von ${int(totals.finished)} Turnieren mit Preisgeld.`}</small>
          </div>
          <div>
            <dt>Turniere</dt>
            <dd class="num">{int(totals.tournaments)}</dd>
            <small
              >{totals.finished === totals.tournaments ? "alle mit Ergebnis" : `${int(totals.finished)} mit Ergebnis, ${int(totals.tournaments - totals.finished)} noch offen`}{unknown
                ? ` · ${int(unknown)} ohne Währung`
                : ""} · Ø Buy-in {totals.finished ? cash(Math.round(totals.cost / totals.finished), cur) : "–"}</small
            >
          </div>
        </dl>
      </section>

      <Panel
        flush
        title="Gewinn-Verlauf · {NAMES[totals.currency] ?? totals.currency}"
        sub="Jeder Schritt ist ein Turnier: nach oben mit Preisgeld oder Bounties, nach unten, wenn der Buy-in weg ist."
      >
        {#snippet actions()}
          {#if series.length > 1}
            <div class="legend">
              {#each series as s (s.key)}<span><i class="sw" style="background:{s.color}"></i>{s.label}</span>{/each}
            </div>
          {/if}
        {/snippet}
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
      </Panel>
    {/if}

    <Panel flush title="Deine Turniere" sub="Beträge in der Währung des Turniers · {adviceLocked() ? 'Den Bericht gibt es, sobald du PokerStars schließt' : 'Zeile anklicken: Bericht mit Fehlern, Glück und dem nächsten Übungsschritt'}">
      <table class="table tours-table">
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
              <th class="open-head"><span class="sr">Bericht</span></th>
            </tr>
          </thead>
          <tbody>
            {#each rows as r (r.id)}
              {@const d = depth(r)}
              <tr
                class:on={r.id === selected}
                aria-selected={r.id === selected}
                onclick={() => onReport(r.id)}
                title="Bericht zu diesem Turnier öffnen"
              >
                <td class="date"
                  >{#if r.started_at}{@const [day, clock] = dateTime(r.started_at).split(". ")}<span class="day">{day}.</span
                    ><span class="clock">{clock}</span>{:else}–{/if}</td
                >
                <td class="game">
                  <span class="gcell">
                    <span class="gname" title="{r.name ? `${r.name} · ${r.game}` : r.game} · #{r.id}">{(r.name || r.game || "Turnier").replace(/ (\S{1,3})$/, "\u00a0$1")}</span>
                    <span class="gid">#{r.id}</span>
                  </span>
                </td>
                <td title={buyInTitle(r)}>{buyInText(r)}</td>
                <td>{r.entrants === null ? "–" : int(r.entrants)}</td>
                {#if r.place !== null}
                  <td
                    class="place"
                    title={d === null ? undefined : `Weiter gekommen als ${Math.round(d * 100)} % des Feldes`}
                  >
                    {#if d !== null}<span class="depth" class:paid={!!r.prize} aria-hidden="true"><i style="width:{d * 100}%"></i></span>{/if}
                    {int(r.place)}.{#if r.entrants}<span class="of">von {int(r.entrants)}</span>{/if}
                  </td>
                {:else if r.prize !== null}
                  <td title="Ausgeschieden ohne Preisgeld; den Platz nennt nur die Turnier-Zusammenfassung.">raus</td>
                {:else}
                  <td>–</td>
                {/if}
                <td class:pos={!!r.prize}>{r.prize ? cash(r.prize, r.currency) : "–"}</td>
                <td>{r.bounties ? cash(r.bounties, r.currency) : "–"}</td>
                <td class="{sign(r.profit)} profit" title={resultTitle(r)}>
                  {#if r.profit === null}<span class="unknown">offen</span>{:else}{signedCash(r.profit, r.currency)}{/if}
                </td>
                <td>{int(r.hands)}</td>
                <td class="open" aria-hidden="true">
                  <svg viewBox="0 0 24 24" width="14" height="14"><path d="M9 5l7 7-7 7" /></svg>
                </td>
              </tr>
            {/each}
          </tbody>
      </table>
    </Panel>
  {/if}
</div>

<style>
  .tours {
    display: flex;
    flex-direction: column;
    gap: 18px;
    padding-top: 20px;
  }

  /* The answer, where the money went, and what it means. */
  .verdict {
    display: grid;
    grid-template-columns: minmax(260px, 0.9fr) minmax(300px, 1.3fr) minmax(220px, 0.8fr);
  }
  .verdict > * {
    padding: 20px 22px 18px;
    min-width: 0;
  }
  .verdict > * + * {
    border-left: 1px solid var(--line);
  }
  .answer {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .eyebrow {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    min-height: 26px;
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .cur {
    flex-shrink: 0;
  }
  .big {
    font-size: var(--fs-hero);
    font-weight: 600;
    line-height: 1.05;
    white-space: nowrap;
  }
  .say {
    margin: 2px 0 0;
    font-size: var(--fs-base);
    line-height: 1.5;
  }
  .say .chance {
    display: block;
    margin-top: 4px;
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .flow {
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 12px;
  }
  .flow-row {
    display: grid;
    grid-template-columns: 104px minmax(0, 1fr) auto;
    align-items: center;
    gap: 12px;
  }
  .flow-label {
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .flow-row b {
    min-width: 76px;
    font-size: var(--fs-lg);
    font-weight: 600;
    text-align: right;
  }
  .flow-bar {
    display: flex;
    gap: 2px;
    height: 14px;
    border-radius: 3px;
    background: var(--surface-2);
    overflow: hidden;
  }
  .flow-bar i {
    transform-origin: left;
    animation: grow-x var(--dur-slow) var(--ease-enter) both;
    display: block;
    height: 100%;
  }
  i.paid {
    background: var(--text-3);
  }
  i.prize {
    background: var(--pos);
  }
  i.bounty {
    background: var(--sd);
  }
  .flow-key {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 18px;
    padding-left: 116px;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .flow-key i {
    display: inline-block;
    width: 8px;
    height: 8px;
    margin-right: 6px;
    border-radius: 2px;
  }
  .flow-key b {
    font-weight: 500;
    color: var(--text);
  }
  .facts {
    display: flex;
    flex-direction: column;
    gap: 12px;
    margin: 0;
  }
  .facts div {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: baseline;
    column-gap: 10px;
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
    font-size: var(--fs-md);
    line-height: 1.4;
    color: var(--text-2);
  }

  .legend {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
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
  }
  .chart {
    padding: 10px 14px 10px 4px;
  }
  .tours-table th:nth-child(n + 3) {
    padding-left: 16px;
  }
  .tours-table th {
    color: var(--text-2);
  }
  .tours-table th:first-child,
  .tours-table td:first-child {
    padding-left: var(--sp-4);
  }
  .tours-table td {
    padding: 0 10px;
    height: 34px;
    font-size: var(--fs-base);
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
  .clock::before {
    content: " ";
  }
  td.game {
    text-align: left;
    font-family: var(--font-ui);
    font-size: var(--fs-md);
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
  .gid {
    flex-shrink: 0;
    color: var(--text-2);
    font-family: var(--font-num);
    font-size: var(--fs-sm);
  }
  td.place {
    min-width: 96px;
  }
  .depth {
    display: inline-block;
    width: 40px;
    height: 4px;
    margin-right: 8px;
    vertical-align: 3px;
    border-radius: 2px;
    background: var(--surface-3);
    overflow: hidden;
  }
  .depth i {
    transform-origin: left;
    animation: grow-x var(--dur-slow) var(--ease-enter) both;
    display: block;
    height: 100%;
    background: var(--text-3);
  }
  .depth.paid i {
    background: var(--pos);
  }
  td.profit {
    font-weight: 600;
  }
  td.open {
    width: 1%;
    padding-right: 12px;
    color: var(--text-3);
  }
  td.open svg {
    display: block;
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  tbody tr:hover td.open {
    color: var(--text);
  }
  .sr {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
  .unknown {
    font-family: var(--font-ui);
    font-weight: 400;
    font-size: var(--fs-md);
    color: var(--text-2);
  }

  @container (max-width: 1180px) {
    .verdict {
      grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    }
    .flow {
      grid-column: 1 / -1;
      grid-row: 2;
      border-left: 0 !important;
      border-top: 1px solid var(--line);
    }
  }
  .of {
    display: block;
    font-size: var(--fs-xs);
    line-height: 1.1;
    color: var(--text-2);
  }
  td.place {
    line-height: 1.2;
  }
  /* Narrow windows: the field size moves into the place column so every column fits without scrolling. */
  @container (max-width: 1000px) {
    .gcell {
      flex-direction: column;
      align-items: flex-start;
      gap: 0;
      line-height: 1.25;
    }
    .gid {
      display: none;
    }
    .gname {
      max-width: 100%;
      display: -webkit-box;
      -webkit-box-orient: vertical;
      -webkit-line-clamp: 2;
      line-clamp: 2;
      white-space: normal;
      line-height: 1.2;
    }
    .tours-table th:nth-child(4),
    .tours-table td:nth-child(4),
    .depth {
      display: none;
    }
    .of {
      display: block;
      font-size: var(--fs-xs);
      line-height: 1.1;
    }
    td.place {
      min-width: 0;
    }
    .gcell {
      max-width: 132px;
    }

    td.date span {
      display: block;
      line-height: 1.2;
    }
    .clock::before {
      content: none;
    }
    .clock {
      font-size: var(--fs-sm);
      color: var(--text-2);
    }
    .tours-table td,
    .tours-table th {
      padding-left: 6px;
      padding-right: 6px;
    }
  }

</style>
