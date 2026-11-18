<script lang="ts">
  import { tick } from "svelte";
  import { inTauri } from "../api";
  import JobsDock from "../jobs/JobsDock.svelte";
  import { jobs } from "../jobs/store.svelte";
  import type { JobTarget } from "../jobs/types";
  import Logo from "../Logo.svelte";
  import type { Filter, HudConfig, ImportStatus, TournamentRow } from "../types";
  import IconButton from "../ui/IconButton.svelte";
  import NavItem from "../ui/NavItem.svelte";
  import SourceNote from "../ui/SourceNote.svelte";
  import { folder } from "./fit";
  import { arrowNav } from "./keys";
  import { areaLine, navLive, tight, type Live } from "./live.svelte";
  import NavMeta from "./NavMeta.svelte";
  import NavNotices from "./NavNotices.svelte";
  import { AREAS, LOCKED_PAGES, PAGES, areaOf, type Area, type AreaId, type Page } from "./pages";

  /**
   * The app's navigation: a bar with the five intents. Each opens a large menu with every page of
   * the area, a sentence and a live value; under the bar the pages of the current area stay one
   * click away. Its content shares the left and right edge of the page below.
   */
  let {
    tab,
    locked,
    filter,
    tournaments,
    tables,
    status,
    hud,
    refresh,
    onSelect,
    onView,
    onChooseFolder,
  }: {
    tab: Page;
    locked: boolean;
    filter: Filter;
    tournaments: TournamentRow[];
    tables: number;
    status: ImportStatus | null;
    hud: HudConfig | null;
    refresh: number;
    onSelect: (page: Page) => void;
    onView: (target: JobTarget) => void;
    onChooseFolder: () => void;
  } = $props();

  // Browser demo only: a menu shown open (&navopen=<area>), the focus on an entry (&navfocus=<n>),
  // folding switched off (&navfit=0) and a mark of any length (&navmark=<text>), for pictures and
  // the layout check in e2e/layout.mjs.
  const preview = inTauri ? new URLSearchParams() : new URLSearchParams(window.location.search);
  const previewOpen = preview.get("navopen") as AreaId | null;
  const foldOff = preview.get("navfit") === "0";
  const longMark = preview.get("navmark");

  $effect(() => {
    void refresh;
    navLive.load(filter, tournaments, tables, status?.total_hands ?? 0, hud);
  });

  const current = $derived(areaOf(tab) ?? null);
  let open = $state<AreaId | null>(null);
  $effect(() => {
    if (previewOpen) open = previewOpen;
  });
  const menu = $derived(AREAS.find((a) => a.id === open && a.pages.length > 1) ?? null);
  const triggers: Partial<Record<AreaId, HTMLButtonElement>> = $state({});
  let panel = $state<HTMLElement>();
  let timer = 0;

  async function show(id: AreaId, focus: "first" | "none" = "none") {
    clearTimeout(timer);
    open = id;
    if (focus === "first") {
      await tick();
      panel?.querySelector<HTMLElement>("[data-nav]")?.focus();
    }
  }
  function close(back = false) {
    clearTimeout(timer);
    const id = open;
    open = null;
    if (back && id) triggers[id]?.focus();
  }
  const hasMenu = (id: AreaId) => AREAS.find((a) => a.id === id)!.pages.length > 1;
  /** Hover opens after a short rest; moving along the bar switches at once. */
  function rest(id: AreaId) {
    clearTimeout(timer);
    if (!hasMenu(id)) return;
    timer = window.setTimeout(() => (open = id), open ? 0 : 140);
  }
  function leave() {
    clearTimeout(timer);
    timer = window.setTimeout(() => (open = null), 260);
  }
  function go(p: Page) {
    close();
    onSelect(p);
  }
  function triggerKey(e: KeyboardEvent, id: AreaId) {
    const ids = AREAS.map((a) => a.id);
    const i = ids.indexOf(id);
    if (e.key === "ArrowRight" || e.key === "ArrowLeft") {
      e.preventDefault();
      const next = ids[(i + (e.key === "ArrowRight" ? 1 : -1) + ids.length) % ids.length];
      triggers[next]?.focus();
      if (open) open = hasMenu(next) ? next : null;
    } else if (e.key === "ArrowDown" && hasMenu(id)) {
      e.preventDefault();
      show(id, "first");
    }
  }
  function key(e: KeyboardEvent) {
    if (e.key === "Escape" && open) close(true);
  }
  function outside(e: PointerEvent) {
    const t = e.target as Node;
    if (open && !panel?.contains(t) && !Object.values(triggers).some((b) => b?.contains(t))) close();
  }
  const lockedNow = (p: Page) => locked && LOCKED_PAGES.includes(p);

  /** What a tab shows at rest: the first value in its area that calls for something. */
  function badge(area: Area): Live | undefined {
    if (longMark && area.id === "spiel") return { text: longMark, long: longMark, line: longMark, tone: "warn" };
    for (const p of area.pages) {
      const l = navLive.of(p, locked);
      if (l && l.tone !== "neutral") return l;
    }
    return undefined;
  }

  /*
   * Folding of the bar when room runs short, one step at a time. What also shows elsewhere goes
   * first, the lock note last:
   *   1 status without "im Blick" / "gespeichert"   2 tab marks as a number ("4")
   *   3 hand count off                               4 lock note short ("PokerStars läuft")
   *   5 name "Ringside" off                          6 open tables off (the Gegner mark keeps them)
   *   7 tasks as their ring only                     8 tab icons off
   *   9 lock note as its icon only (the tabs keep their locks)
   *  10 tab marks off (the menus keep them)
   */
  let step = $state(0);
  let row = $state<HTMLElement>();
  const fold = folder(10, (n) => (step = foldOff ? 0 : n));
  /** The row under the bar: 1 values off, 2 icons off. */
  let subStep = $state(0);
  let subRow = $state<HTMLElement>();
  const foldSub = folder(2, (n) => (subStep = foldOff ? 0 : n));

  $effect(() => {
    if (!row) return;
    const ro = new ResizeObserver(() => fold(row));
    ro.observe(row);
    return () => ro.disconnect();
  });
  $effect(() => {
    if (!subRow) return;
    const ro = new ResizeObserver(() => foldSub(subRow));
    ro.observe(subRow);
    return () => ro.disconnect();
  });
  // Content that changes the width: marks, the lock, tables, hands and running tasks.
  $effect(() => {
    void [AREAS.map((a) => badge(a)?.long), locked, tables, status?.total_hands, jobs.list.map((j) => j.status), jobs.locked];
    fold(row);
    foldSub(subRow);
  });
  $effect(() => {
    void tab;
    tick().then(() => foldSub(subRow));
  });

  // Browser demo: &navfocus=<n> puts the keyboard focus on the n-th menu entry, for pictures.
  const focusAt = Number(preview.get("navfocus"));
  $effect(() => {
    if (!Number.isInteger(focusAt) || focusAt < 1) return;
    const t = window.setTimeout(() => document.querySelectorAll<HTMLElement>("[data-nav]")[focusAt - 1]?.focus(), 400);
    return () => clearTimeout(t);
  });

  const LOCK_HINT =
    "Gesperrt, solange PokerStars läuft. Lernen, Hände spielen, Schwachstellen und die Werkzeuge öffnen wieder, sobald du PokerStars schließt; HUD und Statistik laufen weiter.";
</script>

<svelte:window onkeydown={key} onpointerdown={outside} />

<div class="top" role="banner" onpointerleave={leave} onpointerenter={() => open && clearTimeout(timer)}>
  <div class="bar">
    <div class="row" bind:this={row} data-fold={step}>
      <div class="brand" data-bar-item>
        <Logo size={24} />
        {#if step < 5}<strong>Ringside</strong>{/if}
      </div>
      <nav aria-label="Hauptmenü" class="intents" data-bar-item>
        {#each AREAS as area (area.id)}
          {@const single = area.pages.length === 1}
          {@const mark = badge(area)}
          {@const shut = area.pages.some(lockedNow)}
          <NavItem
            size="top"
            label={area.label}
            icon={step < 8 ? area.icon : undefined}
            active={single && tab === area.pages[0]}
            current={!single && current?.id === area.id}
            chevron={single ? undefined : "down"}
            status={mark && step < 10 ? (step < 2 ? (mark.line ?? mark.long) : mark.text) : undefined}
            tone={mark?.tone}
            locked={shut}
            title={shut ? "Teile sind gesperrt, solange PokerStars läuft" : undefined}
            aria-haspopup={single ? undefined : "true"}
            aria-expanded={single ? undefined : open === area.id}
            aria-controls={single ? undefined : "nav-menu"}
            bind:el={triggers[area.id]}
            onpointerenter={() => rest(area.id)}
            onkeydown={(e) => triggerKey(e, area.id)}
            onclick={() => (single ? go(area.pages[0]) : open === area.id ? close() : show(area.id))}
          />
        {/each}
      </nav>
      <div class="right">
        {#if locked}
          <span class="item" data-bar-item>
            <SourceNote kind="locked" hint={LOCK_HINT} label={step < 4 ? "PokerStars läuft: Teile gesperrt" : step < 9 ? "PokerStars läuft" : ""} />
          </span>
        {/if}
        {#if step < 6}
          <span class="item" data-bar-item><NavMeta {tables} hands={status?.total_hands ?? 0} words={step < 1} showHands={step < 3} /></span>
        {/if}
        <span class="item dock" data-bar-item><JobsDock {onView} placement="top" compact={step >= 7} /></span>
        <span class="item gear" class:on={tab === "settings"} data-bar-item>
          <IconButton label="Einstellungen" aria-current={tab === "settings" ? "page" : undefined} onclick={() => go("settings")}>
            <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true"><path d={PAGES.settings.icon} /></svg>
          </IconButton>
        </span>
      </div>
    </div>
  </div>

  {#if current && current.pages.length > 1}
    <div class="sub">
      <nav class="row" aria-label={current.label} use:arrowNav bind:this={subRow} data-fold={subStep}>
        {#each current.pages as p (p)}
          {@const live = navLive.of(p, locked)}
          <NavItem
            size="sub"
            data-nav
            data-sub-item
            label={PAGES[p].label}
            icon={subStep < 2 ? PAGES[p].icon : undefined}
            active={tab === p}
            locked={lockedNow(p)}
            status={subStep < 1 ? tight(live) : undefined}
            tone={live?.tone}
            title={PAGES[p].text}
            onclick={() => go(p)}
          />
        {/each}
      </nav>
    </div>
  {/if}

  {#if menu}
    {@const line = areaLine(menu, locked)}
    <div class="menu" id="nav-menu" role="region" aria-label={menu.label} bind:this={panel} use:arrowNav>
      <div class="menu-inner">
        <div class="intro">
          <span class="plate"><svg viewBox="0 0 24 24" width="22" height="22" aria-hidden="true"><path d={menu.icon} /></svg></span>
          <b>{menu.label}</b>
          <p>{menu.text}</p>
          {#if locked && menu.pages.some(lockedNow)}
            <SourceNote kind="locked" variant="block">Öffnet wieder, sobald du PokerStars schließt.</SourceNote>
          {:else if line.tone !== "neutral" || line.text !== menu.short}<span class="line {line.tone}">{line.text}</span>{/if}
        </div>
        <div class="tiles">
          {#each menu.pages as p (p)}
            {@const live = navLive.of(p, locked)}
            <NavItem
              size="tile"
              data-nav
              label={PAGES[p].label}
              icon={PAGES[p].icon}
              text={PAGES[p].text}
              active={tab === p}
              locked={lockedNow(p)}
              status={live?.long}
              tone={live?.tone}
              onclick={() => go(p)}
            />
          {/each}
        </div>
      </div>
    </div>
    <div class="scrim" aria-hidden="true"></div>
  {/if}
</div>
<NavNotices {status} settingsOpen={tab === "settings"} {onChooseFolder} />

<style>
  .top {
    position: relative;
    z-index: var(--z-dock);
    background: var(--surface-sunken);
    border-bottom: 1px solid var(--line);
  }
  /* Bar and row carry their own ground, so the scrim behind an open menu leaves them bright. */
  .bar,
  .sub {
    position: relative;
    background: var(--surface-sunken);
  }
  /* Full-width bars; their content starts and ends where the page's does. */
  .row {
    display: flex;
    align-items: center;
    overflow: hidden;
    padding-left: var(--frame-left, var(--frame-auto));
    padding-right: var(--frame-right, var(--frame-auto));
  }
  .row > :global(*) {
    flex-shrink: 0;
  }
  .bar .row {
    gap: var(--sp-4);
    height: 56px;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-right: var(--sp-2);
    white-space: nowrap;
  }
  .brand strong {
    font-family: var(--font-num);
    font-size: var(--fs-lg);
    font-weight: var(--fw-semibold);
    letter-spacing: 0.02em;
  }
  .intents {
    display: flex;
    align-items: center;
    gap: 2px;
  }
  .right {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    margin-left: auto;
  }
  .item {
    display: flex;
    align-items: center;
  }
  .dock {
    min-width: 0;
  }
  .gear {
    margin-right: -6px;
    border-radius: var(--r-sm);
  }
  .gear.on {
    background: var(--surface-2);
  }

  .sub {
    border-top: 1px solid var(--line);
  }
  .sub .row {
    gap: 2px;
    height: 40px;
  }
  /* Sub tabs hang their inner padding into the margin, so their text starts at the page edge. */
  .sub .row > :global(:first-child) {
    margin-left: -10px;
  }

  .menu {
    position: absolute;
    left: 0;
    right: 0;
    top: 56px;
    z-index: var(--z-popover);
    border-top: 1px solid var(--line);
    border-bottom: 1px solid var(--line-2);
    background: var(--surface);
    box-shadow: var(--shadow-3);
    animation: drop var(--dur) var(--ease-enter);
  }
  .menu-inner {
    display: grid;
    grid-template-columns: 260px minmax(0, 1fr);
    gap: var(--sp-6);
    padding: var(--sp-5) var(--frame-right, var(--frame-auto)) var(--sp-6) var(--frame-left, var(--frame-auto));
  }
  .intro {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--sp-2);
    padding-right: var(--sp-6);
    border-right: 1px solid var(--line);
  }
  .plate {
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    margin-bottom: var(--sp-1);
    border-radius: var(--r-md);
    background: var(--surface-2);
  }
  .plate svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 1.6;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .intro b {
    font-size: var(--fs-xl);
    font-weight: var(--fw-semibold);
  }
  .intro p {
    margin: 0;
    font-size: var(--fs-base);
    line-height: var(--lh-base);
    color: var(--text-2);
  }
  .line {
    font-family: var(--font-num);
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .line.warn {
    color: var(--amber);
  }
  .line.bad {
    color: var(--danger-text);
  }
  .line.good {
    color: var(--pos);
  }
  .tiles {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: var(--sp-1) var(--sp-3);
    align-content: start;
  }
  .scrim {
    position: fixed;
    inset: 0;
    z-index: -1;
    background: var(--overlay);
    animation: fade var(--dur) var(--ease-standard);
    pointer-events: none;
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
  @media (max-width: 1279px) {
    .menu-inner {
      grid-template-columns: 200px minmax(0, 1fr);
      gap: var(--sp-4);
    }
    .tiles {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
    .intro {
      padding-right: var(--sp-4);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .menu,
    .scrim {
      animation: none;
    }
  }
</style>
