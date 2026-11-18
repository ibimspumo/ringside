<script lang="ts">
  import NavItem from "../ui/NavItem.svelte";
  import { arrowNav } from "./keys";
  import { areaLine, navLive } from "./live.svelte";
  import { AREAS, LOCKED_PAGES, PAGES, type Page } from "./pages";

  /** Concept B's start page: the four areas as large tiles, each with its pages and live values. */
  let { locked, onSelect }: { locked: boolean; onSelect: (page: Page) => void } = $props();

  const areas = AREAS.filter((a) => a.id !== "start");
</script>

<section class="hub" aria-label="Bereiche" use:arrowNav>
  {#each areas as area (area.id)}
    {@const line = areaLine(area, locked)}
    <article class="area" aria-labelledby="hub-{area.id}">
      <header>
        <span class="plate"><svg viewBox="0 0 24 24" width="22" height="22" aria-hidden="true"><path d={area.icon} /></svg></span>
        <div class="head">
          <h2 id="hub-{area.id}">{area.label}</h2>
          <p>{area.text}</p>
        </div>
      </header>
      {#if line.text !== area.short}<p class="line {line.tone}">{line.text}</p>{/if}
      <div class="pages">
        {#each area.pages as p (p)}
          {@const live = navLive.of(p, locked)}
          <NavItem
            size="md"
            data-nav
            label={PAGES[p].label}
            icon={PAGES[p].icon}
            locked={locked && LOCKED_PAGES.includes(p)}
            status={live?.long}
            tone={live?.tone}
            title={PAGES[p].text}
            onclick={() => onSelect(p)}
          />
        {/each}
      </div>
    </article>
  {/each}
</section>

<style>
  .hub {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: var(--sp-4);
    max-width: 1360px;
    margin-inline: auto;
    padding: 26px 30px 0;
  }
  .area {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    padding: var(--sp-4) var(--sp-3) var(--sp-3);
    border: 1px solid var(--line);
    border-radius: var(--r);
    background: var(--surface);
  }
  header {
    display: flex;
    gap: var(--sp-3);
    padding: 0 var(--sp-1);
  }
  .plate {
    display: grid;
    place-items: center;
    flex-shrink: 0;
    width: 40px;
    height: 40px;
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
  .head {
    display: grid;
    gap: 2px;
    min-width: 0;
  }
  h2 {
    margin: 0;
    font-size: var(--fs-lg);
    font-weight: var(--fw-semibold);
  }
  .head p {
    margin: 0;
    font-size: var(--fs-sm);
    line-height: var(--lh-base);
    color: var(--text-2);
  }
  .line {
    margin: 0;
    padding: 0 var(--sp-1);
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
  .pages {
    display: flex;
    flex-direction: column;
    gap: 1px;
    padding-top: var(--sp-2);
    border-top: 1px solid var(--line);
  }
  @container (max-width: 1099px) {
    .hub {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }
</style>
