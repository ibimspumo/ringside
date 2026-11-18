<script lang="ts">
  import { api } from "../api";
  import type { LearnTarget } from "../app/Leaks.svelte";
  import PlayingCard from "../PlayingCard.svelte";
  import type { Filter } from "../types";
  import { int } from "../app/format";
  import SourceNote from "../ui/SourceNote.svelte";
  import { EV_LOSS_EXPLAIN, KIT_SOURCE, LEARN, RATING, RATINGS, SOURCE, SPOT, SPOT_HINT, bb, reliabilityText } from "./labels";
  import { analysis } from "./store.svelte";
  import type { AnalysisSummary, SpotType } from "./types";

  /**
   * What the analysis found, as three things to work on: the spot types that cost the most,
   * each with an exercise or article, then the costliest single decisions.
   */
  let {
    filter,
    refresh,
    onOpen,
    onSpot,
    onLearn,
    strip = false,
  }: {
    filter: Filter;
    refresh: number;
    onOpen: (handId: number, actionIndex: number) => void;
    onSpot?: (spot: SpotType) => void;
    onLearn?: (target: LearnTarget) => void;
    /** One low band above a list that already shows the costliest hands. */
    strip?: boolean;
  } = $props();

  let data = $state<AnalysisSummary | null>(null);

  $effect(() => {
    void refresh;
    void analysis.solved;
    void analysis.status?.counts.analyzed;
    const f = { ...filter };
    let live = true;
    api.analysisSummary(f).then(
      (s) => live && (data = s),
      () => {},
    );
    return () => (live = false);
  });

  const focus = $derived((data?.by_spot ?? []).filter((s) => s.spot && s.loss_bb > 0.5).slice(0, 3));
  const flawed = $derived(data ? data.ratings.slice(RATINGS.indexOf("inaccuracy")).reduce((a, b) => a + b, 0) : 0);
  const cards = (s: string) => s.split(" ").filter(Boolean);
</script>

{#if data && data.rated > 0 && strip}
  <section class="band">
    <span class="lead"><abbr title={EV_LOSS_EXPLAIN}>Verlust durch Fehler</abbr> <b class="num">{bb(data.loss_bb)}</b></span>
    {#if focus.length}
      <span class="faint">Baustellen:</span>
      {#each focus as f (f.spot)}
        {@const spot = f.spot!}
        <span class="pair">
          <button class="name-chip" onclick={() => onSpot?.(spot)} title="{SPOT_HINT[spot]} Klick zeigt nur diese Hände.">
            {SPOT[spot]} <b class="num">−{bb(f.loss_bb, 0)}</b>
          </button>
          {#if onLearn}
            <button class="learn" onclick={() => onLearn(LEARN[spot][0].target)} title={LEARN[spot][0].label}>Üben</button>
          {/if}
        </span>
      {/each}
    {/if}
  </section>
{:else if data && data.rated > 0}
  <section class="summary">
    <div class="totals">
      <div class="big">
        <span class="label"><abbr title={EV_LOSS_EXPLAIN}>Verlust durch Fehler</abbr></span>
        <b class="num">{bb(data.loss_bb)}</b>
        <span class="sub num">in {int(data.rated)} bewerteten Entscheidungen, {int(flawed)} davon mindestens ungenau</span>
      </div>
      <div class="dist" role="img" aria-label="Bewertungen">
        {#each RATINGS as r, i (r)}
          {#if data.ratings[i] > 0}
            <span style="flex-grow:{data.ratings[i]}; background:{RATING[r].color}" title="{RATING[r].label}: {int(data.ratings[i])}"></span>
          {/if}
        {/each}
      </div>
      <p class="sources">
        {#each data.by_source as [src, n] (src)}
          <span title={SOURCE[src].explain}>{SOURCE[src].label} <b class="num">{int(n)}</b></span>
        {/each}
      </p>
    </div>

    {#if focus.length}
      <div class="focus">
        <h3>Deine {focus.length === 1 ? "wichtigste Baustelle" : `${focus.length} wichtigsten Baustellen`}</h3>
        <ol>
          {#each focus as f (f.spot)}
            {@const spot = f.spot!}
            <li>
              <button class="name" onclick={() => onSpot?.(spot)} title="{SPOT_HINT[spot]} Klick zeigt nur diese Hände.">
                <b>{SPOT[spot]}</b>
                <span class="num">kostet {bb(f.loss_bb)} · {int(f.flawed)} von {int(f.rated)} ungenau oder schlechter</span>
              </button>
              <span class="hint">{SPOT_HINT[spot]}</span>
              {#if onLearn}
                <span class="links">
                  {#each LEARN[spot] as l (l.label)}
                    <button class="btn ghost" onclick={() => onLearn(l.target)}>{l.label}</button>
                  {/each}
                </span>
              {/if}
            </li>
          {/each}
        </ol>
      </div>
    {/if}

    {#if data.costliest.length}
      <div class="costly">
        <h3>Teuerste Entscheidungen</h3>
        {#each data.costliest.slice(0, 3) as c (c.hand_id + ":" + c.action_index)}
          <button class="hand" onclick={() => onOpen(c.hand_id, c.action_index)}>
            <span class="hole">{#each cards(c.hero_cards) as card (card)}<PlayingCard {card} size={18} />{/each}</span>
            <span class="what">{c.played} statt {c.best}</span>
            {#if c.source && KIT_SOURCE[c.source]}
              <SourceNote kind={KIT_SOURCE[c.source]!} hint={c.source === "estimate" && c.reliability !== null ? reliabilityText(c.reliability, c.street) : SOURCE[c.source].explain} />
            {:else}
              <span></span>
            {/if}
            <b class="num" style="color:{c.rating ? RATING[c.rating].color : 'inherit'}">−{bb(c.ev_loss_bb ?? 0, 2)}</b>
          </button>
        {/each}
      </div>
    {/if}
  </section>
{/if}

<style>
  .summary {
    display: grid;
    grid-template-columns: minmax(200px, 0.8fr) minmax(0, 1.6fr) minmax(220px, 1fr);
    gap: 14px 26px;
    padding: 14px 18px;
    border: 1px solid var(--line);
    border-radius: var(--r-lg);
    background: var(--surface);
    font-size: 13px;
  }
  h3 {
    margin: 0 0 8px;
    font-size: 12.5px;
    font-weight: 600;
    color: var(--text-2);
  }
  .totals {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .big {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .label {
    color: var(--text-2);
    font-size: 12.5px;
  }
  abbr {
    text-decoration: underline dotted;
    text-underline-offset: 3px;
    cursor: help;
  }
  .big b {
    font-family: var(--font-num);
    font-size: 26px;
    font-weight: 600;
    color: var(--neg);
  }
  .sub {
    color: var(--text-3);
    font-size: 12px;
  }
  .dist {
    display: flex;
    gap: 2px;
    height: 6px;
  }
  .dist span {
    flex-basis: 0;
    min-width: 3px;
    border-radius: 2px;
  }
  .sources {
    display: flex;
    flex-wrap: wrap;
    gap: 2px 12px;
    margin: 0;
    font-size: 12px;
    color: var(--text-3);
  }
  .sources span {
    cursor: help;
  }
  .sources b {
    font-weight: 500;
    color: var(--text-2);
  }
  ol {
    display: grid;
    gap: 10px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  li {
    display: grid;
    gap: 2px;
  }
  .name {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 2px 10px;
    padding: 0;
    border: 0;
    background: none;
    text-align: left;
    color: var(--text);
    cursor: pointer;
  }
  .name b {
    font-size: 14px;
    font-weight: 600;
  }
  .name:hover b {
    text-decoration: underline;
    text-underline-offset: 3px;
  }
  .name span {
    color: var(--neg);
    font-size: 12.5px;
  }
  .hint {
    color: var(--text-3);
    font-size: 12px;
  }
  .links {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin-top: 2px;
  }
  .links .btn {
    height: 26px;
    padding: 0 9px;
    font-size: 12px;
  }
  .costly {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .hand {
    display: grid;
    grid-template-columns: 40px minmax(0, 1fr) auto auto;
    align-items: center;
    gap: 8px;
    padding: 5px 6px;
    border: 0;
    border-radius: var(--r-sm);
    background: transparent;
    text-align: left;
    color: var(--text-2);
    cursor: pointer;
  }
  .hand:hover {
    background: var(--surface-2);
    color: var(--text);
  }
  .hole {
    display: flex;
    gap: 2px;
  }
  .what {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .band {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 10px;
    font-size: 13px;
    color: var(--text-2);
  }
  .lead b {
    margin-left: 4px;
    font-family: var(--font-num);
    font-size: 16px;
    font-weight: 600;
    color: var(--neg);
  }
  .lead {
    margin-right: 6px;
  }
  .pair {
    display: inline-flex;
    border: 1px solid var(--line-2);
    border-radius: 999px;
    overflow: hidden;
  }
  .pair button {
    height: 26px;
    padding: 0 10px;
    border: 0;
    background: transparent;
    color: var(--text);
    font-size: 12.5px;
    cursor: pointer;
  }
  .pair button:hover {
    background: var(--surface-2);
  }
  .name-chip b {
    margin-left: 3px;
    font-weight: 500;
    color: var(--neg);
  }
  .pair .learn {
    border-left: 1px solid var(--line-2);
    color: var(--text-2);
  }
  @container (max-width: 1100px) {
    .summary {
      grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    }
    .focus {
      grid-column: 1 / -1;
      grid-row: 2;
    }
  }
</style>
