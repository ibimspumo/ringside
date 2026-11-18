<script lang="ts">
  import { tick, type Snippet } from "svelte";
  import Logo from "../Logo.svelte";
  import IconButton from "../ui/IconButton.svelte";
  import NavItem from "../ui/NavItem.svelte";
  import SourceNote from "../ui/SourceNote.svelte";
  import { arrowNav } from "./keys";
  import { areaLine, navLive } from "./live.svelte";
  import { AREAS, LOCKED_PAGES, PAGES, areaOf, type AreaId, type Page } from "./pages";

  /**
   * Concept C: a top bar with the five intents, like a website. Each opens a large menu with
   * every page of the area, a sentence and a live value. Below the bar the pages of the current
   * area stay one click away, so the menu is only needed to change area.
   */
  let {
    tab,
    locked,
    open: preview = null,
    onSelect,
    status,
    notices,
  }: {
    tab: Page;
    locked: boolean;
    open?: AreaId | null;
    onSelect: (page: Page) => void;
    /** Jobs, open tables and stored hands, at the right of the bar. */
    status: Snippet;
    /** Notes across the window under the bar. */
    notices: Snippet;
  } = $props();

  const current = $derived(areaOf(tab) ?? null);
  let open = $state<AreaId | null>(null);
  $effect(() => {
    if (preview) open = preview;
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
  /** Hover opens after a short rest; moving along the bar switches at once. */
  function rest(id: AreaId) {
    clearTimeout(timer);
    if (AREAS.find((a) => a.id === id)!.pages.length < 2) return;
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
      if (open) open = AREAS.find((a) => a.id === next)!.pages.length > 1 ? next : null;
    } else if (e.key === "ArrowDown" && AREAS[i].pages.length > 1) {
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
</script>

<svelte:window onkeydown={key} onpointerdown={outside} />

<div class="top" role="banner" onpointerleave={leave} onpointerenter={() => open && clearTimeout(timer)}>
  <div class="bar">
    <div class="brand">
      <Logo size={24} />
      <strong>Ringside</strong>
    </div>
    <nav aria-label="Hauptmenü" class="intents">
      {#each AREAS as area (area.id)}
        {@const single = area.pages.length === 1}
        <NavItem
          size="top"
          label={area.label}
          icon={area.icon}
          active={single && tab === area.pages[0]}
          current={!single && current?.id === area.id}
          chevron={single ? undefined : "down"}
          aria-haspopup={single ? undefined : "true"}
          aria-expanded={single ? undefined : open === area.id}
          aria-controls={single ? undefined : "nav-c-menu"}
          bind:el={triggers[area.id]}
          onpointerenter={() => rest(area.id)}
          onkeydown={(e) => triggerKey(e, area.id)}
          onclick={() => (single ? go(area.pages[0]) : open === area.id ? close() : show(area.id))}
        />
      {/each}
    </nav>
    <div class="right">
      {@render status()}
      <span class="gear" class:on={tab === "settings"}>
        <IconButton label="Einstellungen" onclick={() => go("settings")}>
          <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true"><path d={PAGES.settings.icon} /></svg>
        </IconButton>
      </span>
    </div>
  </div>

  {#if current && current.pages.length > 1}
    <nav class="sub" aria-label={current.label} use:arrowNav>
      <span class="sub-area">{current.label}</span>
      {#each current.pages as p (p)}
        {@const live = navLive.of(p, locked)}
        <span class="sub-item">
          <NavItem size="sm" data-nav label={PAGES[p].label} icon={PAGES[p].icon} active={tab === p} locked={lockedNow(p)} status={live?.text} tone={live?.tone} title={PAGES[p].text} onclick={() => go(p)} />
        </span>
      {/each}
    </nav>
  {/if}

  {#if menu}
    {@const line = areaLine(menu, locked)}
    <div class="menu" id="nav-c-menu" role="region" aria-label={menu.label} bind:this={panel} use:arrowNav>
      <div class="menu-inner">
        <div class="intro">
          <span class="plate"><svg viewBox="0 0 24 24" width="22" height="22" aria-hidden="true"><path d={menu.icon} /></svg></span>
          <b>{menu.label}</b>
          <p>{menu.text}</p>
          {#if locked && menu.pages.some(lockedNow)}
            <SourceNote kind="locked" />
          {:else if line.tone !== "neutral" || line.text !== menu.short}<span class="line {line.tone}">{line.text}</span>{/if}
        </div>
        <div class="tiles" class:three={menu.pages.length === 3}>
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
{@render notices()}

<style>
  .top {
    position: relative;
    z-index: var(--z-dock);
    background: var(--surface-sunken);
    border-bottom: 1px solid var(--line);
  }
  /* Bar and row carry their own ground, so the scrim behind the open menu leaves them bright. */
  .bar,
  .sub {
    position: relative;
    background: var(--surface-sunken);
  }
  .bar {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
    height: 56px;
    padding: 0 var(--sp-4) 0 var(--sp-5);
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-right: var(--sp-4);
  }
  .brand strong {
    font-family: var(--font-num);
    font-size: var(--fs-lg);
    font-weight: var(--fw-semibold);
    letter-spacing: 0.02em;
  }
  .intents {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 2px;
    min-width: 0;
  }
  .right {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    margin-left: auto;
    min-width: 0;
  }
  .gear.on {
    border-radius: var(--r-sm);
    background: var(--surface-2);
  }

  /* The pages of the current area, one click away. */
  .sub {
    display: flex;
    align-items: center;
    gap: 2px;
    height: 40px;
    padding: 0 var(--sp-4) 0 var(--sp-5);
    border-top: 1px solid var(--line);
    overflow-x: auto;
    scrollbar-width: none;
  }
  .sub-area {
    margin-right: var(--sp-2);
    padding-right: var(--sp-3);
    border-right: 1px solid var(--line-2);
    font-size: var(--fs-xs);
    font-weight: var(--fw-semibold);
    letter-spacing: 0.02em;
    color: var(--text-3);
    white-space: nowrap;
  }
  .sub-item {
    flex-shrink: 0;
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
    max-width: 1360px;
    margin-inline: auto;
    padding: var(--sp-5) 30px var(--sp-6);
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
    .bar {
      gap: var(--sp-3);
    }
    .brand strong {
      display: none;
    }
    .brand {
      margin-right: 0;
    }
    .menu-inner {
      grid-template-columns: 200px minmax(0, 1fr);
      gap: var(--sp-4);
      padding-inline: var(--sp-5);
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
