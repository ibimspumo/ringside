<script lang="ts">
  import { untrack } from "svelte";
  import AnalyzeBar from "../analysis/AnalyzeBar.svelte";
  import HandPanel from "../analysis/HandPanel.svelte";
  import { RATING, SOURCE, SPOT, SPOT_GROUPS, SPOT_HINT, bb } from "../analysis/labels";
  import { analysis, watchAnalysis } from "../analysis/store.svelte";
  import Summary from "../analysis/Summary.svelte";
  import type { Decision, DecisionBrief, DecisionFilter, Rating, Source, SpotType } from "../analysis/types";
  import { api } from "../api";
  import PlayingCard from "../PlayingCard.svelte";
  import { moneyShort } from "../stats";
  import type { Filter, Hand, HandQuery, HandSummary, SpotFilter, Street } from "../types";
  import type { RangeFilter } from "../ranges/types";
  import { date, dateTime, sign, signed, time } from "./format";
  import type { LearnTarget } from "./Leaks.svelte";
  import Replayer from "./Replayer.svelte";

  let {
    filter,
    refresh,
    player,
    tournamentNames,
    openId,
    spot = null,
    onClearPlayer,
    onClearTournament,
    onClearSpot = () => {},
    onLearn,
  }: {
    filter: Filter;
    refresh: number;
    player: string | null;
    /** Names known from table windows, by tournament id. */
    tournamentNames: Map<number, string>;
    openId: number | null;
    /** Hands of a leak-finder spot or a range-chart cell, with a label for the chip. */
    spot?: { filter?: SpotFilter | null; range?: RangeFilter | null; label: string } | null;
    onClearPlayer: () => void;
    onClearTournament: () => void;
    onClearSpot?: () => void;
    onLearn?: (target: LearnTarget) => void;
  } = $props();

  watchAnalysis();

  type View = "all" | "mistakes" | "open";
  const VIEWS: [View, string, string][] = [
    ["mistakes", "Meine teuersten Fehler", "Hände mit mindestens einer ungenauen Entscheidung, die teuersten zuerst"],
    ["all", "Alle Hände", "Alle Hände, die neuesten zuerst"],
    ["open", "Noch nicht bewertet", "Hände, die noch nicht analysiert sind oder deren Flop-Spot noch gerechnet wird"],
  ];
  const PAGE = 100;
  const SUMMARY_KEY = "ringside.hands.summary";

  // Coming from another page with a hand, a player or a leak, the newest hands fit better.
  let view = $state<View | null>(untrack(() => (openId !== null || spot || player ? "all" : null)));
  let showdownOnly = $state(false);
  let result = $state<"won" | "lost" | null>(null);
  let minPot = $state<number | null>(null);
  let spots = $state<SpotType[]>([]);
  let minRating = $state<Rating | null>(null);
  let minLoss = $state<number | null>(null);
  let street = $state<Street | null>(null);
  let source = $state<Source | null>(null);
  let filtersOpen = $state(false);
  let showSummary = $state(readSummary());

  let list = $state<HandSummary[]>([]);
  let loaded = $state(false);
  let more = $state(false);
  let selected = $state<Hand | null>(null);
  let decisions = $state<Decision[] | null>(null);
  let chosen = $state<number | null>(null);
  let jumpTo = $state<{ action: number } | null>(null);
  let busy = $state(false);
  let tournamentBusy = $state(false);

  function readSummary(): boolean {
    try {
      return localStorage.getItem(SUMMARY_KEY) !== "0";
    } catch {
      return true;
    }
  }

  function toggleSummary() {
    showSummary = !showSummary;
    try {
      localStorage.setItem(SUMMARY_KEY, showSummary ? "1" : "0");
    } catch {
      // Only a convenience.
    }
  }

  // Start on the costliest mistakes once there are any.
  $effect(() => {
    const s = analysis.status;
    if (view === null && s) view = s.counts.rated > 0 ? "mistakes" : "all";
  });

  const locked = $derived(analysis.status?.state === "locked");
  const tournament = $derived(filter.tournament_id);
  const extra = $derived(spots.length + (minRating ? 1 : 0) + (minLoss ? 1 : 0) + (street ? 1 : 0) + (source ? 1 : 0) + (showdownOnly ? 1 : 0) + (result ? 1 : 0) + (minPot ? 1 : 0));
  const decisionFilter = $derived.by((): DecisionFilter | null => {
    const f: DecisionFilter = {
      min_rating: minRating ?? (view === "mistakes" ? "inaccuracy" : null),
      min_loss_bb: minLoss && minLoss > 0 ? minLoss : null,
      spots: [...spots],
      street,
      source,
      unrated: view === "open",
    };
    const empty = !f.min_rating && f.min_loss_bb === null && !f.spots.length && !f.street && !f.source && !f.unrated;
    return empty ? null : f;
  });
  const query = $derived<HandQuery>({
    ...filter,
    player,
    showdown_only: showdownOnly,
    result,
    min_pot_bb: minPot && minPot > 0 ? minPot : null,
    spot: spot?.filter ?? null,
    range: spot?.range ?? null,
    decisions: decisionFilter,
    sort_by_loss: view === "mistakes",
    limit: PAGE,
    offset: 0,
  });

  $effect(() => {
    void refresh;
    void analysis.solved;
    if (view === null) return;
    const q = $state.snapshot(query) as HandQuery;
    let live = true;
    api.hands(q).then((r) => {
      if (!live) return;
      list = r;
      loaded = true;
      more = r.length === PAGE;
      const stale = !selected || (tournament !== null && selected.tournament_id !== tournament);
      if (openId === null && r[0] && stale) open(r[0].id, r[0].worst?.action_index ?? null);
    });
    return () => (live = false);
  });

  $effect(() => {
    if (openId !== null) untrack(() => open(openId, null, true));
  });

  // A finished solve of the hand on screen shows at once.
  $effect(() => {
    const id = analysis.lastSolved;
    void analysis.solved;
    untrack(() => {
      if (selected && id === selected.id) loadDecisions(selected.id, chosen);
    });
  });

  async function loadMore() {
    const r = await api.hands({ ...($state.snapshot(query) as HandQuery), offset: list.length });
    list = [...list, ...r];
    more = r.length === PAGE;
  }

  /** `jump`: start the replay at the chosen decision, as when coming from another page. */
  async function open(id: number, at: number | null = null, jump = false) {
    const hand = await api.hand(id);
    selected = hand;
    decisions = null;
    chosen = null;
    if (!hand) return;
    await loadDecisions(id, at);
    if (analysis.request === id) {
      analysis.request = null;
      await analyze();
    }
    if (chosen !== null && (jump || (at !== null && view === "mistakes"))) jumpTo = { action: chosen };
  }

  async function loadDecisions(id: number, at: number | null) {
    try {
      const d = await api.analysisDecisions(id);
      if (selected?.id !== id) return;
      decisions = d;
      chosen = at ?? worstOf(d);
    } catch {
      decisions = [];
    }
  }

  function worstOf(list: Decision[]): number | null {
    const rated = list.filter((d) => d.status === "rated").sort((a, b) => (b.ev_loss_bb ?? 0) - (a.ev_loss_bb ?? 0));
    return (rated[0] ?? list[0])?.action_index ?? null;
  }

  async function analyze(id = selected?.id) {
    if (id === undefined) return;
    busy = true;
    try {
      const d = await api.analysisHands([id]);
      if (selected?.id === id) {
        decisions = d;
        chosen = worstOf(d);
        if (chosen !== null) jumpTo = { action: chosen };
      }
      list = list.map((h) => (h.id === id ? { ...h, worst: brief(d) } : h));
    } finally {
      busy = false;
    }
  }

  async function analyzeTournament() {
    if (tournament === null) return;
    tournamentBusy = true;
    try {
      const hands = await api.hands({ ...filter, player: null, showdown_only: false, result: null, min_pot_bb: null, limit: 5000, offset: 0 });
      await api.analysisHands(hands.map((h) => h.id));
      if (selected) await loadDecisions(selected.id, chosen);
      analysis.solved++;
    } finally {
      tournamentBusy = false;
    }
  }

  function brief(list: Decision[]): DecisionBrief | null {
    const id = worstOf(list);
    return list.find((d) => d.action_index === id) ?? null;
  }

  function select(action: number) {
    chosen = action;
    jumpTo = { action };
  }

  function toggleSpot(s: SpotType) {
    spots = spots.includes(s) ? spots.filter((x) => x !== s) : [...spots, s];
  }

  function clearFilters() {
    spots = [];
    minRating = null;
    minLoss = null;
    street = null;
    source = null;
    showdownOnly = false;
    result = null;
    minPot = null;
  }

  const marks = $derived.by(() => {
    if (!decisions?.length) return null;
    const m = new Map<number, { color: string; label: string }>();
    for (const d of decisions) {
      const label = d.rating ? `${RATING[d.rating].label}: ${d.played}` : d.status === "pending" ? `Wird gerechnet: ${d.played}` : `Nicht bewertet: ${d.played}`;
      m.set(d.action_index, { color: d.rating ? RATING[d.rating].color : "var(--text-3)", label });
    }
    return m;
  });

  const cards = (s: string) => s.split(" ").filter(Boolean);
  const netBb = (h: HandSummary) => (h.hero_net !== null && h.big_blind > 0 ? h.hero_net / h.big_blind : null);

  function verdict(w: DecisionBrief): string {
    if (w.status === "pending") return `${w.played} · wird gerechnet`;
    if (w.status === "unrated") return "nicht bewertet";
    if (w.rating === "best" || w.rating === "good" || !w.best) return `${w.played.split(" ")[0]} · ${SPOT[w.spot]}`;
    return `${w.played.split(" ")[0]} statt ${w.best}`;
  }

  function stakes(h: { game: string; small_blind: number; big_blind: number; currency: string | null }) {
    if (h.game === "tournament") return `Turnier · ${Math.round(h.small_blind / 100)}/${Math.round(h.big_blind / 100)}`;
    return `${moneyShort(h.small_blind, h.currency)}/${moneyShort(h.big_blind, h.currency)}`;
  }
</script>

<div class="hands">
  <div class="top">
    <AnalyzeBar compact />
    {#if view === "mistakes" && showSummary}
      <Summary
        strip
        {filter}
        {refresh}
        onOpen={(id, at) => open(id, at).then(() => (jumpTo = { action: at }))}
        onSpot={(s) => (spots = [s])}
        {onLearn}
      />
    {/if}
  </div>

  <div class="bar">
    <div class="seg views" role="group" aria-label="Ansicht">
      {#each VIEWS as [id, label, hint] (id)}
        <button class:on={view === id} title={hint} onclick={() => (view = id)}>{label}</button>
      {/each}
    </div>
    <button class="btn ghost more-filters" class:on={filtersOpen} aria-expanded={filtersOpen} onclick={() => (filtersOpen = !filtersOpen)}>
      Filter{#if extra}<span class="count num">{extra}</span>{/if}
      <svg viewBox="0 0 24 24" width="12" height="12" aria-hidden="true"><path d={filtersOpen ? "M6 15l6-6 6 6" : "M6 9l6 6 6-6"} /></svg>
    </button>
    {#if view === "mistakes"}
      <button class="btn ghost" onclick={toggleSummary}>{showSummary ? "Zusammenfassung ausblenden" : "Zusammenfassung zeigen"}</button>
    {/if}
    {#each spots as s (s)}
      <span class="chip">
        {SPOT[s]}
        <button onclick={() => toggleSpot(s)} aria-label="Filter {SPOT[s]} entfernen" title="Filter entfernen">
          <svg viewBox="0 0 24 24" width="12" height="12"><path d="M6 6l12 12M18 6 6 18" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" /></svg>
        </button>
      </span>
    {/each}
    {#if tournament !== null}
      {@const name = tournamentNames.get(tournament)}
      <span class="chip" title={name ? `Turnier #${tournament}` : undefined}>
        {#if name}<span class="chip-name">{name}</span>{:else}Turnier <span class="num">#{tournament}</span>{/if}
        <button onclick={onClearTournament} aria-label="Turnierfilter entfernen" title="Turnierfilter entfernen">
          <svg viewBox="0 0 24 24" width="12" height="12"><path d="M6 6l12 12M18 6 6 18" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" /></svg>
        </button>
      </span>
      <button class="btn" disabled={tournamentBusy || locked} onclick={analyzeTournament} title="Bewertet alle Hände dieses Turniers sofort; die Flop-Spots kommen in der Warteschlange nach vorn.">
        {tournamentBusy ? "Analysiere …" : "Turnier analysieren"}
      </button>
    {/if}
    {#if spot}
      <span class="chip">
        <span class="chip-name">{spot.label}</span>
        <button onclick={onClearSpot} aria-label="Filter entfernen" title="Filter entfernen">
          <svg viewBox="0 0 24 24" width="12" height="12"><path d="M6 6l12 12M18 6 6 18" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" /></svg>
        </button>
      </span>
    {/if}
    {#if player}
      <span class="chip">
        mit {player}
        <button onclick={onClearPlayer} aria-label="Spielerfilter entfernen" title="Spielerfilter entfernen">
          <svg viewBox="0 0 24 24" width="12" height="12"><path d="M6 6l12 12M18 6 6 18" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" /></svg>
        </button>
      </span>
    {/if}
  </div>

  {#if filtersOpen}
    <div class="filters">
      <div class="group wide">
        <span class="title">Situation</span>
        <div class="spot-groups">
          {#each SPOT_GROUPS as g (g.label)}
            <div class="spot-group">
              <span class="faint">{g.label}</span>
              <div class="toggles">
                {#each g.spots as s (s)}
                  <button class="toggle" class:on={spots.includes(s)} title={SPOT_HINT[s]} onclick={() => toggleSpot(s)}>{SPOT[s]}</button>
                {/each}
              </div>
            </div>
          {/each}
        </div>
      </div>
      <div class="group">
        <span class="title">Bewertung</span>
        <div class="seg" role="group" aria-label="Bewertung">
          <button class:on={minRating === null} onclick={() => (minRating = null)}>Alle</button>
          <button class:on={minRating === "inaccuracy"} onclick={() => (minRating = "inaccuracy")}>ab ungenau</button>
          <button class:on={minRating === "mistake"} onclick={() => (minRating = "mistake")}>nur Fehler</button>
        </div>
        <label class="inline" title="Nur Entscheidungen, die mindestens so viel kosten">
          kostet ab
          <input class="field num" type="number" min="0" step="0.5" placeholder="0" bind:value={minLoss} />
          BB
        </label>
      </div>
      <div class="group">
        <span class="title">Straße und Quelle</span>
        <div class="seg" role="group" aria-label="Straße">
          <button class:on={street === null} onclick={() => (street = null)}>Alle</button>
          <button class:on={street === "preflop"} onclick={() => (street = "preflop")}>Preflop</button>
          <button class:on={street === "flop"} onclick={() => (street = "flop")}>Flop</button>
          <button class:on={street === "turn"} onclick={() => (street = "turn")}>Turn</button>
          <button class:on={street === "river"} onclick={() => (street = "river")}>River</button>
        </div>
        <div class="seg" role="group" aria-label="Quelle">
          <button class:on={source === null} onclick={() => (source = null)}>Alle</button>
          {#each ["equilibrium", "model", "solver"] as const as s (s)}
            <button class:on={source === s} title={SOURCE[s].explain} onclick={() => (source = s)}>{SOURCE[s].label}</button>
          {/each}
        </div>
      </div>
      <div class="group">
        <span class="title">Hand</span>
        <div class="seg" role="group" aria-label="Ergebnis">
          <button class:on={result === null} onclick={() => (result = null)}>Alle</button>
          <button class:on={result === "won"} onclick={() => (result = "won")}>Gewonnen</button>
          <button class:on={result === "lost"} onclick={() => (result = "lost")}>Verloren</button>
        </div>
        <label class="inline"><input type="checkbox" bind:checked={showdownOnly} /> Nur Showdown</label>
        <label class="inline">
          Pot ab
          <input class="field num" type="number" min="0" step="5" placeholder="0" bind:value={minPot} />
          BB
        </label>
      </div>
      {#if extra}<button class="btn ghost reset" onclick={clearFilters}>Filter zurücksetzen</button>{/if}
    </div>
  {/if}

  <div class="body">
    <div class="list" role="listbox" aria-label="Hände">
      {#each list as h (h.id)}
        {@const net = netBb(h)}
        {@const w = h.worst}
        <div
          class="row"
          class:on={selected?.id === h.id}
          role="option"
          tabindex="0"
          aria-selected={selected?.id === h.id}
          onclick={() => open(h.id, w?.action_index ?? null)}
          onkeydown={(e) => (e.key === "Enter" || e.key === " ") && (e.preventDefault(), open(h.id, w?.action_index ?? null))}
        >
          <span class="hole">
            {#each cards(h.hero_cards) as c (c)}<PlayingCard card={c} size={22} />{:else}<span class="faint">–</span>{/each}
          </span>
          <span class="mid">
            <span class="top-line">
              <b>{h.hero_position ?? "–"}</b>
              {#if h.hero_showdown}<span class="sd" title="Showdown">SD</span>{/if}
              <span class="board">
                {#each cards(h.board) as c (c)}<PlayingCard card={c} size={14} />{/each}
              </span>
            </span>
            {#if w}
              <span class="verdict" style="--c:{w.rating ? RATING[w.rating].color : 'var(--text-3)'}" class:open={!w.rating} title={w.rating ? `${RATING[w.rating].label} · ${SPOT[w.spot]}` : (w.reason ?? "")}>
                <i></i>{verdict(w)}
              </span>
            {:else if !locked}
              <button class="analyze" onclick={(e) => (e.stopPropagation(), open(h.id).then(() => analyze(h.id)))}>Analysieren</button>
            {/if}
          </span>
          <span class="end">
            {#if w?.ev_loss_bb && w.ev_loss_bb >= 0.05 && w.rating && w.rating !== "best"}
              <span class="loss num" style="color:{RATING[w.rating].color}" title="Was diese Entscheidung im Schnitt kostet">−{bb(w.ev_loss_bb, 2)}</span>
            {:else}
              <span class="net num {sign(net)}">{net === null ? "–" : signed(net)}<span class="unit">BB</span></span>
            {/if}
            <small class="num" title={date(h.played_at)}>{time(h.played_at)} · {date(h.played_at).slice(0, 6)}</small>
          </span>
        </div>
      {:else}
        {#if loaded}
          <div class="empty-state">
            {#if view === "mistakes"}
              {#if (analysis.status?.counts.rated ?? 0) === 0}
                Noch nichts analysiert. Starte oben die Analyse – deine teuersten Fehler erscheinen dann hier.
              {:else}
                Keine ungenauen Entscheidungen für diese Filter. Gut gespielt – oder lockere die Filter.
              {/if}
            {:else}
              Keine Hände für diese Filter. Lockere die Filter oder wähle oben einen längeren Zeitraum.
            {/if}
          </div>
        {/if}
      {/each}
      {#if more}
        <button class="btn more" onclick={loadMore}>Mehr laden</button>
      {/if}
    </div>

    <div class="stage">
      {#if selected}
        <Replayer hand={selected} {marks} {jumpTo} onMark={select}>
          {@const name = selected.tournament_id === null ? undefined : tournamentNames.get(selected.tournament_id)}
          <div class="info">
            <h2 title={name ? `${name} · Tisch ${selected.table_name}` : selected.table_name}>{name ?? selected.table_name}</h2>
            {#if name}<span>Tisch {selected.table_name.split(" ").at(-1)}</span>{/if}
            <span>{stakes(selected)}</span>
            <span>{dateTime(selected.played_at)}</span>
            <span class="faint num">#{selected.id}</span>
          </div>
          {#snippet aside()}
            <HandPanel {decisions} selected={chosen} {busy} {locked} onAnalyze={() => analyze()} onSelect={select} {onLearn} />
          {/snippet}
        </Replayer>
      {:else if loaded && list.length}
        <div class="empty-state">Wähle links eine Hand zum Nachspielen.</div>
      {/if}
    </div>
  </div>
</div>

<style>
  .hands {
    display: flex;
    flex-direction: column;
    height: 100%;
  }
  .top {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px 24px 0;
  }
  .bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 12px;
    padding: 10px 24px;
    border-bottom: 1px solid var(--line);
    font-size: 13px;
    color: var(--text-2);
  }
  .more-filters {
    gap: 6px;
  }
  .more-filters svg path {
    fill: none;
    stroke: currentColor;
    stroke-width: 2.2;
    stroke-linecap: round;
  }
  .more-filters.on {
    color: var(--text);
    background: var(--surface-2);
  }
  .count {
    min-width: 17px;
    height: 17px;
    padding: 0 4px;
    border-radius: 9px;
    font-size: 11px;
    line-height: 17px;
    text-align: center;
    color: var(--bg);
    background: var(--text);
  }
  .filters {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    gap: 14px 28px;
    padding: 12px 24px 14px;
    border-bottom: 1px solid var(--line);
    background: var(--surface);
    font-size: 13px;
    color: var(--text-2);
  }
  .group {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 7px;
  }
  .group.wide {
    flex-basis: 100%;
  }
  .title {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-3);
  }
  .spot-groups {
    display: flex;
    flex-wrap: wrap;
    gap: 8px 22px;
  }
  .spot-group {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 12px;
  }
  .toggles {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .toggle {
    height: 28px;
    padding: 0 10px;
    border: 1px solid var(--line-2);
    border-radius: 999px;
    background: transparent;
    color: var(--text-2);
    font-size: 12.5px;
    cursor: pointer;
  }
  .toggle:hover {
    color: var(--text);
  }
  .toggle.on {
    border-color: var(--text);
    background: var(--surface-3);
    color: var(--text);
  }
  .inline {
    display: flex;
    align-items: center;
    gap: 7px;
    cursor: pointer;
  }
  .inline input[type="number"] {
    width: 64px;
    height: 30px;
    text-align: right;
    color: var(--text);
  }
  .inline input[type="checkbox"] {
    margin: 0;
    accent-color: var(--text);
  }
  .reset {
    align-self: flex-end;
  }
  .body {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: 360px minmax(0, 1fr);
  }
  .list {
    overflow: auto;
    border-right: 1px solid var(--line);
    padding: 6px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 7px 10px;
    border-radius: var(--r-sm);
    cursor: pointer;
  }
  .row:hover {
    background: var(--surface);
  }
  .row.on {
    background: var(--surface-2);
  }
  .row:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: -2px;
  }
  .hole {
    display: flex;
    gap: 2px;
    width: 48px;
    flex-shrink: 0;
  }
  .mid {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .top-line {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12.5px;
  }
  .top-line b {
    font-family: var(--font-num);
    font-weight: 600;
    font-size: 13px;
    min-width: 30px;
  }
  .sd {
    padding: 0 4px;
    border-radius: 3px;
    font-family: var(--font-num);
    font-size: 10.5px;
    color: var(--sd);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--sd) 45%, transparent);
  }
  .board {
    display: flex;
    gap: 1px;
    align-items: center;
  }
  .verdict {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    overflow: hidden;
    font-size: 12px;
    white-space: nowrap;
    text-overflow: ellipsis;
    color: var(--text-2);
  }
  .verdict i {
    flex-shrink: 0;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--c);
  }
  .verdict.open i {
    background: transparent;
    border: 1.5px dashed var(--c);
  }
  .analyze {
    align-self: flex-start;
    height: 22px;
    padding: 0 8px;
    border: 1px solid var(--line-2);
    border-radius: 999px;
    background: transparent;
    color: var(--text-2);
    font-size: 11.5px;
    cursor: pointer;
  }
  .analyze:hover {
    color: var(--text);
    border-color: var(--text-2);
  }
  .end {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 3px;
    white-space: nowrap;
  }
  .net,
  .loss {
    font-size: 15px;
  }
  .end small {
    font-size: 11.5px;
    color: var(--text-3);
  }
  .more {
    width: 100%;
    margin-top: 6px;
  }
  .stage {
    display: flex;
    flex-direction: column;
    min-height: 0;
    overflow: hidden;
    padding: 16px 24px 18px;
  }
  .info {
    display: flex;
    align-items: baseline;
    flex-wrap: wrap;
    gap: 4px 14px;
    min-height: 26px;
    color: var(--text-2);
    font-size: 13px;
  }
  .chip-name {
    max-width: 260px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .info h2 {
    max-width: 100%;
    margin: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-num);
    font-size: 17px;
    font-weight: 600;
    color: var(--text);
  }

  @container (max-width: 980px) {
    .body {
      grid-template-columns: 300px minmax(0, 1fr);
    }
    .stage {
      padding: 14px 16px 16px;
    }
    .top,
    .bar,
    .filters {
      padding-inline: 16px;
    }
  }
</style>
