<script lang="ts">
  import JobsDock from "../jobs/JobsDock.svelte";
  import type { JobTarget } from "../jobs/types";
  import type { Filter, HudConfig, ImportStatus, TournamentRow } from "../types";
  import { navLive } from "./live.svelte";
  import NavA from "./NavA.svelte";
  import NavB from "./NavB.svelte";
  import NavC from "./NavC.svelte";
  import NavMeta from "./NavMeta.svelte";
  import NavNotices from "./NavNotices.svelte";
  import type { AreaId, Concept, Page } from "./pages";

  /** The navigation prototypes of the browser demo (`?nav=a|b|c`) in place of the side bar. */
  let {
    concept,
    tab,
    locked,
    open = null,
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
    concept: Concept;
    tab: Page;
    locked: boolean;
    /** Browser demo: a group or menu shown open for pictures (`&navopen=<area>`). */
    open?: AreaId | null;
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

  /** Narrow top bar while PokerStars runs: the lock note needs the room, the paused tasks show as their ring. */
  const media = window.matchMedia("(max-width: 1279px)");
  let narrow = $state(media.matches);
  $effect(() => {
    const on = () => (narrow = media.matches);
    media.addEventListener("change", on);
    return () => media.removeEventListener("change", on);
  });

  // Browser demo: &navfocus=<n> puts the keyboard focus on the n-th menu entry, for pictures.
  const focusAt = Number(new URLSearchParams(window.location.search).get("navfocus"));
  $effect(() => {
    if (!Number.isInteger(focusAt) || focusAt < 1) return;
    const t = window.setTimeout(() => document.querySelectorAll<HTMLElement>("[data-nav]")[focusAt - 1]?.focus(), 400);
    return () => clearTimeout(t);
  });

  $effect(() => {
    void refresh;
    navLive.load(filter, tournaments, tables, status?.total_hands ?? 0, hud);
  });
</script>

{#snippet foot()}
  <JobsDock {onView} />
  <NavNotices {status} settingsOpen={tab === "settings"} {onChooseFolder} />
  <NavMeta {tables} hands={status?.total_hands ?? 0} />
{/snippet}

{#snippet barStatus()}
  <NavMeta {tables} hands={status?.total_hands ?? 0} layout="bar" />
  <span class="dock" class:ring={narrow && locked}><JobsDock {onView} placement="top" compact={narrow && locked} /></span>
{/snippet}

{#snippet barNotices()}
  <NavNotices {status} settingsOpen={tab === "settings"} layout="bar" {onChooseFolder} />
{/snippet}

{#if concept === "a"}
  <NavA {tab} {locked} open={open === "werkzeuge"} {onSelect} {foot} />
{:else if concept === "b"}
  <NavB {tab} {locked} {open} {onSelect} {foot} />
{:else}
  <NavC {tab} {locked} {open} {onSelect} status={barStatus} notices={barNotices} />
{/if}

<style>
  .dock {
    display: flex;
    width: 190px;
  }
  .dock.ring {
    width: auto;
  }

</style>
