<script lang="ts">
  import type { Snippet } from "svelte";
  import Logo from "../Logo.svelte";
  import NavItem from "../ui/NavItem.svelte";
  import SourceNote from "../ui/SourceNote.svelte";
  import { arrowNav } from "./keys";
  import { areaLine, navLive, tight } from "./live.svelte";
  import { AREAS, LOCKED_PAGES, PAGES, areaOf, type AreaId, type Page } from "./pages";

  /**
   * Concept B: a slim side bar of five areas. The area on screen stays open with its pages; the
   * others open with a click. The start page hands out the areas as tiles (Hub).
   */
  let {
    tab,
    locked,
    open,
    onSelect,
    foot,
  }: { tab: Page; locked: boolean; open?: AreaId | null; onSelect: (page: Page) => void; foot: Snippet } = $props();

  const current = $derived(areaOf(tab)?.id ?? null);
  /** Areas the player opened besides the current one; the browser demo may open one (`&navopen=`). */
  let opened = $state<Set<AreaId>>(new Set());
  $effect(() => {
    if (open) opened = new Set([open]);
  });
  /** The current area folded by hand. */
  let folded = $state<AreaId | null>(null);
  const isOpen = (id: AreaId) => (id === current ? folded !== id : opened.has(id));
  function toggle(id: AreaId) {
    if (id === current) folded = folded === id ? null : id;
    else {
      const next = new Set(opened);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      opened = next;
    }
  }
  const lockedNow = (p: Page) => locked && LOCKED_PAGES.includes(p);
</script>

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
        <div class="area" class:open={isOpen(area.id)} role="group" aria-labelledby="nav-b-{area.id}">
          <NavItem
            size={area.id === "werkzeuge" ? "md" : "lg"}
            data-nav
            id="nav-b-{area.id}"
            label={area.label}
            icon={area.icon}
            current={current === area.id}
            status={area.id === "werkzeuge" ? undefined : line.text}
            locked={area.id === "werkzeuge" && area.pages.some(lockedNow)}
            tone={line.tone}
            chevron="down"
            aria-expanded={isOpen(area.id)}
            aria-controls="nav-b-{area.id}-pages"
            onclick={() => toggle(area.id)}
          />
          <div class="pages" id="nav-b-{area.id}-pages" inert={!isOpen(area.id)}>
            <div class="inner">
              {#each area.pages as p (p)}
                {@const live = navLive.of(p, locked)}
                <NavItem size="md" data-nav label={PAGES[p].label} icon={PAGES[p].icon} active={tab === p} locked={lockedNow(p)} status={tight(live)} tone={live?.tone} title={PAGES[p].text} onclick={() => onSelect(p)} />
              {/each}
            </div>
          </div>
        </div>
      {/if}
    {/each}
  </nav>

  <footer>
    {#if locked}
      <div class="lock"><SourceNote
          kind="locked"
          label="PokerStars läuft: Teile gesperrt"
          hint="Gesperrt, solange PokerStars läuft. Lernen, Hände spielen und die Werkzeuge öffnen wieder, sobald du PokerStars schließt; HUD und Statistik laufen weiter."
        /></div>
    {/if}
    {@render foot()}
    <div use:arrowNav>
      <NavItem size="sm" data-nav label="Einstellungen" icon={PAGES.settings.icon} active={tab === "settings"} onclick={() => onSelect("settings")} />
    </div>
  </footer>
</aside>

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
  }
</style>
