<script lang="ts">
  import { inTauri } from "../api";
  import { pokerClient } from "../app/client.svelte";
  import JobsDock from "../jobs/JobsDock.svelte";
  import type { JobTarget } from "../jobs/types";
  import Logo from "../Logo.svelte";
  import type { Filter, HudConfig, ImportStatus, TournamentRow } from "../types";
  import IconButton from "../ui/IconButton.svelte";
  import NavItem from "../ui/NavItem.svelte";
  import { analysis } from "../analysis/store.svelte";
  import SkillChip from "../skill/SkillChip.svelte";
  import { loadSkill } from "../skill/store.svelte";
  import SourceNote from "../ui/SourceNote.svelte";
  import { arrowNav } from "./keys";
  import { navLive, tight } from "./live.svelte";
  import NavMeta from "./NavMeta.svelte";
  import { AREAS, LOCKED_PAGES, PAGES, type Page } from "./pages";

  /**
   * The app's navigation: a side bar with every page under its category, the rating at its head
   * and what Ringside is doing at its foot. Below 1240 px it folds to a rail of icons whose names
   * and values are in their tooltips.
   */
  let {
    tab,
    filter,
    tournaments,
    tables,
    status,
    hud,
    refresh,
    onSelect,
    onView,
  }: {
    tab: Page;
    filter: Filter;
    tournaments: TournamentRow[];
    tables: number;
    status: ImportStatus | null;
    hud: HudConfig | null;
    refresh: number;
    onSelect: (page: Page) => void;
    onView: (target: JobTarget) => void;
  } = $props();

  $effect(() => {
    void refresh;
    navLive.load(filter, tournaments, tables, status?.total_hands ?? 0, hud);
  });
  // The rating follows new analysis and the client: locked while a poker client runs.
  $effect(() => {
    void [refresh, analysis.solved, pokerClient.running];
    loadSkill();
  });

  // Browser demo only: &rail=1 shows the rail at any width, &navmark=<text> a mark of any length on
  // „Hände“, for pictures and the layout check in e2e/layout.mjs.
  const preview = inTauri ? new URLSearchParams() : new URLSearchParams(window.location.search);
  const forceRail = preview.get("rail") === "1";
  const longMark = preview.get("navmark");
  const narrow = window.matchMedia("(max-width: 1239px)");
  let fold = $state(narrow.matches);
  $effect(() => {
    const change = () => (fold = narrow.matches);
    narrow.addEventListener("change", change);
    return () => narrow.removeEventListener("change", change);
  });
  const rail = $derived(forceRail || fold);

  const locked = $derived(pokerClient.running === true);
  const lockedNow = (p: Page) => locked && LOCKED_PAGES.includes(p);
  const size = $derived(rail ? "rail" : "side");

  const GG_HINT = "GGPoker erlaubt kein Programm neben seinem Client: beende Ringside, solange du dort spielst.";
  const LOCK_HINT =
    "Gesperrt, solange ein Poker-Client läuft. Lernen, Hände spielen, Schwachstellen und die Werkzeuge öffnen wieder, sobald du den Poker-Client schließt; HUD und Statistik laufen weiter.";
</script>

<aside class="side" class:rail style:--side-now={rail ? "var(--side-rail-w)" : "var(--side-w)"}>
  <div class="head">
    <div class="brand" title={rail ? "Ringside" : undefined}>
      <Logo size={rail ? 26 : 24} />
      {#if !rail}<strong>Ringside</strong>{/if}
    </div>
    <SkillChip active={tab === "skill"} compact={rail} onclick={() => onSelect("skill")} />
  </div>

  <nav aria-label="Hauptmenü" use:arrowNav>
    {#each AREAS as area (area.id)}
      {@const many = area.pages.length > 1}
      <div class="group" role="group" aria-label={area.label}>
        {#if many}
          {#if rail}<span class="rule" aria-hidden="true"></span>{:else}<span class="heading" aria-hidden="true">{area.label}</span>{/if}
        {/if}
        {#each area.pages as p (p)}
          {@const live = longMark && p === "hands" ? { text: longMark, long: longMark, tone: "warn" as const } : navLive.of(p, locked)}
          <NavItem
            data-nav
            {size}
            label={PAGES[p].label}
            icon={PAGES[p].icon}
            active={tab === p}
            locked={lockedNow(p)}
            status={tight(live)}
            tone={live?.tone}
            title={live?.long && live.long !== tight(live) ? `${PAGES[p].text} · ${live.long}` : PAGES[p].text}
            onclick={() => onSelect(p)}
          />
        {/each}
      </div>
    {/each}
  </nav>

  <div class="foot">
    {#if locked}
      <span class="lock">
        <SourceNote kind="locked" hint={pokerClient.name === "GGPoker" ? GG_HINT : LOCK_HINT} label={rail ? "" : `${pokerClient.name ?? "Poker-Client"} läuft`} />
      </span>
    {/if}
    <NavMeta {tables} hands={status?.total_hands ?? 0} {rail} />
    <div class="tools">
      <span class="dock"><JobsDock {onView} compact={rail} /></span>
      <IconButton
        data-nav
        label={PAGES.settings.label}
        variant={tab === "settings" ? "secondary" : "quiet"}
        aria-current={tab === "settings" ? "page" : undefined}
        onclick={() => onSelect("settings")}
      >
        <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true"><path d={PAGES.settings.icon} /></svg>
      </IconButton>
    </div>
  </div>
</aside>

<style>
  .side {
    display: flex;
    flex-direction: column;
    flex-shrink: 0;
    width: var(--side-now);
    height: 100vh;
    border-right: 1px solid var(--line);
    background: var(--surface-sunken);
  }
  .head {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: var(--sp-3) var(--sp-3) var(--sp-2);
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 32px;
    padding-left: var(--sp-2);
  }
  .brand strong {
    font-family: var(--font-num);
    font-size: var(--fs-lg);
    font-weight: var(--fw-semibold);
    letter-spacing: 0.02em;
  }
  nav {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: var(--sp-1) var(--sp-3) var(--sp-3);
    scrollbar-width: thin;
  }
  .group {
    display: flex;
    flex-direction: column;
  }
  .group + .group {
    margin-top: 10px;
  }
  .heading {
    padding: 0 var(--sp-3) 2px;
    font-size: var(--fs-sm);
    font-weight: var(--fw-semibold);
    color: var(--text-3);
  }
  .foot {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    padding: var(--sp-3);
    border-top: 1px solid var(--line);
  }
  .tools {
    display: flex;
    align-items: center;
    gap: var(--sp-1);
  }
  .dock {
    flex: 1;
    min-width: 0;
  }
  .tools svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 1.6;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .rail .tools {
    flex-direction: column;
    gap: var(--sp-2);
  }
  .lock {
    padding: 0 var(--sp-3);
  }

  .rail .head {
    align-items: center;
    gap: var(--sp-3);
    padding: var(--sp-3) var(--sp-2) var(--sp-2);
  }
  .rail .brand {
    justify-content: center;
    padding: 0;
  }
  .rail nav,
  .rail .foot {
    align-items: center;
    padding-inline: var(--sp-2);
  }
  .rail .group {
    align-items: center;
  }
  .rail .group + .group {
    margin-top: var(--sp-2);
  }
  .rule {
    width: 24px;
    height: 1px;
    margin-bottom: var(--sp-2);
    background: var(--line-2);
  }
  .rail .lock {
    padding: 0;
  }
</style>
