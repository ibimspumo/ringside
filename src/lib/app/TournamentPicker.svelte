<script lang="ts">
  import { buyInText, tournamentName } from "../stats";
  import type { TournamentRow } from "../types";
  import { date, floating, int, type Anchor } from "./format";

  let {
    tournaments,
    selected,
    onSelect,
  }: { tournaments: TournamentRow[]; selected: number | null; onSelect: (id: number | null) => void } = $props();

  let open = $state(false);
  let search = $state("");
  let active = $state(0);
  let anchor = $state<Anchor>({ left: 0, top: 0, right: 0, bottom: 0 });
  let button = $state<HTMLButtonElement>();
  let popover = $state<HTMLElement>();
  let input = $state<HTMLInputElement>();

  const current = $derived(selected === null ? null : (tournaments.find((t) => t.id === selected) ?? null));
  const label = $derived(selected === null ? "Alle Turniere" : current ? tournamentName(current) : `Turnier #${selected}`);

  /** Newest first; the search matches name, game and id. */
  const matches = $derived.by(() => {
    const q = search.trim().toLowerCase();
    const rows = [...tournaments].sort((a, b) => b.started_at.localeCompare(a.started_at) || b.id - a.id);
    if (!q) return rows;
    return rows.filter((t) => `${t.name ?? ""} ${t.game} ${t.id}`.toLowerCase().includes(q));
  });
  /** Row 0 is "Alle Turniere" while nothing is searched. */
  const offset = $derived(search.trim() ? 0 : 1);
  const count = $derived(matches.length + offset);

  function toggle() {
    if (open) return close();
    const r = button!.getBoundingClientRect();
    anchor = { left: r.left, top: r.top, right: r.right, bottom: r.bottom };
    search = "";
    const at = selected === null ? -1 : matches.findIndex((t) => t.id === selected);
    active = at < 0 ? 0 : at + 1;
    open = true;
  }

  function close() {
    open = false;
  }

  function pick(id: number | null) {
    onSelect(id);
    close();
    button?.focus();
  }

  function pickIndex(i: number) {
    if (i < offset) return pick(null);
    const t = matches[i - offset];
    if (t) pick(t.id);
  }

  $effect(() => {
    if (!open) return;
    input?.focus();
  });

  $effect(() => {
    void search;
    active = Math.min(active, Math.max(0, count - 1));
  });

  $effect(() => {
    if (!open || !popover) return;
    popover.querySelector(`[data-index="${active}"]`)?.scrollIntoView({ block: "nearest" });
  });

  function key(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      close();
      button?.focus();
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      active = Math.min(count - 1, active + 1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      active = Math.max(0, active - 1);
    } else if (e.key === "Enter") {
      e.preventDefault();
      pickIndex(active);
    }
  }

  function outside(e: PointerEvent) {
    if (!open) return;
    const target = e.target as Node;
    if (popover?.contains(target) || button?.contains(target)) return;
    close();
  }
</script>

<svelte:window onpointerdown={outside} onresize={close} />

<div class="picker" class:set={selected !== null}>
  <button
    bind:this={button}
    class="trigger"
    aria-haspopup="listbox"
    aria-expanded={open}
    title={current ? `${tournamentName(current)} · ${current.game}` : undefined}
    onclick={toggle}
  >
    <svg viewBox="0 0 24 24" width="15" height="15" aria-hidden="true">
      <path d="M8 4.5h8V9a4 4 0 0 1-8 0zM8 6.5H5.5v1a3 3 0 0 0 2.7 3M16 6.5h2.5v1a3 3 0 0 1-2.7 3M12 13v3.5M8.5 19.5h7M10 16.5h4" />
    </svg>
    <span class="label">{label}</span>
    <svg class="chev" viewBox="0 0 24 24" width="13" height="13" aria-hidden="true"><path d="m7 10 5 5 5-5" /></svg>
  </button>
  {#if selected !== null}
    <button class="clear" onclick={() => onSelect(null)} aria-label="Turnierfilter entfernen" title="Turnierfilter entfernen">
      <svg viewBox="0 0 24 24" width="12" height="12" aria-hidden="true"><path d="M6 6l12 12M18 6 6 18" /></svg>
    </button>
  {/if}
</div>

{#if open}
  <div class="popover" bind:this={popover} use:floating={{ anchor, side: "bottom" }}>
    <input
      bind:this={input}
      bind:value={search}
      class="field search"
      type="search"
      placeholder="Turnier suchen – Name oder Nummer"
      aria-label="Turnier suchen"
      aria-controls="tournament-list"
      aria-activedescendant="tournament-{active}"
      onkeydown={key}
    />
    <div class="list" id="tournament-list" role="listbox" aria-label="Turniere">
      {#if offset}
        <button
          id="tournament-0"
          data-index="0"
          class="item all"
          class:active={active === 0}
          role="option"
          aria-selected={selected === null}
          tabindex="-1"
          onpointerenter={() => (active = 0)}
          onclick={() => pick(null)}
        >
          <span class="name">Alle Turniere</span>
          <span class="meta">{int(tournaments.length)} Turniere</span>
        </button>
      {/if}
      {#each matches as t, i (t.id)}
        {@const index = i + offset}
        <button
          id="tournament-{index}"
          data-index={index}
          class="item"
          class:active={active === index}
          class:on={t.id === selected}
          role="option"
          aria-selected={t.id === selected}
          tabindex="-1"
          onpointerenter={() => (active = index)}
          onclick={() => pick(t.id)}
        >
          <span class="name" title={tournamentName(t)}>{tournamentName(t)}</span>
          <span class="meta">
            <span>{buyInText(t)}</span>
            <span>{t.started_at ? date(t.started_at) : "–"}</span>
            <span><span class="num">{int(t.hands)}</span> Hände</span>
            {#if t.name}<span class="faint num">#{t.id}</span>{/if}
          </span>
        </button>
      {:else}
        <p class="none">Kein Turnier passt zu „{search.trim()}“.</p>
      {/each}
    </div>
  </div>
{/if}

<style>
  .picker {
    display: inline-flex;
    align-items: center;
    max-width: 300px;
    height: 32px;
    padding: 2px;
    border-radius: 8px;
    background: var(--bg);
    border: 1px solid var(--line);
  }
  .picker.set {
    border-color: var(--line-2);
    background: var(--surface-2);
  }
  .trigger {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    min-width: 0;
    height: 26px;
    padding: 0 8px 0 9px;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: var(--text-2);
    font-size: 13px;
    cursor: pointer;
  }
  .set .trigger,
  .trigger:hover,
  .trigger[aria-expanded="true"] {
    color: var(--text);
  }
  .label {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .trigger svg,
  .clear svg {
    flex-shrink: 0;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.7;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .chev {
    opacity: 0.7;
  }
  .clear {
    display: grid;
    place-items: center;
    flex-shrink: 0;
    width: 22px;
    height: 22px;
    margin-right: 1px;
    border: 0;
    border-radius: 50%;
    background: transparent;
    color: var(--text-2);
    cursor: pointer;
  }
  .clear svg {
    stroke-width: 2.2;
  }
  .clear:hover {
    background: var(--surface-3);
    color: var(--text);
  }

  .popover {
    display: flex;
    flex-direction: column;
    width: min(400px, calc(100vw - 16px));
    max-height: min(440px, calc(100vh - 16px));
    border: 1px solid var(--line-2);
    border-radius: var(--r-lg);
    background: var(--surface);
    box-shadow: 0 16px 40px rgb(0 0 0 / 0.45);
    overflow: hidden;
  }
  .search {
    margin: 8px;
    height: 32px;
  }
  .list {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 0 6px 6px;
    overscroll-behavior: contain;
  }
  .item {
    display: flex;
    flex-direction: column;
    gap: 3px;
    width: 100%;
    padding: 7px 10px;
    border: 0;
    border-radius: var(--r-sm);
    background: transparent;
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }
  .item.active {
    background: var(--surface-2);
  }
  .item.on {
    box-shadow: inset 2px 0 0 var(--text);
  }
  .item.all {
    margin-bottom: 4px;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 13.5px;
  }
  .meta {
    display: flex;
    gap: 12px;
    font-size: 12px;
    color: var(--text-2);
    white-space: nowrap;
  }
  .none {
    margin: 6px 10px 10px;
    font-size: 13px;
    color: var(--text-2);
  }
</style>
