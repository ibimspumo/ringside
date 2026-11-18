<script lang="ts">
  import type { Snippet } from "svelte";
  import Logo from "../Logo.svelte";
  import { floating, rectOf, type Anchor } from "../ui/floating";
  import NavItem from "../ui/NavItem.svelte";
  import SourceNote from "../ui/SourceNote.svelte";
  import { arrowNav } from "./keys";
  import { areaLine, navLive } from "./live.svelte";
  import { AREAS, LOCKED_PAGES, PAGES, areaOf, type Area, type AreaId, type Page } from "./pages";

  /**
   * Concept B: a slim side bar of five areas. The area on screen stays open with its pages; the
   * others open on click, or show their pages beside the bar while the pointer rests on them.
   * The start page hands out the areas as large tiles (Hub).
   */
  let {
    tab,
    locked,
    open,
    onSelect,
    foot,
  }: { tab: Page; locked: boolean; open?: AreaId | null; onSelect: (page: Page) => void; foot: Snippet } = $props();

  const current = $derived(areaOf(tab)?.id ?? null);
  /** Areas the player opened besides the current one. */
  let opened = $state<Set<AreaId>>(new Set());
  /** The current area folded by hand. */
  let folded = $state<AreaId | null>(null);
  const isOpen = (id: AreaId) => (id === current ? folded !== id : opened.has(id));
  function toggle(id: AreaId) {
    fly = null;
    if (id === current) folded = folded === id ? null : id;
    else {
      const next = new Set(opened);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      opened = next;
    }
  }

  /** Pages beside the bar while the pointer rests on a folded area. */
  let fly = $state<{ area: Area; anchor: Anchor } | null>(null);
  const heads: Partial<Record<AreaId, HTMLButtonElement>> = $state({});
  let timer = 0;
  function rest(area: Area) {
    clearTimeout(timer);
    if (isOpen(area.id)) return void (fly = null);
    timer = window.setTimeout(() => {
      const el = heads[area.id];
      if (el) fly = { area, anchor: rectOf(el) };
    }, fly ? 0 : 160);
  }
  function leave() {
    clearTimeout(timer);
    timer = window.setTimeout(() => (fly = null), 220);
  }
  // Browser demo: ?navopen=<area> shows the pages beside the bar without a pointer.
  $effect(() => {
    if (!open || open === current) return;
    const area = AREAS.find((a) => a.id === open);
    const el = heads[open];
    if (area && el) fly = { area, anchor: rectOf(el) };
  });
  function key(e: KeyboardEvent) {
    if (e.key !== "Escape" || !fly) return;
    const id = fly.area.id;
    fly = null;
    heads[id]?.focus();
  }
  const lockedNow = (p: Page) => locked && LOCKED_PAGES.includes(p);
</script>

<svelte:window onkeydown={key} />

<aside class="side" aria-label="Navigation">
  <div class="brand">
    <Logo size={26} />
    <strong>Ringside</strong>
  </div>

  <nav aria-label="Hauptmenü" use:arrowNav>
    {#each AREAS as area (area.id)}
      {#if area.id === "start"}
        {@const live = navLive.of("dashboard", locked)}
        <NavItem size="lg" data-nav label="Start" icon={area.icon} active={tab === "dashboard"} status={live?.long ?? area.short} onclick={() => onSelect("dashboard")} />
      {:else}
        {@const line = areaLine(area, locked)}
        <div class="area" class:open={isOpen(area.id)} role="group" aria-labelledby="nav-b-{area.id}" onpointerenter={() => rest(area)} onpointerleave={leave}>
          <NavItem
            size="lg"
            data-nav
            id="nav-b-{area.id}"
            label={area.label}
            icon={area.icon}
            current={current === area.id}
            status={line.text}
            tone={line.tone}
            chevron="down"
            aria-expanded={isOpen(area.id)}
            aria-controls="nav-b-{area.id}-pages"
            bind:el={heads[area.id]}
            onclick={() => toggle(area.id)}
          />
          <div class="pages" id="nav-b-{area.id}-pages" inert={!isOpen(area.id)}>
            <div class="inner">
              {#each area.pages as p (p)}
                {@const live = navLive.of(p, locked)}
                <NavItem size="md" data-nav label={PAGES[p].label} icon={PAGES[p].icon} active={tab === p} locked={lockedNow(p)} status={live?.text} tone={live?.tone} title={PAGES[p].text} onclick={() => onSelect(p)} />
              {/each}
            </div>
          </div>
        </div>
      {/if}
    {/each}
  </nav>

  <footer>
    {#if locked}
      <div class="lock"><SourceNote kind="locked" hint="Lernen, Hände spielen und die Werkzeuge öffnen wieder, sobald du PokerStars schließt. HUD und Statistik laufen weiter." /></div>
    {/if}
    {@render foot()}
    <div use:arrowNav>
      <NavItem size="sm" data-nav label="Einstellungen" icon={PAGES.settings.icon} active={tab === "settings"} onclick={() => onSelect("settings")} />
    </div>
  </footer>
</aside>

{#if fly}
  {@const area = fly.area}
  <div
    class="fly"
    role="menu"
    aria-label={area.label}
    tabindex="-1"
    use:floating={{ anchor: fly.anchor, side: "right" }}
    use:arrowNav
    onpointerenter={() => clearTimeout(timer)}
    onpointerleave={leave}
  >
    <header>
      <b>{area.label}</b>
      <span>{area.text}</span>
    </header>
    {#each area.pages as p (p)}
      {@const live = navLive.of(p, locked)}
      <NavItem
        size="tile"
        role="menuitem"
        data-nav
        label={PAGES[p].label}
        icon={PAGES[p].icon}
        text={PAGES[p].text}
        active={tab === p}
        locked={lockedNow(p)}
        status={live?.long}
        tone={live?.tone}
        onclick={() => {
          fly = null;
          onSelect(p);
        }}
      />
    {/each}
  </div>
{/if}

<style>
  .side {
    display: flex;
    flex-direction: column;
    min-height: 0;
    padding: var(--sp-4) 10px 14px;
    background: var(--surface-sunken);
    border-right: 1px solid var(--line);
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 32px;
    padding: 0 var(--sp-2);
    margin-bottom: 14px;
  }
  .brand strong {
    font-family: var(--font-num);
    font-size: var(--fs-lg);
    font-weight: var(--fw-semibold);
    letter-spacing: 0.02em;
  }
  nav {
    display: flex;
    flex-direction: column;
    gap: var(--sp-1);
    min-height: 0;
    overflow-y: auto;
  }
  .area {
    display: flex;
    flex-direction: column;
  }
  /* The pages fold in and out by height; a guide line ties them to their area. */
  .pages {
    display: grid;
    grid-template-rows: 0fr;
    opacity: 0;
    transition:
      grid-template-rows var(--dur) var(--ease-standard),
      opacity var(--dur-fast) var(--ease-standard);
  }
  .open .pages {
    grid-template-rows: 1fr;
    opacity: 1;
  }
  .inner {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-height: 0;
    overflow: hidden;
    margin-left: 22px;
    padding: 2px 0 6px 10px;
    border-left: 1px solid var(--line-2);
  }
  .lock {
    padding: 0 10px 2px;
  }
  footer {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    margin-top: auto;
    padding-top: var(--sp-3);
    border-top: 1px solid var(--line);
  }

  .fly {
    display: flex;
    flex-direction: column;
    gap: 2px;
    width: 340px;
    padding: var(--sp-2);
    border: 1px solid var(--line-2);
    border-radius: var(--r);
    background: var(--surface);
    box-shadow: var(--shadow-3);
    animation: fly-in var(--dur) var(--ease-enter);
  }
  .fly header {
    display: grid;
    gap: 2px;
    padding: 6px 10px var(--sp-2);
    border-bottom: 1px solid var(--line);
    margin-bottom: var(--sp-1);
  }
  .fly header b {
    font-size: var(--fs-md);
    font-weight: var(--fw-semibold);
  }
  .fly header span {
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  @keyframes fly-in {
    from {
      opacity: 0;
      transform: translateX(-6px);
    }
  }
  @media (max-height: 760px) {
    .side {
      padding-top: var(--sp-3);
    }
    .brand {
      margin-bottom: var(--sp-2);
    }
    .inner {
      padding-bottom: 2px;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .pages {
      transition: none;
    }
    .fly {
      animation: none;
    }
  }
</style>
