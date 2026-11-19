<script lang="ts" module>
  import type { Selection } from "./select";

  /** Kept while the app runs, so coming back from the hand list shows the same situation. */
  let remembered: Selection | null = null;
</script>

<script lang="ts">
  import { api } from "../api";
  import type { Filter } from "../types";
  import { int } from "../app/format";
  import RangeGrid, { type GridMode } from "./RangeGrid.svelte";
  import Button from "../ui/Button.svelte";
  import {
    FEW,
    HISTORY,
    MOVE,
    POSITION_NAME,
    TIGHT,
    WIDE,
    against,
    className,
    depthLabel,
    headline,
    lineText,
    pct,
  } from "./labels";
  import {
    BANDS,
    HISTORIES,
    POSITIONS,
    STACK_CAP,
    bandOf,
    facet,
    hasOpponent,
    fromPreset,
    initial,
    settle,
    stackHistogram,
    toFilter,
    total,
    type Band,
    type Behind,
    type Size,
  } from "./select";
  import { MOVES, type RangeChart, type RangeFilter, type RangeGroup } from "./types";
  import type { RangePreset } from "../session/types";

  let {
    filter,
    refresh,
    onHands,
    preset = null,
    onPresetUsed,
  }: {
    filter: Filter;
    refresh: number;
    onHands: (range: RangeFilter, label: string) => void;
    /** A situation to open on, e.g. a pattern of the session review; every depth. */
    preset?: RangePreset | null;
    onPresetUsed?: () => void;
  } = $props();

  let groups = $state<RangeGroup[] | null>(null);
  let sel = $state<Selection | null>(remembered);
  let chart = $state<RangeChart | null>(null);
  let loading = $state(false);
  let error = $state<string | null>(null);
  /** Narrow windows show two grids and switch the second; wide ones show all three. */
  let second = $state<"model" | "diff">("model");
  let outline = $state(true);

  $effect(() => {
    void refresh;
    const f = $state.snapshot(filter) as Filter;
    let live = true;
    api
      .rangeGroups(f)
      .then((g) => {
        if (!live) return;
        groups = g;
        error = null;
        if (preset) {
          sel = fromPreset(g, preset);
          onPresetUsed?.();
        }
        // A new period or tournament keeps the situation while it still has hands.
        if (!sel || total(g, sel) === 0) sel = initial(g);
      })
      .catch((e) => live && (error = String(e)));
    return () => (live = false);
  });

  $effect(() => {
    remembered = sel ? $state.snapshot(sel) : null;
  });

  const range = $derived(sel ? toFilter(sel) : null);

  $effect(() => {
    if (!range) return;
    const f = $state.snapshot(filter) as Filter;
    const r = $state.snapshot(range) as RangeFilter;
    let live = true;
    loading = true;
    // Dragging the depth slider sends many changes; ask once it rests.
    const t = setTimeout(() => {
      api
        .rangeChart(f, r)
        .then((c) => {
          if (!live) return;
          chart = c;
          error = null;
        })
        .catch((e) => live && (error = String(e)))
        .finally(() => live && (loading = false));
    }, 90);
    return () => {
      live = false;
      clearTimeout(t);
    };
  });

  /** The situation the page opens with; "Zurücksetzen" returns to it. */
  const start = $derived(groups ? initial(groups) : null);
  const changed = $derived(!!sel && !!start && JSON.stringify(sel) !== JSON.stringify(start));

  function update(patch: Partial<Selection>, resettle = false) {
    if (!sel || !groups) return;
    const next = { ...sel, ...patch };
    sel = resettle ? settle(groups, next, "depth" in patch) : next;
  }

  const counts = $derived.by(() => {
    if (!groups || !sel) return null;
    const s = sel;
    return {
      history: facet(groups, s, "history", (g) => g.history),
      position: facet(groups, s, "position", (g) => g.position),
      vs: facet(groups, s, "vs", (g) => g.vs_position),
      band: facet(groups, s, "depth", (g) => bandOf(g.stack)),
      size: facet(groups, s, "size", (g) => (g.players >= 7 ? "full" : g.players >= 5 ? "mid" : g.players >= 3 ? "short" : "hu")),
      behind: facet(groups, s, "behind", (g) => g.behind),
      all: total(groups, s),
    };
  });
  const seats = $derived(POSITIONS.filter((p) => counts?.position.get(p)));
  const opponents = $derived(POSITIONS.filter((p) => counts?.vs.get(p)));
  const histogram = $derived(groups && sel ? stackHistogram(groups, sel) : []);
  const histMax = $derived(Math.max(1, ...histogram));
  const freeRange = $derived(sel?.depth.kind === "free" ? sel.depth : null);

  const SIZE_LABEL: [Size, string, string][] = [
    ["all", "Alle", "Jede Tischgröße"],
    ["full", "7–9", "Voller Tisch"],
    ["mid", "5–6", "Short-handed"],
    ["short", "3–4", "Wenige Spieler, oft am Final Table"],
    ["hu", "2", "Heads-up"],
  ];
  const BEHIND_LABEL: [Behind, string, string][] = [
    ["all", "Egal", "Alle Hände"],
    [0, "unter 10", "Hinter dir sitzt ein kurzer Stack, der jederzeit All-in gehen kann"],
    [1, "10–25", "Der kürzeste Stack hinter dir hat 10 bis 25 BB"],
    [2, "ab 25", "Hinter dir sitzen nur tiefe Stacks"],
  ];
  const BAND_LABEL = ["unter 10", "10–20", "20–40", "ab 40"];

  function setBand(band: Band) {
    update({ depth: { kind: "band", band } }, true);
  }

  function setFree() {
    if (!sel) return;
    const [lo, hi] = sel.depth.kind === "band" ? BANDS[sel.depth.band] : [0, null];
    update({ depth: { kind: "free", min: lo, max: hi ?? STACK_CAP } });
  }

  function setFreeBound(which: "min" | "max", value: number) {
    if (!sel || sel.depth.kind !== "free") return;
    const d = { ...sel.depth };
    if (which === "min") d.min = Math.min(value, d.max - 1);
    else d.max = Math.max(value, d.min + 1);
    update({ depth: d });
  }

  /** What the selection is, for the chip on the hand list. */
  function situation(): string {
    if (!sel) return "";
    const vs = hasOpponent(sel.history) && sel.vs ? ` ${against(sel.vs)}` : "";
    return `${sel.position ?? "alle Positionen"} · ${HISTORY[sel.history].label}${vs} · ${depthLabel(sel.depth)}`;
  }

  function openCell(i: number) {
    if (!range || !chart) return;
    const c = chart.counts.cells[i];
    if (!c || c[0] + c[1] + c[2] + c[3] === 0) return;
    onHands({ ...range, class: i }, `${className(i)} · ${situation()}`);
  }

  const totals = $derived.by(() => {
    if (!chart) return null;
    const sum = [0, 0, 0, 0];
    for (const c of chart.counts.cells) for (let k = 0; k < 4; k++) sum[k] += c[k];
    const n = chart.counts.total;
    return { n, play: n ? (100 * (n - sum[0])) / n : 0, sum };
  });
  const answer = $derived(sel && totals ? headline(sel, totals.play, chart?.comparison ?? null) : "");
  const model = $derived(chart?.model ?? null);
  const thin = $derived(totals ? totals.n < 40 : false);
  /** While PokerStars runs only the hero's own chart shows; the comparison waits. */
  const locked = $derived(chart?.locked ?? false);
  const views = (wide: boolean): GridMode[] => (locked ? ["you"] : wide ? ["you", "model", "diff"] : ["you", second]);
</script>

{#if error}
  <div class="state"><p>Die Ranges ließen sich nicht laden: {error}</p></div>
{:else if !groups}
  <div class="state"><p class="faint">Lade deine Preflop-Entscheidungen …</p></div>
{:else if groups.length === 0 || !sel || !counts}
  <div class="state empty">
    <h2>Noch keine Preflop-Entscheidungen hier</h2>
    <p>
      Sobald Hände mit deinen Karten importiert sind, zeigt diese Seite, welche Hände du von jeder Position spielst – und was das Modell dort spielen würde.
      Prüfe den Zeitraum oder das gewählte Turnier oben.
    </p>
  </div>
{:else}
  <div class="page ranges-page">
    <section class="answer" aria-live="polite">
      <p class="eyebrow">
        <span>{situation()}</span>
        {#if totals}<span class="num">{int(totals.n)} {totals.n === 1 ? "Hand" : "Hände"}</span>{/if}
        {#if loading}<span class="busy">rechnet …</span>{/if}
      </p>
      <h2>{answer}</h2>
      {#if chart?.comparison}
        {@const cmp = chart.comparison}
        <div class="gaps">
          {#if cmp.missing.length}
            <p><i class="dot" style="background:{TIGHT}"></i>Am meisten fehlen dir: <b>{cmp.missing.join(", ")}</b>.</p>
          {/if}
          {#if cmp.extra.length}
            <p><i class="dot" style="background:{WIDE}"></i>Zu oft spielst du: <b>{cmp.extra.join(", ")}</b>.</p>
          {/if}
          {#if cmp.missing.length || cmp.extra.length}
            <p class="key">s = beide Karten in einer Farbe, o = verschiedene Farben, z. B. Q9o = Dame und Neun ungleichfarbig.</p>
          {/if}
          {#if !cmp.missing.length && !cmp.extra.length}
            <p><i class="dot" style="background:var(--pos)"></i>Keine Hand weicht deutlich ab – deine Range passt hier zum Modell.</p>
          {/if}
        </div>
      {:else if chart?.model_missing}
        <p class="gaps-off">{chart.model_missing}</p>
      {/if}
      {#if thin}
        <p class="thin">Erst {totals?.n} Hände in dieser Auswahl – die Prozente schwanken noch stark. Ein breiterer Filter (z. B. eine ganze Stacktiefe) hilft.</p>
      {/if}
    </section>

    <div class="layout">
      <aside class="situation" aria-label="Situation wählen">
        <div class="aside-head">
          <Button
            size="sm"
            variant="quiet"
            disabled={!changed}
            title={changed ? "Alle Filter auf die Ausgangs-Auswahl zurück" : "Das ist schon die Ausgangs-Auswahl"}
            onclick={() => start && (sel = $state.snapshot(start) as Selection)}>Filter zurücksetzen</Button
          >
        </div>
        <div class="pick full">
          <span class="pick-label">Was vor dir passiert ist</span>
          <div class="tiles history">
            {#each HISTORIES as h (h)}
              {@const n = counts.history.get(h) ?? 0}
              <button class="tile stack" class:on={sel.history === h} disabled={!n} title={HISTORY[h].hint} onclick={() => update({ history: h }, true)}>
                <span>{HISTORY[h].label}</span><small class="num">{int(n)}</small>
              </button>
            {/each}
          </div>
        </div>

        <div class="pick full">
          <span class="pick-label">Deine Position</span>
          <div class="seats" role="group" aria-label="Position">
            {#each seats as p (p)}
              {@const n = counts.position.get(p) ?? 0}
              {@const max = Math.max(...counts.position.values())}
              <button class="seat" class:on={sel.position === p} title={POSITION_NAME[p]} onclick={() => update({ position: p }, true)}>
                <b>{p}</b>
                <small class="num">{int(n)}</small>
                <i class="meter"><i style="width:{(100 * n) / max}%"></i></i>
              </button>
            {/each}
          </div>
        </div>

        {#if hasOpponent(sel.history)}
          <div class="pick full">
            <span class="pick-label">{sel.history === "limped" ? "Erster Limp von" : sel.history === "vs_shove" ? "All-in von" : "Raise von"}</span>
            <div class="seg wrap-seg" role="group" aria-label="Gegner-Position">
              <button class:on={sel.vs === null} onclick={() => update({ vs: null })}>Alle</button>
              {#each opponents as p (p)}
                <button class:on={sel.vs === p} title={POSITION_NAME[p]} onclick={() => update({ vs: p })}>
                  {p} <small class="num">{counts.vs.get(p)}</small>
                </button>
              {/each}
            </div>
          </div>
        {/if}

        <div class="pick full">
          <span class="pick-label" title="Effektiver Stack: der kleinere von deinem Stack und dem größten Stack eines Gegners, in Big Blinds">Dein Stack in Big Blinds</span>
          <div class="tiles bands">
            {#each BAND_LABEL as b, k (b)}
              <button class="tile" class:on={sel.depth.kind === "band" && sel.depth.band === k} disabled={!counts.band.get(k as Band)} onclick={() => setBand(k as Band)}>
                <span>{b}</span><small class="num">{int(counts.band.get(k as Band) ?? 0)}</small>
              </button>
            {/each}
            <button class="tile" class:on={sel.depth.kind === "free"} title="Tiefe frei per Schieber wählen" onclick={setFree}>
              <span>Frei</span><small aria-hidden="true">⇆</small>
            </button>
          </div>
          {#if freeRange}
            <div class="free">
              <div class="hist" aria-hidden="true">
                {#each histogram as v, k (k)}
                  <i class:in={k >= freeRange.min && (freeRange.max >= STACK_CAP || k < freeRange.max)} style="height:{(100 * v) / histMax}%"></i>
                {/each}
              </div>
              <div class="sliders">
                <input type="range" min="0" max={STACK_CAP} step="1" value={freeRange.min} aria-label="Mindestens BB" oninput={(e) => setFreeBound("min", Number(e.currentTarget.value))} />
                <input type="range" min="0" max={STACK_CAP} step="1" value={freeRange.max} aria-label="Unter BB" oninput={(e) => setFreeBound("max", Number(e.currentTarget.value))} />
              </div>
              <p class="num free-text">{depthLabel(sel.depth)}</p>
            </div>
          {/if}
        </div>

        <div class="pick">
          <span class="pick-label">Spieler am Tisch</span>
          <div class="seg" role="group" aria-label="Tischgröße">
            {#each SIZE_LABEL as [id, label, hint] (id)}
              <button class:on={sel.size === id} title={hint} disabled={id !== "all" && !counts.size.get(id)} onclick={() => update({ size: id })}>{label}</button>
            {/each}
          </div>
        </div>

        <div class="pick">
          <span class="pick-label" title="Der kürzeste Stack unter den Spielern, die nach dir noch handeln">Kürzester Stack hinter dir (BB)</span>
          <div class="seg" role="group" aria-label="Kürzester Stack hinter dir">
            {#each BEHIND_LABEL as [id, label, hint] (id)}
              <button class:on={sel.behind === id} title={hint} disabled={id !== "all" && !counts.behind.get(id)} onclick={() => update({ behind: id })}>{label}</button>
            {/each}
          </div>
        </div>
      </aside>

      <section class="charts" aria-label="Range-Charts">
        <div class="toolbar">
          {#if !locked}
            <div class="seg narrow-only" role="group" aria-label="Zweites Chart">
              <button class:on={second === "model"} onclick={() => (second = "model")}>Modell</button>
              <button class:on={second === "diff"} onclick={() => (second = "diff")}>Differenz</button>
            </div>
            <label class="outline-toggle" title="Zeichnet die Hände, die das Modell meistens spielt, als Linie über dein Chart">
              <input type="checkbox" bind:checked={outline} disabled={!model} /> Modell-Umriss über deinem Chart
            </label>
          {/if}
          <div class="legend">
            {#each MOVES as m (m)}
              <span><i style="background:{MOVE[m].color}" class:fold={m === "fold"}></i>{MOVE[m].label}</span>
            {/each}
            <span title="Weniger als {FEW} Hände: Zufall spielt noch mit"><i class="few"></i>wenige Hände</span>
          </div>
        </div>

        {#if chart}
          {#each [false, true] as wide (wide)}
            <div class="grids" class:wide-set={wide} class:narrow-set={!wide}>
              {#each views(wide) as mode (mode)}
                <figure class="grid-card">
                  <figcaption>
                    {#if mode === "you"}
                      <b>Du</b><span class="num">{int(chart.counts.total)} Hände · {pct(totals?.play ?? 0)} gespielt</span>
                    {:else if mode === "model"}
                      <b>{model?.source === "push_fold" ? "Push/Fold" : "Modell"}</b>
                      <span class="num">{model ? `${model.players} Spieler · ${model.stack_bb} BB · ${pct(chart.comparison?.model_pct ?? 0)} gespielt` : "nicht verfügbar"}</span>
                    {:else}
                      <b>Differenz</b>
                      <span class="diff-key"><i style="background:{WIDE}"></i>zu weit <i style="background:{TIGHT}"></i>zu eng</span>
                    {/if}
                  </figcaption>
                  {#if mode !== "you" && !model}
                    <div class="no-model">
                      <p>{chart.model_missing ?? "Für diese Auswahl gibt es kein Modell."}</p>
                    </div>
                  {:else}
                    <RangeGrid {mode} counts={chart.counts} {model} outline={mode === "you" && outline} onCell={openCell} />
                  {/if}
                </figure>
              {/each}
            </div>
          {/each}

          {#if model}
            <details class="notes">
              <summary>So nah ist das Modell an deinen Händen</summary>
              <ul>
                {#each model.notes as n (n)}<li>{n}</li>{/each}
                {#if chart.counts.line && hasOpponent(sel.history)}
                  <li>Gerechnet für: {lineText(chart.counts.line[0])}</li>
                {/if}
                <li>
                  {model.source === "push_fold"
                    ? "Gleichgewicht in Chips, ohne ICM: bei Bubble oder Final Table spielt man oft enger."
                    : "Das Preflop-Modell ist eine Näherung von Ringside, kein exaktes GTO."}
                </li>
              </ul>
            </details>
          {/if}
          <p class="howto">Fahr über eine Hand für Details. Klick zeigt deine Hände damit – dort kannst du jede Hand analysieren.</p>
        {/if}
      </section>
    </div>
  </div>
{/if}

<style>
  .ranges-page {
    display: flex;
    flex-direction: column;
    gap: 18px;
  }
  .state {
    padding: 48px 24px;
    max-width: 640px;
  }
  .state h2 {
    margin: 0 0 8px;
    font-family: var(--font-num);
    font-size: 20px;
    font-weight: 600;
  }
  .state p {
    margin: 0;
    color: var(--text-2);
  }

  /* The answer first: one sentence, then the few hands that matter. */
  .answer {
    display: grid;
    gap: 8px;
    padding-bottom: 16px;
    border-bottom: 1px solid var(--line);
  }
  .eyebrow {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 14px;
    margin: 0;
    font-size: 12.5px;
    color: var(--text-3);
  }
  .busy {
    color: var(--text-2);
  }
  .answer h2 {
    margin: 0;
    max-width: 52ch;
    font-family: var(--font-num);
    font-size: clamp(21px, 2.1vw, 30px);
    font-weight: 500;
    line-height: 1.22;
    letter-spacing: -0.005em;
    text-wrap: balance;
  }
  .gaps {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 26px;
  }
  .gaps p,
  .gaps-off,
  .thin {
    margin: 0;
    font-size: 14px;
    color: var(--text-2);
  }
  .gaps p.key {
    flex-basis: 100%;
    color: var(--text-3);
    font-size: var(--fs-sm);
  }
  .gaps b {
    color: var(--text);
    font-family: var(--font-num);
    font-weight: 600;
    letter-spacing: 0.01em;
  }
  .thin {
    font-size: 12.5px;
    color: var(--amber);
  }
  .dot {
    display: inline-block;
    width: 9px;
    height: 9px;
    margin-right: 8px;
    border-radius: 50%;
  }

  .layout {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 18px;
  }
  @container (min-width: 1180px) {
    .layout {
      grid-template-columns: 300px minmax(0, 1fr);
      align-items: start;
    }
    .situation {
      position: sticky;
      top: 16px;
      grid-template-columns: minmax(0, 1fr) !important;
    }
  }

  .situation {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 14px 22px;
    padding: 14px 16px;
    border: 1px solid var(--line);
    border-radius: var(--r);
    background: var(--surface);
  }
  .pick {
    display: grid;
    gap: 7px;
    align-content: start;
    min-width: 0;
  }
  .pick.full {
    grid-column: 1 / -1;
  }
  .aside-head {
    grid-column: 1 / -1;
    display: flex;
    justify-content: flex-end;
    margin: calc(-1 * var(--sp-1)) 0 calc(-1 * var(--sp-2));
  }
  .pick-label {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-2);
  }
  .tiles {
    display: grid;
    gap: 5px;
  }
  .history {
    grid-template-columns: repeat(auto-fit, minmax(100px, 1fr));
  }
  .tile.stack {
    flex-direction: column;
    gap: 1px;
    padding: 6px 10px;
  }
  .bands {
    grid-template-columns: repeat(auto-fit, minmax(84px, 1fr));
  }
  @container (min-width: 1180px) {
    .history,
    .bands {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
    .tile.stack {
      flex-direction: row;
      padding: 7px 10px;
    }
  }
  .tile {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 8px;
    min-width: 0;
    padding: 7px 10px;
    border: 1px solid var(--line-2);
    border-radius: var(--r-sm);
    background: var(--surface-2);
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }
  .tile span {
    font-size: 13px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .tile small {
    font-size: 11.5px;
    color: var(--text-3);
  }
  .tile:hover:not(:disabled) {
    background: var(--surface-3);
  }
  .tile.on {
    border-color: var(--text);
    background: var(--surface-3);
    box-shadow: inset 0 0 0 1px var(--text);
  }
  .tile.on small {
    color: var(--text-2);
  }
  .tile:disabled {
    opacity: 0.35;
    cursor: default;
  }

  /* Positions in the order they act, like a row of seats. */
  .seats {
    display: grid;
    grid-auto-flow: column;
    grid-auto-columns: minmax(0, 1fr);
    gap: 4px;
  }
  @container (min-width: 1180px) {
    .seats {
      grid-auto-flow: row;
      grid-template-columns: repeat(3, minmax(0, 1fr));
    }
  }
  .seat {
    display: grid;
    grid-template-columns: auto auto;
    justify-content: space-between;
    align-items: baseline;
    gap: 5px 6px;
    padding: 6px 8px 7px;
    border: 1px solid var(--line-2);
    border-radius: var(--r-sm);
    background: var(--surface-2);
    cursor: pointer;
    min-width: 0;
  }
  .seat b {
    font-family: var(--font-num);
    font-size: 13.5px;
    font-weight: 600;
  }
  .seat small {
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .meter {
    grid-column: 1 / -1;
    display: block;
    width: 100%;
    height: 3px;
    border-radius: 2px;
    background: var(--surface-3);
    overflow: hidden;
  }
  .meter i {
    display: block;
    height: 100%;
    background: var(--text-3);
  }
  .seat:hover {
    background: var(--surface-3);
  }
  .seat.on {
    border-color: var(--text);
    box-shadow: inset 0 0 0 1px var(--text);
    background: var(--surface-3);
  }
  .seat.on .meter i {
    background: var(--text);
  }

  .seg button small {
    margin-left: 3px;
    color: var(--text-3);
    font-size: var(--fs-xs);
  }
  .wrap-seg {
    flex-wrap: wrap;
  }
  .situation .seg button {
    white-space: nowrap;
  }
  .seg button:disabled {
    opacity: 0.35;
  }

  .free {
    display: grid;
    gap: 4px;
  }
  .hist {
    display: flex;
    align-items: flex-end;
    gap: 1px;
    height: 34px;
  }
  .hist i {
    flex: 1;
    min-height: 1px;
    border-radius: 1px 1px 0 0;
    background: var(--surface-3);
  }
  .hist i.in {
    background: var(--text-2);
  }
  .sliders {
    position: relative;
    height: 18px;
  }
  .sliders input {
    position: absolute;
    inset: 0;
    width: 100%;
    margin: 0;
    background: none;
    pointer-events: none;
    appearance: none;
  }
  .sliders input::-webkit-slider-thumb {
    pointer-events: auto;
    appearance: none;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: var(--text);
    border: 2px solid var(--bg);
    cursor: pointer;
  }
  .sliders input::-moz-range-thumb {
    pointer-events: auto;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--text);
    border: 2px solid var(--bg);
    cursor: pointer;
  }
  .free-text {
    margin: 0;
    font-size: 12.5px;
    color: var(--text);
  }

  .charts {
    display: grid;
    gap: 12px;
    min-width: 0;
  }
  .toolbar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 18px;
  }
  .outline-toggle {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    font-size: 13px;
    color: var(--text-2);
    cursor: pointer;
  }
  .legend {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 14px;
    margin-left: auto;
    font-size: 12px;
    color: var(--text-2);
  }
  .legend span {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .legend i,
  .diff-key i {
    width: 10px;
    height: 10px;
    border-radius: 2px;
  }
  .legend i.fold {
    box-shadow: inset 0 0 0 1px var(--line-2);
  }
  .legend i.few {
    background: color-mix(in srgb, var(--amber) 80%, var(--surface-2));
    opacity: 0.5;
  }

  .grids {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 16px;
  }
  .wide-set {
    display: none;
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }
  @container (min-width: 1500px) {
    .wide-set {
      display: grid;
    }
    .narrow-set,
    .narrow-only {
      display: none;
    }
  }
  .grid-card {
    margin: 0;
    display: grid;
    gap: 8px;
    align-content: start;
    min-width: 0;
  }
  figcaption {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 10px;
    font-size: 12.5px;
    color: var(--text-2);
  }
  figcaption b {
    font-family: var(--font-num);
    font-size: 15px;
    font-weight: 600;
    color: var(--text);
  }
  .diff-key {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .diff-key i:last-of-type {
    margin-left: 8px;
  }
  .no-model {
    display: grid;
    place-items: center;
    aspect-ratio: 1;
    padding: 24px;
    border: 1px dashed var(--line-2);
    border-radius: var(--r);
    text-align: center;
  }
  .no-model p {
    margin: 0;
    max-width: 30ch;
    color: var(--text-2);
  }
  .notes {
    font-size: 12.5px;
    color: var(--text-2);
  }
  .notes summary {
    cursor: pointer;
    color: var(--text-2);
  }
  .notes ul {
    margin: 6px 0 0;
    padding-left: 18px;
    display: grid;
    gap: 2px;
  }
  .howto {
    margin: 0;
    font-size: 12px;
    color: var(--text-3);
  }
</style>
