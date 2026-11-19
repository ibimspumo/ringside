<script lang="ts">
  import SiteMark from "../ui/SiteMark.svelte";
  import { severalSites } from "./sites.svelte";
  import { api } from "../api";
  import { bountyText, fmt, stat } from "../stats";
  import type { Filter, GroupView, Reports } from "../types";
  import Button from "../ui/Button.svelte";
  import EmptyState from "../ui/EmptyState.svelte";
  import SortHeader from "../ui/SortHeader.svelte";
  import Term from "../ui/Term.svelte";
  import { dayShort, floating, int, secondsBetween, sign, signed, time, type Anchor } from "./format";
  import { compact, duration } from "../ui/format";
  import { POSITION_HELP, POSITION_PLAIN } from "./positions";

  let { filter, refresh }: { filter: Filter; refresh: number } = $props();

  let data = $state<Reports | null>(null);
  let failed = $state(false);
  let retry = $state(0);
  let mode = $state<"bb100" | "played">("bb100");
  let hover = $state<{ g: Cell; anchor: Anchor } | null>(null);

  $effect(() => {
    void refresh;
    void retry;
    const f = { ...filter };
    let live = true;
    failed = false;
    api.reports(f).then(
      (r) => live && (data = r),
      () => live && (failed = true),
    );
    return () => (live = false);
  });

  const RANKS = "AKQJT98765432";
  /** Fewer hands than this say nothing about a win rate, so those cells stay blank in BB/100. */
  const MIN_HANDS = 5;
  type Cell = { key: string; hands: number; net_bb: number; bb100: number | null; vpip: number | null };

  const hands = (g: GroupView) => stat(g.stats, "hands")?.value ?? 0;
  const per100 = (g: GroupView) => (hands(g) > 0 ? (g.net_bb / hands(g)) * 100 : null);

  const grid = $derived.by(() => {
    const byKey = new Map((data?.starting_hands ?? []).map((g) => [g.key, g]));
    const cells: Cell[] = [];
    for (let r = 0; r < 13; r++)
      for (let c = 0; c < 13; c++) {
        const hi = RANKS[Math.min(r, c)];
        const lo = RANKS[Math.max(r, c)];
        const key = r === c ? hi + lo : r < c ? `${hi}${lo}s` : `${hi}${lo}o`;
        const g = byKey.get(key);
        cells.push({
          key,
          hands: g ? hands(g) : 0,
          net_bb: g?.net_bb ?? 0,
          bb100: g ? per100(g) : null,
          vpip: g ? (stat(g.stats, "vpip")?.value ?? null) : null,
        });
      }
    // Scale colour to the 90th percentile so one lucky hand does not wash out the rest.
    const mags = cells
      .filter((c) => c.bb100 !== null && c.hands >= MIN_HANDS)
      .map((c) => Math.abs(c.bb100!))
      .sort((a, b) => a - b);
    const scale = mags.length ? Math.max(mags[Math.floor(mags.length * 0.9)] ?? 1, 1) : 1;
    return { cells, scale };
  });

  /** BB/100 is shown only once a hand has enough samples; "played" works from the first deal. */
  const blank = (c: Cell) => c.hands === 0 || (mode === "bb100" && c.hands < MIN_HANDS);

  function fill(c: Cell): string {
    if (blank(c)) return "transparent";
    if (mode === "played") {
      const p = Math.min(100, Math.max(0, c.vpip ?? 0)) / 100;
      return `color-mix(in srgb, var(--sd) ${Math.round(10 + p * 60)}%, transparent)`;
    }
    const v = c.bb100 ?? 0;
    const t = Math.min(1, Math.abs(v) / grid.scale);
    const tone = v >= 0 ? "var(--pos)" : "var(--neg)";
    return `color-mix(in srgb, ${tone} ${Math.round(8 + t * 72)}%, var(--surface-2))`;
  }

  function ink(c: Cell): string {
    if (blank(c)) return "var(--text-3)";
    if (mode === "played") return "var(--text)";
    return Math.abs(c.bb100 ?? 0) / grid.scale > 0.55 ? "#121418" : "var(--text)";
  }

  function value(c: Cell): string {
    if (blank(c)) return "";
    if (mode === "played") return c.vpip === null ? "–" : `${Math.round(c.vpip)}`;
    const v = c.bb100 ?? 0;
    return Math.abs(v) >= 1000 ? `${v > 0 ? "+" : "−"}${compact(Math.abs(v))}` : signed(v, 0);
  }

  function show(e: MouseEvent, g: Cell) {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    hover = { g, anchor: { left: r.left, top: r.top, right: r.right, bottom: r.bottom } };
  }

  const totals = $derived.by(() => {
    const pos = data?.positions ?? [];
    return { hands: pos.reduce((a, g) => a + hands(g), 0), net: pos.reduce((a, g) => a + g.net_bb, 0) };
  });
  const several = $derived(severalSites());
</script>

<div class="page">
  {#if failed && !data}
    <EmptyState page tone="error" title="Die Statistik konnte nicht geladen werden" text="Deine Hände sind sicher gespeichert. Versuch es gleich noch einmal.">
      {#snippet action()}<Button variant="primary" onclick={() => retry++}>Erneut laden</Button>{/snippet}
    </EmptyState>
  {:else if !data}
    <EmptyState page tone="loading" title="Statistik wird geladen …" />
  {:else}
    <div class="grid">
      <section class="panel matrix-panel">
        <div class="panel-head">
          <h2 class="panel-title">Starthände</h2>
          <div class="seg" role="group" aria-label="Wert in den Feldern">
            <button class:on={mode === "bb100"} onclick={() => (mode = "bb100")}>BB/100</button>
            <button class:on={mode === "played"} onclick={() => (mode = "played")}>Gespielt %</button>
          </div>
        </div>
        {#if data.starting_hands.length}
          <div class="matrix-body">
          <div class="matrix-wrap">
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div class="matrix" class:played={mode === "played"} onmouseleave={() => (hover = null)}>
              {#each grid.cells as c (c.key)}
                <div
                  class="cell"
                  class:empty={blank(c)}
                  style="background:{fill(c)}; color:{ink(c)}"
                  onmouseenter={(e) => show(e, c)}
                >
                  <span class="k">{c.key}</span>
                  <span class="v num">{value(c)}</span>
                </div>
              {/each}
              {#if hover}
                {@const g = hover.g}
                <div class="tip" use:floating={{ anchor: hover.anchor }}>
                  <header><b>{g.key}</b> · <span class="num">{int(g.hands)}</span> Hände</header>
                  {#if g.hands}
                    <div class="row"><span>Ergebnis</span><b class={sign(g.net_bb)}>{signed(g.net_bb)} BB</b></div>
                    <div class="row"><span>BB/100</span><b class={sign(g.bb100)}>{signed(g.bb100 ?? 0)}</b></div>
                    <div class="row"><span>Gespielt</span><b>{g.vpip === null ? "–" : `${Math.round(g.vpip)} %`}</b></div>
                  {/if}
                </div>
              {/if}
            </div>
          </div>
          <div class="key">
            {#if mode === "bb100"}
              <div class="scale">
                <span class="ramp bb"></span>
                <span class="ticks num"><span>−{int(grid.scale)} und weniger</span><span>0</span><span>+{int(grid.scale)} und mehr</span></span>
              </div>
              <p class="note">
                Gewinn je 100 Hände mit dieser Starthand. Rot verliert, Grün gewinnt; je kräftiger, desto deutlicher.
                Felder mit weniger als {MIN_HANDS} Händen bleiben leer.
              </p>
            {:else}
              <div class="scale">
                <span class="ramp played"></span>
                <span class="ticks num"><span>0 %</span><span>50 %</span><span>100 %</span></span>
              </div>
              <p class="note">Wie oft du die Hand freiwillig gespielt hast. Je kräftiger das Blau, desto öfter.</p>
            {/if}
            <p class="note">Paare auf der Diagonale. Oben rechts gleiche Farbe (AKs = Ass und König in einer Farbe), unten links verschiedene Farben (AKo).</p>
          </div>
          </div>
        {:else}
          <EmptyState compact title="Noch keine eigenen Hände mit bekannten Karten" text="Wähle oben einen längeren Zeitraum." />
        {/if}
      </section>

      <div class="side">
        <section class="panel">
          <div class="panel-head"><h2 class="panel-title">Nach Position</h2></div>
          {#if data.positions.length}
            <div class="scroll">
              <table class="table">
                <thead>
                  <tr>
                    <th><SortHeader>Position</SortHeader></th>
                    <th><SortHeader>Hände</SortHeader></th>
                    <th><SortHeader sub="BB">Ergebnis</SortHeader></th>
                    <th><SortHeader sub="BB/100">Je 100 Hände</SortHeader></th>
                    <th><SortHeader sub="VPIP %" term title="Wie oft du freiwillig mitgespielt hast">Gespielt</SortHeader></th>
                    <th><SortHeader sub="PFR %" term title="Wie oft du vor dem Flop erhöht hast">Erhöht</SortHeader></th>
                    <th><SortHeader sub="Steal %" term title="Wie oft du aus Cutoff, Button und Small Blind die Blinds angegriffen hast">Blinds geklaut</SortHeader></th>
                  </tr>
                </thead>
                <tbody>
                  {#each data.positions as g (g.key)}
                    {@const v = per100(g)}
                    <tr>
                      <td class="pos-name">
                        {#if POSITION_HELP[g.key]}<Term tip={POSITION_HELP[g.key]}>{g.key}</Term>{:else}{g.key}{/if}
                        {#if POSITION_PLAIN[g.key]}<small>{POSITION_PLAIN[g.key]}</small>{/if}
                      </td>
                      <td>{int(hands(g))}</td>
                      <td class={sign(g.net_bb)}>{signed(g.net_bb)}</td>
                      <td class={sign(v)}>{v === null ? "–" : signed(v)}</td>
                      <td>{fmt(stat(g.stats, "vpip"))}</td>
                      <td>{fmt(stat(g.stats, "pfr"))}</td>
                      <td class:faint={stat(g.stats, "steal")?.value == null}>{fmt(stat(g.stats, "steal"))}</td>
                    </tr>
                  {/each}
                </tbody>
                <tfoot>
                  <tr>
                    <td class="pos-name">Gesamt</td>
                    <td>{int(totals.hands)}</td>
                    <td class={sign(totals.net)}>{signed(totals.net)}</td>
                    <td class={sign(totals.net)}>{totals.hands ? signed((totals.net / totals.hands) * 100) : "–"}</td>
                    <td colspan="3"></td>
                  </tr>
                </tfoot>
              </table>
            </div>
            <p class="pos-note">Small und Big Blind liegen fast immer im Minus, weil du dort Pflichteinsätze zahlst. Wichtig ist nur, dort nicht mehr als nötig zu verlieren.</p>
          {:else}
            <EmptyState compact title="Noch keine eigenen Hände in diesem Zeitraum" text="Wähle oben einen längeren Zeitraum." />
          {/if}
        </section>
      <section class="panel">
        <div class="panel-head">
          <h2 class="panel-title">Sessions</h2>
          <span class="faint count">Eine Session endet nach 30 Minuten Pause</span>
        </div>
        {#if data.sessions.length}
          <div class="scroll">
            <table class="table sessions">
              <thead>
                <tr>
                  <th>Datum</th>
                  <th>Uhrzeit</th>
                  <th>Dauer</th>
                  <th>Tische</th>
                  <th>Hände</th>
                  <th>BB</th>
                  <th>BB/100</th>
                  {#if filter.game !== "cash"}<th>Bounties</th>{/if}
                </tr>
              </thead>
              <tbody>
                {#each data.sessions as s (s.start)}
                  {@const v = s.hands ? (s.net_bb / s.hands) * 100 : 0}
                  <tr>
                    <td class="ui">{dayShort(s.start)}{#each several ? s.sites : [] as site (site)}{" "}<SiteMark {site} />{/each}</td>
                    <td>{time(s.start)}–{time(s.end)}</td>
                    <td>{duration(secondsBetween(s.start, s.end))}</td>
                    <td>{s.tables}</td>
                    <td>{int(s.hands)}</td>
                    <td class={sign(s.net_bb)}>{signed(s.net_bb)}</td>
                    <td class={sign(v)}>{signed(v)}</td>
                    {#if filter.game !== "cash"}
                      <td class:pos={!!bountyText(s.bounties)} class:faint={!bountyText(s.bounties)}>{bountyText(s.bounties) || "–"}</td>
                    {/if}
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        {:else}
          <EmptyState compact title="Keine Sessions in diesem Zeitraum" text="Wähle oben einen längeren Zeitraum." />
        {/if}
      </section>
      </div>
    </div>

  {/if}
</div>

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding-top: 20px;
  }
  .side {
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-width: 0;
  }
  .grid {
    display: grid;
    grid-template-columns: minmax(0, 560px) minmax(0, 1fr);
    gap: 16px;
    align-items: start;
  }
  .panel {
    overflow: hidden;
  }
  .scroll {
    overflow-x: auto;
    margin-top: 10px;
  }
  .count {
    font-size: 12px;
  }
  .pos-name,
  .ui {
    font-family: var(--font-ui) !important;
    font-size: 13.5px !important;
  }
  .pos-name {
    font-weight: 600;
  }
  .pos-note {
    margin: 0;
    padding: var(--sp-3) var(--sp-5) var(--sp-4);
    border-top: 1px solid var(--line);
    color: var(--text-3);
    font-size: var(--fs-sm);
  }
  .pos-name small {
    display: block;
    color: var(--text-3);
    font-size: var(--fs-xs);
    font-weight: var(--fw-regular);
  }
  tfoot td {
    border-bottom: 0;
    border-top: 1px solid var(--line-2);
  }
  .table tbody tr:last-child td {
    border-bottom: 0;
  }

  .matrix-wrap {
    padding: 14px 18px 0;
  }
  .matrix {
    position: relative;
    display: grid;
    grid-template-columns: repeat(13, minmax(0, 1fr));
    gap: 2px;
  }
  .cell {
    position: relative;
    aspect-ratio: 1;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    padding: 3px 4px;
    border-radius: 3px;
    overflow: hidden;
    cursor: default;
    min-width: 0;
  }
  .cell.empty {
    box-shadow: inset 0 0 0 1px var(--line);
  }
  .cell:hover {
    outline: 1px solid var(--text);
    outline-offset: 0;
    z-index: 1;
  }
  .k {
    font-family: var(--font-num);
    font-size: var(--fs-xs);
    font-weight: 600;
    line-height: 1;
    opacity: 0.9;
  }
  .v {
    align-self: flex-end;
    font-size: var(--fs-xs);
    line-height: 1;
  }
  .played .cell {
    justify-content: center;
    align-items: center;
    gap: 3px;
  }
  .played .k {
    font-size: 10px;
    opacity: 0.7;
  }
  .played .v {
    align-self: center;
    font-size: 13px;
    font-weight: 600;
  }
  .key {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 14px 18px 16px;
  }
  .note {
    margin: 0;
    font-size: 12.5px;
    color: var(--text-2);
  }
  .scale {
    display: flex;
    flex-direction: column;
    gap: 4px;
    max-width: 240px;
  }
  .ramp {
    height: 6px;
    border-radius: 3px;
  }
  .ramp.bb {
    background: linear-gradient(
      to right,
      color-mix(in srgb, var(--neg) 80%, var(--surface-2)),
      color-mix(in srgb, var(--neg) 8%, var(--surface-2)) 50%,
      color-mix(in srgb, var(--pos) 8%, var(--surface-2)) 50%,
      color-mix(in srgb, var(--pos) 80%, var(--surface-2))
    );
  }
  .ramp.played {
    background: linear-gradient(
      to right,
      color-mix(in srgb, var(--sd) 10%, transparent),
      color-mix(in srgb, var(--sd) 70%, transparent)
    );
  }
  .ticks {
    display: flex;
    justify-content: space-between;
    font-size: var(--fs-xs);
    color: var(--text-2);
  }

  @container (max-width: 1020px) {
    .grid {
      grid-template-columns: minmax(0, 1fr);
    }
    /* One column: the explanation moves beside the matrix instead of leaving a hole there. */
    .matrix-body {
      display: grid;
      grid-template-columns: minmax(0, 560px) minmax(180px, 1fr);
      align-items: start;
    }
    .matrix-body .key {
      padding: 14px 18px 16px 4px;
    }
  }
</style>
