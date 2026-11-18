<script lang="ts">
  import Card from "../ui/Card.svelte";
  import { arrowNav } from "./keys";
  import { areaLine, navLive } from "./live.svelte";
  import { AREAS, PAGES, type Area, type Page } from "./pages";

  /**
   * Concept B's start page: the four areas as one flat row of tiles above the answer, each with
   * what calls for attention there. A click opens the page that value is about.
   */
  let { locked, onSelect }: { locked: boolean; onSelect: (page: Page) => void } = $props();

  const areas = AREAS.filter((a) => a.id !== "start");
  /** The page a tile leads to: the one with something to do, else the first. */
  function target(area: Area): Page {
    return area.pages.find((p) => (navLive.of(p, locked)?.tone ?? "neutral") !== "neutral") ?? area.pages[0];
  }
</script>

<section class="hub" aria-label="Bereiche" use:arrowNav>
  {#each areas as area (area.id)}
    {@const line = areaLine(area, locked)}
    {@const to = target(area)}
    <Card pad="sm" data-nav title="{area.text} Öffnet {PAGES[to].label}." onclick={() => onSelect(to)}>
      <span class="tile">
        <span class="plate"><svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true"><path d={area.icon} /></svg></span>
        <span class="text">
          <b>{area.label}</b>
          <span class="line {line.tone}">{line.text}</span>
        </span>
      </span>
    </Card>
  {/each}
</section>

<style>
  .hub {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: var(--sp-3);
    max-width: 1360px;
    margin-inline: auto;
    padding: var(--sp-6) 30px 0;
  }
  .tile {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    min-width: 0;
  }
  .plate {
    display: grid;
    place-items: center;
    flex-shrink: 0;
    width: 36px;
    height: 36px;
    border-radius: var(--r-md);
    background: var(--surface-2);
    color: var(--text);
  }
  .plate svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 1.6;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .text {
    display: grid;
    gap: 2px;
    min-width: 0;
    text-align: left;
  }
  b {
    font-size: var(--fs-base);
    font-weight: var(--fw-semibold);
  }
  .line {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
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
</style>
