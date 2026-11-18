<script lang="ts">
  import { tick, untrack } from "svelte";
  import AnalyzeStatus from "../analysis/AnalyzeStatus.svelte";
  import HandPanel from "../analysis/HandPanel.svelte";
  import { FILTER_SOURCES, GRADE_OF, RATING, SOURCE, SPOT, SPOT_GROUPS, SPOT_HINT, bb, reliabilityText } from "../analysis/labels";
  import { analysis, watchAnalysis } from "../analysis/store.svelte";
  import Summary from "../analysis/Summary.svelte";
  import type { Decision, DecisionBrief, DecisionFilter, Rating, Source, SpotType } from "../analysis/types";
  import { api } from "../api";
  import { preview } from "../learn/preview";
  import { moneyShort } from "../stats";
  import Button from "../ui/Button.svelte";
  import { bb as fmtBb } from "../ui/format";
  import EmptyState from "../ui/EmptyState.svelte";
  import GradeIcon from "../ui/GradeIcon.svelte";
  import IconButton from "../ui/IconButton.svelte";
  import MiniCard from "../ui/MiniCard.svelte";
  import { usePageHead } from "../ui/page.svelte";
  import PlayingCard from "../ui/PlayingCard.svelte";
  import SegmentedControl from "../ui/SegmentedControl.svelte";
  import type { Grade } from "../ui/poker";
  import Tag from "../ui/Tag.svelte";
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
    openAt = null,
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
    /** Decision of `openId` to stop at (action index), e.g. coming back from the ranges page. */
    openAt?: number | null;
    /** Hands of a leak-finder spot, a range-chart cell or a kind of spot of the playing profile, with a label for the chip. */
    spot?: { filter?: SpotFilter | null; range?: RangeFilter | null; profile?: string | null; label: string } | null;
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
  // A hand opened from elsewhere shows in "Alle Hände"; coming back to a decision keeps the usual view.
  let view = $state<View | null>(
    untrack(() => (preview("hands") as View | null) ?? ((openId !== null && openAt === null) || spot || player ? "all" : null)),
  );
  let showdownOnly = $state(false);
  let result = $state<"won" | "lost" | null>(null);
  let minPot = $state<number | null>(null);
  let spots = $state<SpotType[]>([]);
  let minRating = $state<Rating | null>(null);
  let minLoss = $state<number | null>(null);
  let street = $state<Street | null>(null);
  let source = $state<Source | null>(null);
  let filtersOpen = $state(preview("filters") === "1");
  let showSummary = $state(readSummary());
  /** Narrow windows need the height for the table: there the summary only opens on request. */
  let summaryNarrow = $state(false);

  let list = $state<HandSummary[]>([]);
  let loaded = $state(false);
  let more = $state(false);
  let selected = $state<Hand | null>(null);
  let decisions = $state<Decision[] | null>(null);
  let chosen = $state<number | null>(null);
  const current = $derived(decisions?.find((d) => d.action_index === chosen) ?? null);
  let jumpTo = $state<{ action: number } | null>(null);
  let busy = $state(false);
  /** The hand analysed last, so an empty result reads as "nothing to rate". */
  let analyzedId = $state<number | null>(null);
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
  /** Before the first run over all hands, starting that run is the page's main action. */
  const started = $derived(!!analysis.status && (analysis.status.enabled || analysis.status.counts.analyzed > 0));
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
    profile: spot?.profile ?? null,
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
      // A hand the new filters leave out makes room for the first one they keep.
      const stale = !selected || (tournament !== null && selected.tournament_id !== tournament) || !r.some((h) => h.id === selected!.id);
      const first = r[Number(preview("pick") ?? 0)] ?? r[0];
      const at = preview("at");
      if (openId === null && first && stale) open(first.id, at === null ? (first.worst?.action_index ?? null) : Number(at), at !== null);
    });
    return () => (live = false);
  });

  $effect(() => {
    if (openId !== null) untrack(() => open(openId, openAt, true));
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
    // The replay starts right before the decision the verdict is about, as the row promised.
    if (chosen !== null && (jump || preview("step") === null)) jumpTo = { action: chosen };
    else if (standing !== undefined) follow(standing);
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
      analyzedId = id;
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

  /** Playing on shows the verdict of the decision the replay stands before. */
  let standing = $state<number | null | undefined>(undefined);
  /** Says whether the table shows the moment of the decision on the right, or a later one. */
  const caption = $derived.by(() => {
    if (!current || standing === undefined) return null;
    const move = current.played.split(" ")[0];
    if (standing === current.action_index) return { text: `Am Tisch bist du jetzt dran – bewertet wird dein ${move}:`, ahead: false };
    if (standing === null || standing > current.action_index)
      return { text: `Bewertet wird dein ${move} am ${({ preflop: "Preflop", flop: "Flop", turn: "Turn", river: "River" })[current.street]}:`, ahead: false };
    return { text: `Deine nächste Entscheidung – so wird dein ${move} bewertet:`, ahead: true };
  });
  function follow(next: number | null) {
    standing = next;
    if (!decisions?.length) return;
    const d = (next === null ? undefined : decisions.find((x) => x.action_index >= next)) ?? decisions[decisions.length - 1];
    chosen = d.action_index;
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
    if (!decisions?.length || locked) return null;
    const m = new Map<number, { grade: Grade | null; label: string }>();
    for (const d of decisions) {
      const label = d.rating
        ? `${RATING[d.rating].label}: ${d.played}`
        : d.status === "pending"
          ? `Wird gerechnet: ${d.played}`
          : d.status === "forced"
            ? `Keine Entscheidung: ${d.played}`
            : `Nicht bewertet: ${d.played}`;
      m.set(d.action_index, { grade: d.rating ? GRADE_OF[d.rating] : null, label });
    }
    return m;
  });

  const cards = (s: string) => s.split(" ").filter(Boolean);
  const netBb = (h: HandSummary) => (h.hero_net !== null && h.big_blind > 0 ? h.hero_net / h.big_blind : null);
  const costly = (w: DecisionBrief | null | undefined) => !!w?.rating && w.rating !== "best" && w.rating !== "good" && (w.ev_loss_bb ?? 0) >= 0.05;

  /** Where the verdict comes from, in one line; the estimate with its measured reliability. */
  function sourceHint(w: DecisionBrief): string {
    if (!w.source) return "";
    const s = `Quelle: ${SOURCE[w.source].label}`;
    return w.source === "estimate" && w.reliability !== null ? `${s} – ${reliabilityText(w.reliability, w.street)}` : s;
  }

  /** The worst decision of a row in a few words: what was better first. */
  function verdict(w: DecisionBrief): { lead: string; rest: string } {
    const played = w.played.split(" ")[0];
    if (w.status === "pending") return { lead: played, rest: "wird gerechnet" };
    if (w.status === "unrated") return { lead: "", rest: "nicht bewertet" };
    if (w.status === "forced") return { lead: "", rest: "keine Entscheidung" };
    if (!w.rating || w.rating === "best" || w.rating === "good" || !w.best) return { lead: played, rest: w.rating ? RATING[w.rating].label : SPOT[w.spot] };
    return { lead: `Besser ${w.best.split(" ")[0]}`, rest: `statt ${played}` };
  }

  function stakes(h: { game: string; small_blind: number; big_blind: number; currency: string | null }) {
    if (h.game === "tournament") return `Turnier · ${Math.round(h.small_blind / 100)}/${Math.round(h.big_blind / 100)}`;
    return `${moneyShort(h.small_blind, h.currency)}/${moneyShort(h.big_blind, h.currency)}`;
  }

  usePageHead(() => ({
    tabs: VIEWS.map(([id, label, hint]) => ({ id, label, hint })),
    value: view ?? "",
    label: "Welche Hände",
    onchange: (id) => (view = id as View),
    end: headEnd,
  }));

  // Room decides the layout: a card strip instead of the list when narrow, the analysis beside the table when wide.
  let bodyW = $state(1200);
  let detailW = $state(900);
  const rail = $derived(bodyW < 1000);
  const summaryShown = $derived(rail ? summaryNarrow : showSummary);
  function flipSummary() {
    if (rail) summaryNarrow = !summaryNarrow;
    else toggleSummary();
  }
  // Narrow: table and controls fill what is visible below the verdict line, the log follows below.
  let workH = $state(600);
  const replayH = $derived(Math.max(480, workH + 162));
  const wide = $derived(detailW >= 900);

  // The detail keeps the hand's place in the list: up and down walk through it.
  const at = $derived(selected ? list.findIndex((h) => h.id === selected!.id) : -1);
  const summaryOf = $derived(at >= 0 ? list[at] : null);
  let listEl = $state<HTMLElement>();
  let analysisEl = $state<HTMLElement>();

  async function step(by: number) {
    const next = list[at + by];
    if (!next) return;
    await open(next.id, next.worst?.action_index ?? null);
    await tick();
    listEl?.querySelector<HTMLElement>(`[data-id="${next.id}"]`)?.scrollIntoView({ block: "nearest" });
  }

  function key(e: KeyboardEvent) {
    const t = e.target as HTMLElement | null;
    if (t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.tagName === "SELECT")) return;
    if (e.key === "Escape" && filtersOpen) {
      filtersOpen = false;
      return;
    }
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    e.preventDefault();
    step(e.key === "ArrowDown" ? 1 : -1);
  }
</script>
<svelte:window onkeydown={key} />

{#snippet headEnd()}
  <AnalyzeStatus short={rail} />
  {#if view === "mistakes" && !summaryShown && !locked}
    {#if !rail}<Button size="sm" variant="quiet" onclick={flipSummary}>Zusammenfassung</Button>{/if}
  {/if}
  <Button size="sm" variant={filtersOpen || extra ? "secondary" : "quiet"} aria-expanded={filtersOpen} onclick={() => (filtersOpen = !filtersOpen)}>
    {#snippet icon()}<svg class="stroke" viewBox="0 0 24 24" width="14" height="14" aria-hidden="true"><path d="M4 6h16M7 12h10M10 18h4" /></svg>{/snippet}
    Filter{#if extra}<span class="count num">{extra}</span>{/if}
  </Button>
{/snippet}

{#snippet verdictLine()}
  <div class="brief">
    {#if locked}
      <p class="quiet">Die Analyse pausiert, solange PokerStars läuft – Nachspielen geht weiter.</p>
    {:else if current?.rating}
      {@const g = GRADE_OF[current.rating]}
      <GradeIcon grade={g} size={18} />
      <p>
        <b style:color="color-mix(in srgb, var(--grade-{g}) 60%, var(--text))">{RATING[current.rating].label}</b>
        <span class="quiet">· {({ preflop: "Preflop", flop: "Flop", turn: "Turn", river: "River" })[current.street]}</span>
        {#if current.best && (current.rating === "mistake" || current.rating === "blunder" || current.rating === "inaccuracy")}
          – Besser <b>{current.best}</b>, dein {current.played.split(" ")[0]} bringt im Schnitt <b class="num">{fmtBb(current.ev_loss_bb ?? 0, { digits: 2 })}</b> weniger
        {:else}
          – {current.played.split(" ")[0]} passt
        {/if}
      </p>
      <Button size="sm" onclick={() => analysisEl?.scrollIntoView({ block: "start", behavior: "smooth" })}>Warum?</Button>
    {:else if decisions && !decisions.length && !locked}
      <p>Diese Hand ist noch nicht bewertet.</p>
      <Button size="sm" variant={started ? "primary" : "secondary"} loading={busy} disabled={busy} onclick={() => analyze()}>Diese Hand analysieren</Button>
    {:else if current}
      <p class="quiet">{current.status === "pending" ? "Der Solver rechnet diese Entscheidung noch." : "Diese Entscheidung bewertet Ringside nicht."}</p>
    {/if}
  </div>
{/snippet}

{#snippet panel()}
  <HandPanel {decisions} selected={chosen} {busy} {locked} analyzed={analyzedId === selected?.id} onAnalyze={() => analyze()} onSelect={select} {onLearn} {caption} main={started} />
{/snippet}

{#snippet close()}<svg viewBox="0 0 24 24" width="14" height="14" aria-hidden="true"><path d="M6 6l12 12M18 6 6 18" /></svg>{/snippet}

<div class="hands">
  {#if spots.length || tournament !== null || spot || player}
    <div class="chips">
      {#each spots as s (s)}
        <Tag selected onremove={() => toggleSpot(s)} removeLabel="Filter {SPOT[s]} entfernen">{SPOT[s]}</Tag>
      {/each}
      {#if tournament !== null}
        {@const name = tournamentNames.get(tournament)}
        <Tag selected onremove={onClearTournament} removeLabel="Turnierfilter entfernen">
          <span class="chip-name" title={name ? `Turnier #${tournament}` : undefined}>{name ?? `Turnier #${tournament}`}</span>
        </Tag>
        <Button size="sm" disabled={tournamentBusy || locked} onclick={analyzeTournament} title="Bewertet alle Hände dieses Turniers sofort; die Flop-Spots kommen in der Warteschlange nach vorn.">
          {tournamentBusy ? "Analysiere …" : "Turnier analysieren"}
        </Button>
      {/if}
      {#if spot}
        <Tag selected onremove={onClearSpot} removeLabel="Filter entfernen"><span class="chip-name">{spot.label}</span></Tag>
      {/if}
      {#if player}
        <Tag selected onremove={onClearPlayer} removeLabel="Spielerfilter entfernen">mit {player}</Tag>
      {/if}
    </div>
  {/if}

  {#if filtersOpen}
    <button class="scrim" aria-label="Filter schließen" tabindex="-1" onclick={() => (filtersOpen = false)}></button>
    <div class="filters" role="dialog" aria-label="Filter">
      <div class="filters-head">
        <span class="title">Filter</span>
        {#if extra}<Button size="sm" variant="quiet" onclick={clearFilters}>Zurücksetzen</Button>{/if}
        <Button size="sm" kbd="Esc" onclick={() => (filtersOpen = false)}>Fertig</Button>
      </div>
      <div class="group wide">
        <span class="title">Situation</span>
        <div class="spot-groups">
          {#each SPOT_GROUPS as g (g.label)}
            <div class="spot-group">
              <span class="quiet">{g.label}</span>
              <div class="toggles">
                {#each g.spots.filter((x) => x !== "turn" && x !== "river") as s (s)}
                  <span title={SPOT_HINT[s]}><Tag selected={spots.includes(s)} onclick={() => toggleSpot(s)}>{s === "flop" ? "Sonst am Flop" : SPOT[s]}</Tag></span>
                {/each}
              </div>
            </div>
          {/each}
        </div>
      </div>
      <div class="group">
        <span class="title">Bewertung</span>
        <SegmentedControl
          size="sm"
          label="Bewertung"
          options={[
            { value: "all", label: "Alle" },
            { value: "inaccuracy", label: "ab ungenau" },
            { value: "mistake", label: "nur Fehler" },
          ]}
          bind:value={() => minRating ?? "all", (v) => (minRating = v === "all" ? null : (v as Rating))}
        />
        <span class="sub" title="Nur Entscheidungen, die im Schnitt mindestens so viel kosten">Kostet im Schnitt</span>
        <SegmentedControl
          size="sm"
          label="Kostet im Schnitt mindestens"
          options={[
            { value: 0, label: "egal" },
            ...[0.5, 1, 2, 5].map((v) => ({ value: v, label: `ab ${v.toLocaleString("de-DE")} BB` })),
          ]}
          bind:value={() => minLoss ?? 0, (v) => (minLoss = v || null)}
        />
      </div>
      <div class="group">
        <span class="title">Straße und Quelle</span>
        <SegmentedControl
          size="sm"
          label="Straße"
          options={[
            { value: "all", label: "Alle" },
            { value: "preflop", label: "Preflop" },
            { value: "flop", label: "Flop" },
            { value: "turn", label: "Turn" },
            { value: "river", label: "River" },
          ]}
          bind:value={() => street ?? "all", (v) => (street = v === "all" ? null : (v as Street))}
        />
        <SegmentedControl
          size="sm"
          label="Quelle"
          options={[{ value: "all", label: "Alle" }, ...FILTER_SOURCES.map((s) => ({ value: s, label: SOURCE[s].label, hint: SOURCE[s].explain }))]}
          bind:value={() => source ?? "all", (v) => (source = v === "all" ? null : (v as Source))}
        />
        <p class="note">Woher die Bewertung kommt. Zeig auf eine Quelle, um zu lesen, wie sie rechnet.</p>
      </div>
      <div class="group">
        <span class="title">Hand</span>
        <SegmentedControl
          size="sm"
          label="Ergebnis"
          options={[
            { value: "all", label: "Alle" },
            { value: "won", label: "Gewonnen" },
            { value: "lost", label: "Verloren" },
          ]}
          bind:value={() => result ?? "all", (v) => (result = v === "all" ? null : (v as "won" | "lost"))}
        />
        <label class="inline"><input type="checkbox" bind:checked={showdownOnly} /> Nur Showdown</label>
        <span class="sub">Pot mindestens</span>
        <SegmentedControl
          size="sm"
          label="Pot mindestens"
          options={[{ value: 0, label: "egal" }, ...[10, 25, 50, 100].map((v) => ({ value: v, label: `${v} BB` }))]}
          bind:value={() => minPot ?? 0, (v) => (minPot = v || null)}
        />
      </div>

    </div>
  {/if}

  {#if view === "mistakes" && summaryShown && !locked}
    <div class="summary">
      <Summary
        strip
        {filter}
        {refresh}
        onOpen={(id, at) => open(id, at).then(() => (jumpTo = { action: at }))}
        onSpot={(s) => (spots = [s])}
        {onLearn}
      />
      <IconButton size="sm" label="Zusammenfassung ausblenden" onclick={flipSummary}>{@render close()}</IconButton>
    </div>
  {/if}

  <div class="body" class:rail bind:clientWidth={bodyW}>
    <div class="list" role="listbox" aria-label="Hände" bind:this={listEl}>
      {#if list.length}
        <div class="list-head">
          <span class="num">{list.length}{more ? "+" : ""} Hände</span>
          {#if rail}<span class="rail-what">{locked ? "pausiert" : view === "mistakes" ? "nach Kosten" : "Ergebnis"}</span>{/if}
          <span title={view === "mistakes" ? "Rechts: was dein teuerster Fehler der Hand im Schnitt kostet" : "Rechts: was du in der Hand gewonnen oder verloren hast"}
            >{locked ? "Bewertungen pausiert" : view === "mistakes" ? "Kosten des Fehlers · teuerste zuerst" : "Ergebnis · neueste zuerst"}</span
          >
        </div>
      {/if}
      {#each list as h (h.id)}
        {@const net = netBb(h)}
        {@const w = h.worst}
        {@const v = w ? verdict(w) : null}
        <div
          class="row"
          data-id={h.id}
          title={rail
            ? `${h.hero_position ?? ""} ${h.hero_cards} · ${time(h.played_at)} Uhr${w && v ? ` · ${w.rating ? RATING[w.rating].label + ": " : ""}${v.lead} ${v.rest}` : ""}`
            : `${date(h.played_at)}, ${time(h.played_at)} Uhr`}
          class:on={selected?.id === h.id}
          role="option"
          tabindex="0"
          aria-selected={selected?.id === h.id}
          onclick={() => open(h.id, w?.action_index ?? null)}
          onkeydown={(e) => (e.key === "Enter" || e.key === " ") && (e.preventDefault(), open(h.id, w?.action_index ?? null))}
        >
          <span class="who">
            <span class="hole">
              {#each cards(h.hero_cards) as c (c)}<PlayingCard card={c} size={22} />{:else}<span class="quiet">–</span>{/each}
            </span>
            <b class="posn num">{h.hero_position ?? "–"}</b>
          </span>
          <span class="mid">
            <span class="line">
              <span class="board">{#each cards(h.board) as c (c)}<MiniCard card={c} />{:else}<span class="quiet">kein Flop</span>{/each}</span>
              {#if h.hero_showdown}<svg class="sd" viewBox="0 0 24 24" width="13" height="13" role="img" aria-label="Showdown"
                  ><title>Showdown: die Karten wurden aufgedeckt</title><path d="M2.5 12s3.5-6.5 9.5-6.5S21.5 12 21.5 12 18 18.5 12 18.5 2.5 12 2.5 12z" /><circle
                    cx="12"
                    cy="12"
                    r="2.8"
                  /></svg
                >{/if}

            </span>
            {#if w && v && !locked}
              <span class="verdict" title={w.rating ? `${RATING[w.rating].label} · ${SPOT[w.spot]}${w.best ? ` · besser ${w.best}` : ""} · ${sourceHint(w)}` : (w.reason ?? "")}>
                {#if w.rating}<GradeIcon grade={GRADE_OF[w.rating]} size={14} />{:else}<i class:wait={w.status === "pending"}></i>{/if}
                {#if v.lead}<b>{v.lead}</b>{/if}<span>{v.rest}</span>
              </span>
            {:else if !locked}
              <span class="verdict"><i></i><span>noch nicht analysiert</span></span>
            {/if}
          </span>
          {#if rail && w?.rating && !locked}
            <span class="rail-verdict"><GradeIcon grade={GRADE_OF[w.rating]} size={12} />{w.best && costly(w) ? `Besser ${w.best.split(" ")[0]}` : w.played.split(" ")[0]}</span>
          {/if}
          <span class="end">

            {#if view === "mistakes" && costly(w) && !locked}
              <span class="loss num" style:color="var(--grade-{GRADE_OF[w!.rating!]})" title="Was diese Entscheidung im Schnitt kostet">−{fmtBb(w!.ev_loss_bb!, { digits: 2 })}</span>
            {:else}
              <span class="net num {sign(net)}" title="Was du in dieser Hand gewonnen oder verloren hast"
                >{net === null ? "–" : signed(net, Math.abs(net) >= 10 ? 0 : 1)}<span class="bb-unit">BB</span></span
              >
            {/if}

          </span>
        </div>
      {:else}
        {#if loaded}
          {#if view === "mistakes" && (analysis.status?.counts.rated ?? 0) === 0}
            <EmptyState compact title="Noch nichts analysiert" text="Starte oben rechts die Analyse – deine teuersten Fehler erscheinen dann hier." />
          {:else if view === "mistakes"}
            <EmptyState compact title="Keine ungenauen Entscheidungen" text="Gut gespielt – oder lockere die Filter." />
          {:else}
            <EmptyState compact title="Keine Hände für diese Filter" text="Lockere die Filter oder wähle oben einen längeren Zeitraum." />
          {/if}
        {:else}
          <EmptyState compact tone="loading" title="Lädt deine Hände …" />
        {/if}
      {/each}
      {#if more}
        <div class="more"><Button size="sm" full onclick={loadMore}>Weitere 100 Hände laden</Button></div>
      {/if}
    </div>

    <section class="detail" aria-label="Hand im Detail" bind:clientWidth={detailW}>
      {#if selected}
        {@const name = selected.tournament_id === null ? undefined : tournamentNames.get(selected.tournament_id)}
        {@const net = summaryOf ? netBb(summaryOf) : null}
        <header class="hand-head" class:narrow={!wide}>
          <span class="hole">{#each selected.hero_cards as c (c)}<PlayingCard card={c} size={34} />{/each}</span>
          <div class="ident">
            <h2 title="{name ? `${name} · Tisch ${selected.table_name}` : selected.table_name} · {stakes(selected)} · {dateTime(selected.played_at)} · #{selected.id}">
              {#if summaryOf?.hero_position}<span class="num">{summaryOf.hero_position}</span>{/if}
              <span class="table-name">{name ?? selected.table_name}</span>
            </h2>
            <p class="num">
              {#if name}Tisch {selected.table_name.split(" ").at(-1)} · {/if}{stakes(selected)} · {dateTime(selected.played_at)}
              <span title="Nummer der Hand bei PokerStars">#{selected.id}</span>
            </p>
          </div>
          {#if net !== null}
            <div class="result" title="Was du in dieser einen Hand gewonnen oder verloren hast – mit Glück und Pech. Die Bewertung rechts sagt dagegen, was eine Entscheidung im Schnitt kostet.">
              <span>{wide ? "Ergebnis der Hand" : "Ergebnis"}</span>
              <b class="num {sign(net)}">{signed(net, Math.abs(net) >= 10 ? 0 : 1)} BB</b>
            </div>
          {/if}
          <div class="nav">
            {#if at >= 0}<span class="num where">{at + 1} von {list.length}{more ? "+" : ""}</span>{/if}
            <IconButton label="Vorherige Hand (↑)" disabled={at <= 0} onclick={() => step(-1)}>
              <svg class="stroke" viewBox="0 0 24 24" width="16" height="16" aria-hidden="true"><path d="M6 15l6-6 6 6" /></svg>
            </IconButton>
            <IconButton label="Nächste Hand (↓)" disabled={at < 0 || at >= list.length - 1} onclick={() => step(1)}>
              <svg class="stroke" viewBox="0 0 24 24" width="16" height="16" aria-hidden="true"><path d="M6 9l6 6 6-6" /></svg>
            </IconButton>
          </div>
          {#if !wide}{@render verdictLine()}{/if}
        </header>
        <div class="work" class:wide bind:clientHeight={workH}>
          {#if wide}
            <div class="replay">
              <Replayer hand={selected} {marks} {jumpTo} onMark={select} onStep={follow} quietPlay solve={!current?.solver}>
                {#snippet aside()}{@render panel()}{/snippet}
              </Replayer>
            </div>
          {:else}
            <div class="replay" style:height="{replayH}px">
              <Replayer hand={selected} {marks} {jumpTo} onMark={select} onStep={follow} quietPlay solve={!current?.solver} />
            </div>
            <div class="analysis" bind:this={analysisEl}>{@render panel()}</div>
          {/if}
        </div>
      {:else if loaded && list.length}
        <EmptyState title="Wähle links eine Hand" text="Sie wird hier nachgespielt und bewertet." />
      {/if}
    </section>
  </div>
</div>

<style>
  .hands {
    position: relative;
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }
  .scrim {
    position: absolute;
    inset: 0;
    z-index: var(--z-popover);
    border: 0;
    background: var(--overlay);
    cursor: default;
    animation: fade var(--dur) var(--ease-standard) both;
  }
  .stroke path {
    fill: none;
    stroke: currentColor;
    stroke-width: 2.2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2);
    padding: var(--sp-2) var(--sp-6);
    border-bottom: 1px solid var(--line);
  }
  .count {
    min-width: 17px;
    height: 17px;
    margin-left: 6px;
    padding: 0 4px;
    border-radius: var(--r-pill);
    background: var(--text);
    color: var(--bg);
    font-size: var(--fs-xs);
    line-height: 17px;
    text-align: center;
  }
  .chip-name {
    display: inline-block;
    max-width: 260px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    vertical-align: bottom;
  }
  .summary {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    padding: var(--sp-2) var(--sp-6);
    border-bottom: 1px solid var(--line);
  }
  .summary > :global(:first-child) {
    flex: 1;
    min-width: 0;
  }

  .filters {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    z-index: var(--z-popover);
    max-height: 100%;
    overflow: auto;
    box-shadow: var(--shadow-3);
    animation: drop var(--dur) var(--ease-enter) both;
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    gap: var(--sp-4) var(--sp-8);
    padding: var(--sp-3) var(--sp-6) var(--sp-4);
    border-bottom: 1px solid var(--line);
    background: var(--surface);
    color: var(--text-2);
    font-size: var(--fs-md);
  }
  .group {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--sp-2);
  }
  .group.wide {
    flex-basis: 100%;
  }
  .title {
    color: var(--text);
    font-size: var(--fs-sm);
    font-weight: var(--fw-semibold);
  }
  .spot-groups {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-2) var(--sp-6);
  }
  .spot-group {
    display: flex;
    flex-direction: column;
    gap: var(--sp-1);
    font-size: var(--fs-sm);
  }
  .spot-group .quiet {
    color: var(--text-2);
  }
  .toggles {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-1);
  }
  .inline {
    display: flex;
    align-items: center;
    gap: 7px;
    cursor: pointer;
  }
  .note {
    max-width: 340px;
    margin: 0;
    color: var(--text-2);
    font-size: var(--fs-sm);
    line-height: var(--lh-base);
  }
  .sub {
    margin-top: var(--sp-1);
    color: var(--text-2);
    font-size: var(--fs-sm);
  }
  .inline input[type="checkbox"] {
    margin: 0;
    accent-color: var(--text);
  }
  .filters-head {
    position: sticky;
    top: calc(-1 * var(--sp-3));
    z-index: 1;
    flex-basis: 100%;
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    padding: var(--sp-3) 0 var(--sp-2);
    margin-top: calc(-1 * var(--sp-3));
    background: var(--surface);
  }
  .filters-head .title {
    flex: 1;
    font-size: var(--fs-base);
  }
  @keyframes drop {
    from {
      opacity: 0;
      transform: translateY(-6px);
    }
  }
  @keyframes fade {
    from {
      opacity: 0;
    }
  }
  .quiet {
    color: var(--text-3);
  }

  .body {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: clamp(300px, 24%, 420px) minmax(0, 1fr);
  }
  .list {
    overflow: auto;
    padding: 0 var(--sp-2) var(--sp-2);
    border-right: 1px solid var(--line);
  }
  .list-head {
    position: sticky;
    top: 0;
    z-index: 1;
    display: flex;
    justify-content: space-between;
    gap: var(--sp-2);
    padding: var(--sp-3) var(--sp-2) var(--sp-2);
    background: var(--bg);
    color: var(--text-2);
    font-size: var(--fs-sm);
  }
  /* Two lines: board and amount on top, verdict and time below; the verdict only shares its line with the short time. */
  .row {
    display: grid;
    grid-template-columns: 48px minmax(0, 1fr) auto;
    align-items: center;
    gap: 5px 10px;
    padding: var(--sp-2) 10px;
    border-radius: var(--r-md);
    box-shadow: inset 0 0 0 1px transparent;
    cursor: pointer;
    transition:
      background-color var(--dur-fast) var(--ease-standard),
      box-shadow var(--dur-fast) var(--ease-standard);
  }
  .row:hover {
    background: var(--surface);
  }
  .row.on {
    background: var(--surface-2);
    box-shadow: inset 0 0 0 1px var(--line-2);
  }
  .row:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: -2px;
  }
  .hole {
    display: flex;
    gap: 2px;
    flex-shrink: 0;
  }
  .who {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 3px;
    flex-shrink: 0;
    width: 48px;
  }
  .mid,
  .end {
    display: contents;
  }
  .row .who {
    grid-row: 1 / 3;
  }
  .line {
    grid-column: 2;
    grid-row: 1;
  }
  .verdict {
    grid-column: 2 / 4;
    grid-row: 2;
  }
  .row .net,
  .row .loss {
    grid-column: 3;
    grid-row: 1;
    justify-self: end;
  }
  .line {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    min-width: 0;
  }
  .posn {
    color: var(--text-2);
    font-size: var(--fs-xs);
    font-weight: var(--fw-semibold);
    line-height: 1;
  }
  .board {
    display: flex;
    min-width: 0;
    overflow: hidden;
  }
  .sd {
    flex-shrink: 0;
    fill: none;
    stroke: var(--sd);
    stroke-width: 2;
  }
  .verdict {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    color: var(--text-2);
    font-size: var(--fs-sm);
    white-space: nowrap;
  }
  .verdict b {
    flex-shrink: 0;
    color: var(--text);
    font-weight: var(--fw-semibold);
  }
  .verdict span {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .verdict i {
    flex-shrink: 0;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    border: 1.5px dashed var(--text-3);
  }
  .verdict i.wait {
    border-style: solid;
    border-color: var(--amber);
    border-right-color: transparent;
    animation: spin var(--dur-loop) var(--ease-spin) infinite;
  }
  .end > * {
    white-space: nowrap;
  }
  .net,
  .loss {
    font-size: var(--fs-base);
    font-weight: var(--fw-semibold);
  }
  .bb-unit {
    margin-left: 2px;
    font-size: var(--fs-xs);
  }
  .more {
    padding: var(--sp-2) var(--sp-1);
  }
  .brief {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    min-height: 48px;
    padding: var(--sp-2) var(--sp-4);
  }
  .brief p {
    flex: 1;
    min-width: 0;
    margin: 0;
    font-size: var(--fs-md);
  }

  /* The detail is the hand: its head, then the verdict and the replay side by side, or stacked when narrow. */
  .detail {
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    container: detail / inline-size;
  }
  .hand-head {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
    padding: var(--sp-3) var(--sp-6);
    border-bottom: 1px solid var(--line);
  }
  .ident {
    flex: 1;
    min-width: 0;
  }
  h2 {
    display: flex;
    align-items: baseline;
    gap: var(--sp-2);
    margin: 0;
    font-size: var(--fs-lg);
    font-weight: var(--fw-semibold);
  }
  .table-name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ident p {
    margin: 2px 0 0;
    overflow: hidden;
    color: var(--text-2);
    font-size: var(--fs-sm);
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .result {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    color: var(--text-3);
    font-size: var(--fs-xs);
    cursor: help;
  }
  /* The result of the hand is luck and skill together: shown, but quieter than the verdict. */
  .result b {
    font-size: var(--fs-lg);
    font-weight: var(--fw-medium);
  }
  .nav {
    display: flex;
    align-items: center;
    gap: 2px;
  }
  .where {
    margin-right: var(--sp-2);
    color: var(--text-3);
    font-size: var(--fs-sm);
  }
  .work {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: auto;
  }
  .replay {
    flex: 1;
    min-width: 0;
    min-height: 0;
    display: flex;
    padding: var(--sp-4) var(--sp-6);
  }
  .work.wide {
    overflow: hidden;
  }
  /* Narrow: the verdict first, then the replay; the detail scrolls. */
  .work:not(.wide) .replay {
    flex: none;
    padding: 0 var(--sp-4) var(--sp-4);
  }
  .analysis {
    padding: var(--sp-4);
  }
  /* Narrow: the hand in one line and the verdict right under it; cards are on the table below, stakes and date in the title's tooltip. */
  .hand-head.narrow {
    flex-wrap: wrap;
    row-gap: 2px;
    padding: var(--sp-2) var(--sp-4);
  }
  .narrow .hole,
  .narrow .where,
  .narrow .ident p {
    display: none;
  }
  .narrow .result {
    flex-direction: row;
    align-items: baseline;
    gap: var(--sp-2);
  }
  .narrow .result b {
    font-size: var(--fs-lg);
  }
  .narrow .brief {
    flex-basis: 100%;
    min-height: 0;
    padding: 0;
  }

  /* The list as a strip of hole cards: the hand's verdict is in its tooltip and in the detail. */
  .body.rail {
    grid-template-columns: 108px minmax(0, 1fr);
  }
  .rail .list {
    padding-inline: var(--sp-1);
  }
  .rail .list-head {
    flex-direction: column;
    align-items: center;
    gap: 0;
    padding-inline: 0;
    text-align: center;
  }
  .rail-what {
    font-size: var(--fs-xs);
  }
  .rail-verdict {
    display: flex;
    align-items: center;
    gap: 3px;
    white-space: nowrap;
    color: var(--text-2);
    font-size: var(--fs-xs);
  }
  .rail .list-head span:last-child:not(.rail-what),
  .rail .mid {
    display: none;
  }
  .rail .row {
    display: flex;
    flex-direction: column;
    gap: var(--sp-1);
    padding: var(--sp-2) var(--sp-1);
  }
  .rail .row .who {
    width: auto;
  }
  .rail .end {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .rail .net,
  .rail .loss {
    font-size: var(--fs-sm);
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  @container (max-width: 980px) {
    .chips,
    .filters,
    .summary {
      padding-inline: var(--sp-4);
    }
  }
</style>
