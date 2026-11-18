<script lang="ts" module>
  import type { StatId, StatValue } from "../types";

  export interface PlayerRow {
    key: string;
    name: string;
    stats: StatValue[];
    label?: { name: string; color: string } | null;
    /** Your note; a narrow list shows it under the name instead of in its own column. */
    note?: string;
    hero?: boolean;
  }

  /** A plain column next to the values, e.g. stack or note; `wide` takes the free width. */
  export interface ExtraColumn {
    key: string;
    head: string;
    help?: string;
    at: "before" | "after";
    wide?: boolean;
    text: (row: PlayerRow) => string;
    unit?: string;
  }

  /** The four values that tell a type apart; the rest on request. */
  export const CORE: StatId[] = ["vpip", "pfr", "three_bet", "af"];
  export const MORE: StatId[] = ["fold_to_three_bet", "steal", "fold_to_steal", "cbet", "fold_to_cbet", "wtsd", "wsd", "afq"];
</script>

<script lang="ts">
  import { STAT_META, stat } from "../stats";
  import type { HudConfig } from "../types";
  import Badge from "../ui/Badge.svelte";
  import CellButton from "../ui/CellButton.svelte";
  import SortHeader from "../ui/SortHeader.svelte";
  import Tooltip from "../ui/Tooltip.svelte";
  import { int } from "./format";
  import { SMALL_SAMPLE, TYPE, read } from "./reads";
  import StatCell from "./StatCell.svelte";
  import Wheel from "./Wheel.svelte";

  let {
    rows,
    hud,
    columns,
    advice = false,
    extra = [],
    selected = null,
    onPick,
    sort,
    label,
    maxHeight,
    onMore,
  }: {
    rows: PlayerRow[];
    hud: HudConfig;
    columns: "core" | "all";
    /** Show the player type column (only while PokerStars is closed). */
    advice?: boolean;
    extra?: ExtraColumn[];
    selected?: string | null;
    onPick: (key: string) => void;
    /** Sortable headers; without it the rows keep their order (seats at a table). */
    sort?: { by: StatId | "name"; asc: boolean; on: (id: StatId | "name") => void };
    label: string;
    /** Scrolls vertically beyond this height, header stays. */
    maxHeight?: string;
    /** Told whether more columns sit to the right than fit; the page says so next to its tools. */
    onMore?: (more: boolean) => void;
  } = $props();

  const SHORT: Partial<Record<StatId, string>> = { pfr: "PFR", three_bet: "3-Bet", af: "AF" };
  /** All twelve side by side: German words, two lines where needed. */
  const NARROW: Record<StatId, string> = {
    hands: "Hände",
    vpip: "VPIP",
    pfr: "PFR",
    three_bet: "3-Bet",
    af: "AF",
    fold_to_three_bet: "Fold auf\n3-Bet",
    steal: "Steal",
    fold_to_steal: "Fold auf\nSteal",
    cbet: "C-Bet",
    fold_to_cbet: "Fold auf\nC-Bet",
    wtsd: "Show-\ndown",
    wsd: "Showdown\ngewonnen",
    afq: "Aggr.\nnach Flop",
  };
  const cols = $derived(columns === "all" ? [...CORE, ...MORE] : CORE);
  const before = $derived(extra.filter((c) => c.at === "before"));
  const after = $derived(extra.filter((c) => c.at === "after"));
  const reads = $derived(new Map(rows.map((r) => [r.key, read(r.stats)])));
  let body = $state<HTMLElement>();
  let scroller = $state<HTMLElement>();
  let more = $state(false);
  $effect(() => onMore?.(more));

  function measure() {
    if (scroller) more = scroller.scrollLeft + scroller.clientWidth < scroller.scrollWidth - 1;
  }
  $effect(() => {
    if (!scroller) return;
    void columns;
    const ro = new ResizeObserver(measure);
    ro.observe(scroller);
    if (scroller.firstElementChild) ro.observe(scroller.firstElementChild);
    return () => ro.disconnect();
  });

  const ariaSort = (id: StatId | "name") => (sort?.by === id ? (sort.asc ? "ascending" : "descending") : undefined);

  function key(e: KeyboardEvent, i: number) {
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    e.preventDefault();
    const next = Math.max(0, Math.min(rows.length - 1, i + (e.key === "ArrowDown" ? 1 : -1)));
    onPick(rows[next].key);
    body?.querySelectorAll<HTMLElement>("td.name button")[next]?.focus();
  }
</script>

{#snippet head(id: StatId | "name", text: string)}
  <SortHeader term={id !== "name"} active={sort?.by === id} asc={sort?.asc} onclick={sort ? () => sort.on(id) : undefined}>{text}</SortHeader>
{/snippet}

<div class="frame" class:more>
<div class="wrap" bind:this={scroller} onscroll={measure} style:max-height={maxHeight}>
<table class:all={columns === "all"} aria-label={label}>
  <thead>
    <tr>
      <th class="name" aria-sort={ariaSort("name")}>{@render head("name", "Spieler")}</th>
      {#if advice}<th class="type"><Tooltip text="Faustregel aus VPIP und PFR. Gestrichelt heißt: noch zu wenig Hände für ein sicheres Urteil." side="bottom"><SortHeader term>Typ</SortHeader></Tooltip></th>{/if}
      {#each before as c (c.key)}<th class="extra" class:wide={c.wide}>{#if c.help}<Tooltip text={c.help} side="bottom"><SortHeader term>{c.head}</SortHeader></Tooltip>{:else}<SortHeader>{c.head}</SortHeader>{/if}</th>{/each}
      <th class="hands" aria-sort={ariaSort("hands")}>
        <Tooltip text={STAT_META.hands.help} side="bottom">{@render head("hands", "Hände")}</Tooltip>
      </th>
      {#each cols as id (id)}
        <th class:core={columns === "core"} class:ringed={columns === "all"} aria-sort={ariaSort(id)}>
          <Tooltip text={`${STAT_META[id].name}: ${STAT_META[id].help}`} side="bottom">
            {@render head(id, columns === "core" ? `${SHORT[id] ?? STAT_META[id].name} ${STAT_META[id].ratio ? "×" : "%"}` : NARROW[id])}
          </Tooltip>
        </th>
      {/each}
      {#each after as c (c.key)}<th class="extra left" class:wide={c.wide} class:note-col={c.key === "note"}><SortHeader>{c.head}</SortHeader></th>{/each}
    </tr>
  </thead>
  <tbody bind:this={body}>
    {#each rows as p, i (p.key)}
      {@const r = reads.get(p.key)}
      {@const hands = stat(p.stats, "hands")?.value ?? 0}
      <tr class:on={selected === p.key} onclick={() => onPick(p.key)}>
        <td class="name">
          <CellButton current={selected === p.key} onclick={(e) => (e.stopPropagation(), onPick(p.key))} onkeydown={(e) => key(e, i)}>
            <Wheel stats={p.stats} {hud} size={18} />
            <span class="who-text">
              <span class="line">
                <span class="n" title={p.name}>{p.name}</span>
                {#if p.label}<i class="dot" style="--c:{p.label.color}" title={p.label.name || "Label aus PokerStars"}></i>{/if}
                {#if p.hero}<Badge size="sm">du</Badge>{/if}
              </span>
              {#if p.note}<span class="note-line" title={p.note}>{p.note}</span>{/if}
            </span>
          </CellButton>
        </td>
        {#if advice}
          <td class="type">
            {#if p.hero}
              <span class="q">–</span>
            {:else if r?.type}
              <span class="t" title={`${TYPE[r.type].long} – ${r.note}`}>
                <Badge size="sm" dashed={r.sure !== "sure"}>{TYPE[r.type].name}</Badge>
              </span>
            {:else}
              <span class="q" title="Die Werte passen zu keinem der vier Typen.">kein klarer Typ</span>
            {/if}
          </td>
        {/if}
        {#each before as c (c.key)}<td class="extra num" class:wide={c.wide}>{c.text(p)}{#if c.unit}<span class="unit">{c.unit}</span>{/if}</td>{/each}
        <td class="hands num" class:thin={hands < SMALL_SAMPLE}>{int(hands)}</td>
        {#each cols as id (id)}
          <td><StatCell stats={p.stats} {id} {hud} meter={columns === "core"} /></td>
        {/each}
        {#each after as c (c.key)}<td class="extra left note" class:wide={c.wide} class:note-col={c.key === "note"} title={c.text(p) || undefined}>{c.text(p)}</td>{/each}
      </tr>
    {/each}
  </tbody>
</table>
</div>
</div>

<style>
  .frame {
    position: relative;
  }
  .wrap {
    container: player-list / inline-size;
    overflow: auto;
  }
  .more::after {
    content: "";
    position: absolute;
    top: 0;
    right: 0;
    bottom: 0;
    width: 28px;
    background: linear-gradient(to left, var(--shadow-edge), transparent);
    pointer-events: none;
  }
  table {
    width: 100%;
    border-collapse: collapse;
  }
  th {
    position: sticky;
    top: 0;
    z-index: var(--z-raised);
    height: 36px;
    vertical-align: bottom;
    padding: var(--sp-2) var(--sp-2) 10px;
    background: var(--surface);
    border-bottom: 1px solid var(--line-2);
    text-align: right;
    white-space: nowrap;
  }
  /* With scales the number opens the cell, so its title stands left above it. */
  th.core {
    width: 120px;
    text-align: left;
  }
  @container player-list (max-width: 760px) {
    th.core {
      width: auto;
      padding-right: calc(var(--sp-2) + 10px);
      text-align: right;
    }
  }
  /* Numbers carry room for the "unsure" ring after them; titles line up with the numbers. */
  th.ringed {
    padding-right: calc(var(--sp-2) + 10px);
  }
  th.name,
  th.type,
  th.left {
    text-align: left;
  }
  th.name {
    padding-left: var(--sp-4);
  }
  .wide {
    width: 30%;
  }
  td {
    height: 40px;
    padding: 0 var(--sp-2);
    border-bottom: 1px solid var(--line);
    text-align: right;
    transition: background var(--dur-fast) var(--ease-standard);
  }
  tbody tr {
    cursor: pointer;
  }
  tbody tr:hover td,
  tbody tr.on td {
    background: var(--surface-2);
  }
  tbody tr.on td.name {
    box-shadow: inset 2px 0 0 var(--text);
  }
  tbody tr:last-child td {
    border-bottom: 0;
  }
  td.name {
    padding: 0;
    text-align: left;
  }
  .who-text {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .line {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    min-width: 0;
  }
  .note-line {
    display: none;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-2);
    font-size: var(--fs-sm);
  }
  @container player-list (max-width: 760px) {
    .line {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    min-width: 0;
  }
  .note-line {
      display: block;
    }
    .note-col {
      display: none;
    }
  }
  .n {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .dot {
    flex-shrink: 0;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--c);
  }
  td.type,
  td.left {
    text-align: left;
    white-space: nowrap;
  }
  .t {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .q {
    color: var(--text-3);
    font-size: var(--fs-sm);
  }
  .hands,
  .extra {
    font-size: var(--fs-base);
    color: var(--text);
    white-space: nowrap;
  }
  .hands.thin {
    color: var(--text-3);
  }
  .note {
    max-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--text-2);
    font-family: var(--font-ui);
    font-size: var(--fs-md);
  }
</style>
