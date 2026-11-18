<script lang="ts">
  import { tick, type Snippet } from "svelte";
  import Logo from "../Logo.svelte";
  import NavItem from "../ui/NavItem.svelte";
  import SourceNote from "../ui/SourceNote.svelte";
  import { arrowNav } from "./keys";
  import { navLive, tight } from "./live.svelte";
  import { AREAS, LOCKED_PAGES, PAGES, type Page } from "./pages";

  /**
   * Concept A: every page stays in the side bar, grouped by intent. Start, Lernen and Hände
   * spielen stand out with a plate and a status line; the tools step back and fold away.
   */
  let {
    tab,
    locked,
    open = false,
    onSelect,
    foot,
  }: { tab: Page; locked: boolean; open?: boolean; onSelect: (page: Page) => void; foot: Snippet } = $props();

  const BIG: Page[] = ["dashboard", "training", "play"];
  const STORE = "ringside.nav.tools";
  let toolsOpen = $state(load());
  function load() {
    try {
      return localStorage.getItem(STORE) === "1";
    } catch {
      return false;
    }
  }
  function toggleTools() {
    toolsOpen = !toolsOpen;
    try {
      localStorage.setItem(STORE, toolsOpen ? "1" : "0");
    } catch {
      // The fold is a convenience; it may forget.
    }
  }
  const tools = AREAS.find((a) => a.id === "werkzeuge")!;
  /** A tool on screen keeps its group open. */
  const showTools = $derived(toolsOpen || open || tools.pages.includes(tab));
  const lockedNow = (p: Page) => locked && LOCKED_PAGES.includes(p);

  /** In low windows the menu scrolls; its end fades out while entries are hidden below. */
  let navEl = $state<HTMLElement>();
  let more = $state(false);
  const measure = () => navEl && (more = navEl.scrollTop + navEl.clientHeight < navEl.scrollHeight - 2);
  $effect(() => {
    if (!navEl) return;
    const ro = new ResizeObserver(measure);
    ro.observe(navEl);
    return () => ro.disconnect();
  });
  /** Opened tools scroll into view in low windows, so the click shows what it opened. */
  let toolsEl = $state<HTMLElement>();
  $effect(() => {
    if (!showTools) return;
    tick().then(() => {
      toolsEl?.scrollIntoView({ block: "nearest" });
      measure();
    });
  });
</script>

<aside class="side" aria-label="Navigation">
  <div class="brand">
    <Logo size={26} />
    <strong>Ringside</strong>
  </div>

  <nav aria-label="Hauptmenü" use:arrowNav bind:this={navEl} class:more onscroll={measure}>
    {#each AREAS as area (area.id)}
      {#if area.id === "werkzeuge"}
        <div class="group quiet" role="group" aria-labelledby="nav-a-{area.id}">
          <NavItem
            size="group"
            id="nav-a-{area.id}"
            label={area.label}
            chevron="down"
            aria-expanded={showTools}
            aria-controls="nav-a-tools"
            status={showTools ? undefined : String(area.pages.length)}
            locked={locked && !showTools}
            data-nav
            onclick={toggleTools}
          />
          {#if showTools}
            <div class="items" id="nav-a-tools" bind:this={toolsEl}>
              {#each area.pages as p (p)}
                {@const live = navLive.of(p, locked)}
                <NavItem size="sm" data-nav label={PAGES[p].label} icon={PAGES[p].icon} active={tab === p} locked={lockedNow(p)} status={tight(live)} tone={live?.tone} onclick={() => onSelect(p)} />
              {/each}
            </div>
          {/if}
        </div>
      {:else if area.id === "start"}
        {@const live = navLive.of("dashboard", locked)}
        <NavItem size="lg" data-nav label="Start" icon={PAGES.dashboard.icon} active={tab === "dashboard"} status={live?.long ?? "Dein letzter Abend"} tone={live?.tone} onclick={() => onSelect("dashboard")} />
      {:else}
        <div class="group" role="group" aria-labelledby="nav-a-{area.id}">
          <span class="group-label" id="nav-a-{area.id}">{area.label}</span>
          {#each area.pages as p (p)}
            {@const live = navLive.of(p, locked)}
            <NavItem
              size={BIG.includes(p) ? "lg" : "md"}
              data-nav
              label={PAGES[p].label}
              icon={PAGES[p].icon}
              active={tab === p}
              locked={lockedNow(p)}
              status={BIG.includes(p) ? (live?.long ?? PAGES[p].short) : tight(live)}
              tone={live?.tone}
              title={PAGES[p].text}
              onclick={() => onSelect(p)}
            />
          {/each}
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
    gap: 10px;
    min-height: 0;
    overflow-y: auto;
  }
  nav.more {
    mask-image: linear-gradient(to bottom, var(--text) calc(100% - 28px), transparent);
  }
  .group,
  .items {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .group-label {
    padding: 0 10px var(--sp-1);
    font-size: var(--fs-xs);
    font-weight: var(--fw-semibold);
    letter-spacing: 0.02em;
    color: var(--text-3);
  }
  /* The tools sit apart, under a line. */
  .quiet {
    padding-top: 10px;
    border-top: 1px solid var(--line);
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
    nav {
      gap: 4px;
    }
    /* Every entry stays visible above the footer: the status lines give way first. */
    nav :global(.ui-nav.lg .status) {
      display: none;
    }
    .group-label {
      padding-bottom: 2px;
    }
    footer {
      gap: var(--sp-1);
      padding-top: var(--sp-2);
    }
  }
</style>
