<script lang="ts">
  import { tick } from "svelte";
  import { STAT_META } from "../../stats";
  import type { HudConfig } from "../../types";
  import ArticleView from "./Article.svelte";
  import { ARTICLE_BY_ID, ARTICLES, LEVELS, UNITS, fold, searchText, type Article } from "./articles";
  import { loadRead, storeRead } from "./read";

  let {
    openId = null,
    onOpenChange,
    hud = null,
  }: { openId?: string | null; onOpenChange?: (id: string | null) => void; hud?: HudConfig | null } = $props();

  let read = $state(loadRead());
  let query = $state("");
  let current = $derived(openId ?? null);
  let root: HTMLElement | undefined = $state();
  let searchEl: HTMLInputElement | undefined = $state();

  const article = $derived(current ? ARTICLE_BY_ID[current] : undefined);

  function saveRead(next: Set<string>) {
    read = next;
    storeRead(next);
  }

  async function open(id: string | null) {
    current = id;
    onOpenChange?.(id);
    if (id && !read.has(id)) saveRead(new Set([...read, id]));
    await tick();
    root?.scrollIntoView({ block: "start" });
  }

  function toggleRead(id: string) {
    const next = new Set(read);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    saveRead(next);
  }

  const terms = $derived(fold(query).split(/\s+/).filter(Boolean));
  const INDEX = ARTICLES.map((a) => ({ a, text: searchText(a) }));
  const hits = $derived(terms.length ? INDEX.filter((x) => terms.every((t) => x.text.includes(t))).map((x) => x.a) : ARTICLES);
  const groups = $derived(UNITS.map((u) => ({ ...u, items: hits.filter((a) => a.unit === u.id) })).filter((g) => g.items.length));
  const readCount = $derived(ARTICLES.filter((a) => read.has(a.id)).length);

  /** A small glyph that hints at the page's main picture. */
  function glyph(a: Article): string {
    const first = a.blocks.find((b) => !["points", "avoid", "note", "formula"].includes(b.kind));
    return first?.kind === "widget" ? first.widget : (first?.kind ?? "formula");
  }
  const ICONS: Record<string, string> = {
    "pot-odds": "M4 16h16M7 16V9M12 16V5M17 16v-4",
    position: "M12 4.5c4.7 0 8.5 3.4 8.5 7.5s-3.8 7.5-8.5 7.5S3.5 16.1 3.5 12 7.3 4.5 12 4.5zM12 19.5v-3",
    outs: "M5 5h6v8H5zM13 9h6v8h-6z",
    "rule-24": "M4 19c5-1 9-6 16-14M4 19c6-3 10-7 16-11",
    combos: "M4 6h5v7H4zM10 6h5v7h-5zM4 15h11M17 6v10",
    stack: "M3.5 12h17M6 9v6M10 10.5v3M14 10.5v3M18 9v6",
    cbet: "M4 8h10M4 12h14M4 16h6",
    table: "M12 6.5c4.7 0 8.5 2.5 8.5 5.5s-3.8 5.5-8.5 5.5S3.5 15 3.5 12 7.3 6.5 12 6.5z",
    cards: "M6 5h7v10H6zM11 9h7v10h-7z",
    matchup: "M3 7h5v8H3zM16 7h5v8h-5zM10 11h4",
    bars: "M4 7h12M4 12h16M4 17h8",
    compare: "M5 19V9M9 19V5M15 19v-7M19 19v-4",
    tree: "M5 12h4M9 12l5-5h5M9 12l5 5h5",
    matrix: "M4 4h16v16H4zM4 9.3h16M4 14.6h16M9.3 4v16M14.6 4v16",
    sample: "M4 8h16M7 8h6M4 16h16M9 16h3",
    formula: "M5 12h14M9 7.5h6M9 16.5h6",
  };

  function onKey(e: KeyboardEvent) {
    const t = e.target as HTMLElement | null;
    const typing = t?.tagName === "INPUT" || t?.tagName === "TEXTAREA";
    if (e.key === "Escape" && current) {
      e.preventDefault();
      open(null);
    } else if (e.key === "/" && !current && !typing) {
      e.preventDefault();
      searchEl?.focus();
    }
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="wissen" bind:this={root}>
  {#if article}
    <nav class="crumbs">
      <button class="btn ghost back" onclick={() => open(null)}>
        <svg width="15" height="15" viewBox="0 0 24 24" aria-hidden="true"><path d="M15 5l-7 7 7 7" /></svg>
        Wissen
      </button>
    </nav>
    {#key article.id}
      <ArticleView {article} {hud} read={read.has(article.id)} onToggleRead={() => toggleRead(article.id)} onOpen={(id) => open(id)} />
    {/key}
  {:else}
    <header class="head">
      <div>
        <h1>Wissen</h1>
        <p>Kurze Seiten mit Bild statt Textwand. Eine Idee pro Seite, in einer Minute gelesen.</p>
      </div>
      <div class="progress" title="{readCount} von {ARTICLES.length} gelesen">
        <span class="num"><b>{readCount}</b> / {ARTICLES.length}</span>
        <span class="faint">gelesen</span>
        <span class="pbar"><span style="width:{(readCount / ARTICLES.length) * 100}%"></span></span>
      </div>
    </header>

    <div class="search">
      <svg width="16" height="16" viewBox="0 0 24 24" aria-hidden="true"><path d="M10.5 17a6.5 6.5 0 1 0 0-13 6.5 6.5 0 0 0 0 13zM15.5 15.5 20 20" /></svg>
      <input
        class="field"
        type="search"
        placeholder="Suchen, z. B. Pot Odds, VPIP, Bubble"
        aria-label="Wissen durchsuchen"
        bind:value={query}
        bind:this={searchEl}
      />
      {#if !query}<kbd>/</kbd>{/if}
    </div>

    {#if !groups.length}
      <div class="empty">Nichts gefunden zu „{query}“.</div>
    {/if}

    {#each groups as g (g.id)}
      <section class="group">
        <h2>
          {g.title}
          <span class="num faint">{g.items.filter((a) => read.has(a.id)).length} / {g.items.length}</span>
        </h2>
        <div class="tiles">
          {#each g.items as a (a.id)}
            {@const kind = glyph(a)}
            <button class="tile" class:done={read.has(a.id)} onclick={() => open(a.id)}>
              <span class="ico" class:stat={!!a.stat}>
                {#if a.stat}
                  <b>{STAT_META[a.stat].short}</b>
                {:else}
                  <svg width="22" height="22" viewBox="0 0 24 24" aria-hidden="true"><path d={ICONS[kind] ?? ICONS.formula} /></svg>
                {/if}
              </span>
              <span class="txt">
                <span class="title">{a.title}</span>
                <span class="one">{a.oneLiner}</span>
                <span class="tags">
                  <span class="lvl" title={LEVELS[a.level]}>
                    {#each [1, 2, 3] as n (n)}<i class:on={n <= a.level}></i>{/each}
                    {LEVELS[a.level]}
                  </span>
                  {#if a.blocks.some((b) => b.kind === "widget")}<span class="tag">Interaktiv</span>{/if}
                </span>
              </span>
              {#if read.has(a.id)}
                <span class="check" aria-label="gelesen">
                  <svg width="12" height="12" viewBox="0 0 24 24" aria-hidden="true"><path d="M5 12.5l4.5 4.5L19 7.5" /></svg>
                </span>
              {/if}
            </button>
          {/each}
        </div>
      </section>
    {/each}
  {/if}
</div>

<style>
  .wissen {
    container: wissen / inline-size;
    display: grid;
    gap: 20px;
    max-width: 1180px;
    scroll-margin-top: 20px;
  }
  svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 1.7;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .crumbs {
    margin: -6px 0 -10px -10px;
  }
  .back {
    gap: 4px;
    padding: 0 10px;
  }
  .head {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 20px;
  }
  .head h1 {
    margin: 0;
    font-family: var(--font-num);
    font-size: 26px;
    font-weight: 600;
  }
  .head p {
    margin: 3px 0 0;
    color: var(--text-2);
  }
  .progress {
    display: grid;
    grid-template-columns: auto auto;
    align-items: baseline;
    gap: 2px 8px;
    min-width: 150px;
    font-size: 13px;
  }
  .progress b {
    font-size: 20px;
    font-weight: 600;
  }
  .pbar {
    grid-column: 1 / -1;
    height: 4px;
    border-radius: 99px;
    background: var(--surface-3);
    overflow: hidden;
  }
  .pbar span {
    display: block;
    height: 100%;
    background: var(--text);
  }
  .search {
    position: relative;
    max-width: 460px;
  }
  .search svg {
    position: absolute;
    left: 11px;
    top: 50%;
    transform: translateY(-50%);
    color: var(--text-3);
    pointer-events: none;
  }
  .search .field {
    width: 100%;
    height: 38px;
    padding: 0 36px;
    font-size: 14px;
    background: var(--surface);
  }
  kbd {
    position: absolute;
    right: 10px;
    top: 50%;
    transform: translateY(-50%);
    padding: 0 6px;
    border-radius: 4px;
    border: 1px solid var(--line-2);
    color: var(--text-3);
    font-family: var(--font-num);
    font-size: 12px;
  }
  .group h2 {
    display: flex;
    align-items: baseline;
    gap: 10px;
    margin: 6px 0 10px;
    font-size: 14px;
    font-weight: 600;
  }
  .group h2 span {
    font-size: 12.5px;
    font-weight: 400;
  }
  .tiles {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(250px, 1fr));
    gap: 10px;
  }
  .tile {
    position: relative;
    display: grid;
    align-items: start;
    grid-template-columns: 44px minmax(0, 1fr);
    gap: 12px;
    padding: 14px 14px 12px;
    border-radius: var(--r);
    border: 1px solid var(--line);
    background: var(--surface);
    color: var(--text);
    text-align: left;
    cursor: pointer;
    transition:
      background 0.12s,
      border-color 0.12s;
  }
  .tile:hover {
    background: var(--surface-2);
    border-color: var(--line-2);
  }
  .ico {
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    border-radius: 9px;
    background: var(--surface-3);
    color: var(--text);
  }
  .ico.stat {
    background: var(--plate);
    box-shadow: inset 0 0 0 1px var(--plate-edge);
  }
  .ico b {
    font-family: var(--font-num);
    font-size: 14px;
    font-weight: 600;
    letter-spacing: 0.01em;
  }
  .txt {
    display: grid;
    align-content: start;
    gap: 3px;
    min-width: 0;
  }
  .title {
    padding-right: 18px;
    font-size: 14.5px;
    font-weight: 600;
  }
  .one {
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    align-self: start;
    max-height: calc(2 * 1.4em);
    color: var(--text-2);
    font-size: 13px;
    line-height: 1.4;
  }
  .tags {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 10px;
    margin-top: 5px;
    color: var(--text-3);
    font-size: 11.5px;
  }
  .lvl {
    display: inline-flex;
    align-items: center;
    gap: 3px;
  }
  .lvl i {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: var(--surface-3);
  }
  .lvl i.on {
    background: var(--text-2);
  }
  .lvl i:last-of-type {
    margin-right: 4px;
  }
  .tag {
    padding: 0 7px;
    border-radius: 99px;
    border: 1px solid var(--line-2);
  }
  .check {
    position: absolute;
    top: 12px;
    right: 12px;
    display: grid;
    place-items: center;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: var(--text);
    color: var(--bg);
  }
  .check svg {
    stroke-width: 2.8;
  }
  .tile.done .title {
    color: var(--text-2);
  }
</style>
