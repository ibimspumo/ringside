<script lang="ts">
  import { api } from "../api";
  import type { LearnTarget } from "../app/Leaks.svelte";
  import PlayingCard from "../PlayingCard.svelte";
  import type { Filter } from "../types";
  import { int } from "../app/format";
  import Term from "../app/Term.svelte";
  import { EV_LOSS_EXPLAIN, LEARN, RATING, RATING_RULE, RATINGS, SOURCE, SPOT, SPOT_HINT, bb } from "./labels";
  import { analysis } from "./store.svelte";
  import type { AnalysisSummary, Rating, SpotType } from "./types";

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
  const flawed = $derived(data ? data.ratings.slice(2).reduce((a, b) => a + b, 0) : 0);
  const cards = (s: string) => s.split(" ").filter(Boolean);
</script>

{#snippet mark(r: Rating)}
  <svg class="mark" viewBox="0 0 24 24" width="13" height="13" aria-hidden="true">
    {#if r === "best"}
      <path d="M2 12.5l4.5 4.5L15 8.5M11.5 16l1 1L22 7.5" />
    {:else if r === "good"}
      <path d="M5 12.5l4.5 4.5L19 7.5" />
    {:else if r === "inaccuracy"}
      <circle cx="12" cy="12" r="8.5" /><path d="M12 11v5.5M12 7.8v.2" />
    {:else if r === "mistake"}
      <circle cx="12" cy="12" r="8.5" /><path d="M12 7v6M12 16.2v.2" />
    {:else}
      <path d="M12 3.5l9.5 16.5h-19z" /><path d="M12 10v4.5M12 17.2v.2" />
    {/if}
  </svg>
{/snippet}

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
  {@const top = focus[0]?.loss_bb ?? 1}
  <section class="summary">
    <div class="totals">
      <span class="label"><Term tip={EV_LOSS_EXPLAIN}>Verlust durch Fehler</Term></span>
      <b class="big num">{bb(data.loss_bb)}</b>
      <p class="mean">
        So viel haben dich Entscheidungen gekostet, die besser gegangen wären – auf lange Sicht gerechnet. Das ist dein Spielraum.
      </p>
      <h4 class="levels-head">
        <Term tip={RATING_RULE}>So wurden deine {int(data.rated)} Entscheidungen bewertet</Term>
      </h4>
      <ul class="levels">
        {#each RATINGS as r, i (r)}
          {@const share = data.ratings[i] / Math.max(data.rated, 1)}
          <li style="--c:{RATING[r].color}">
            <span class="lvl">{@render mark(r)}{RATING[r].label}</span>
            <span class="lvl-bar" aria-hidden="true"><i style="width:{data.ratings[i] ? Math.max(1.5, share * 100) : 0}%"></i></span>
            <b class="num">{int(data.ratings[i])}</b>
          </li>
        {/each}
      </ul>
      <p class="flawed num">{int(flawed)} davon ungenau oder schlechter</p>
      <p class="sources">
        <span class="faint">Bewertet mit</span>
        {#each data.by_source as [src, n] (src)}
          <span><Term tip={SOURCE[src].explain}>{SOURCE[src].label}</Term> <b class="num">{int(n)}</b></span>
        {/each}
      </p>
    </div>

    {#if focus.length}
      <div class="focus">
        <h3>Fang hier an: {focus.length === 1 ? "deine wichtigste Baustelle" : `deine ${focus.length} wichtigsten Baustellen`}</h3>
        <ol>
          {#each focus as f, i (f.spot)}
            {@const spot = f.spot!}
            <li>
              <span class="rank num">{i + 1}</span>
              <div class="body">
                <div class="row">
                  <b class="spot">{SPOT[spot]}</b>
                  <b class="cost num">kostet {bb(f.loss_bb)}</b>
                </div>
                <span class="meter" aria-hidden="true"><i style="width:{Math.max(4, (f.loss_bb / top) * 100)}%"></i></span>
                <span class="hint">
                  {SPOT_HINT[spot]} <span class="num">{int(f.flawed)} von {int(f.rated)}</span> Entscheidungen waren ungenau oder schlechter.
                </span>
                <span class="links">
                  {#if onLearn}
                    {#each LEARN[spot] as l, j (l.label)}
                      <button class="btn {j === 0 ? 'go' : 'ghost'}" onclick={() => onLearn(l.target)}>{l.label}</button>
                    {/each}
                  {/if}
                  {#if onSpot}
                    <button class="btn ghost" onclick={() => onSpot(spot)}>Hände ansehen</button>
                  {/if}
                </span>
              </div>
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
            <span class="hole">{#each cards(c.hero_cards) as card (card)}<PlayingCard {card} size={20} />{/each}</span>
            <span class="what">
              <span>{c.played} statt {c.best}</span>
              <small>{c.position ? `${c.position} · ` : ""}Hand ansehen</small>
            </span>
            <span class="loss" style="--c:{c.rating ? RATING[c.rating].color : 'var(--text)'}">
              <b class="num">−{bb(c.ev_loss_bb ?? 0, 2)}</b>
              {#if c.rating}<small>{@render mark(c.rating)}{RATING[c.rating].label}</small>{/if}
            </span>
          </button>
        {/each}
      </div>
    {/if}
  </section>
{/if}

<style>
  .summary {
    display: grid;
    grid-template-columns: minmax(230px, 0.85fr) minmax(0, 1.55fr) minmax(250px, 1fr);
    gap: 18px 30px;
    padding: 18px 20px;
    border: 1px solid var(--line);
    border-radius: var(--r-lg);
    background: var(--surface);
    font-size: 13.5px;
  }
  h3 {
    margin: 0 0 10px;
    font-size: 13.5px;
    font-weight: 600;
    color: var(--text);
  }
  .totals {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .label {
    color: var(--text-2);
    font-size: 13px;
  }
  abbr {
    text-decoration: underline dotted;
    text-underline-offset: 3px;
    cursor: help;
  }
  .big {
    font-family: var(--font-num);
    font-size: 32px;
    font-weight: 600;
    line-height: 1.1;
    color: var(--neg);
  }
  .mean {
    margin: 0 0 6px;
    color: var(--text-2);
    line-height: 1.5;
  }
  .levels-head {
    margin: 4px 0 2px;
    font-size: 13px;
    font-weight: 400;
    color: var(--text-2);
  }
  .levels {
    display: grid;
    gap: 5px;
    margin: 0;
    padding: 0;
    list-style: none;
    font-size: 13px;
  }
  .levels li {
    display: grid;
    grid-template-columns: 118px minmax(0, 1fr) 46px;
    align-items: center;
    gap: 10px;
  }
  .lvl {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--c);
    white-space: nowrap;
  }
  .mark {
    flex: none;
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .lvl-bar {
    height: 6px;
    border-radius: 3px;
    background: var(--surface-2);
    overflow: hidden;
  }
  .lvl-bar i {
    transform-origin: left;
    animation: grow-x var(--dur-slow) var(--ease-out) both;
    display: block;
    height: 100%;
    border-radius: 3px;
    background: var(--c);
  }
  .levels b {
    font-weight: 500;
    text-align: right;
  }
  .flawed {
    margin: 2px 0 0;
    font-size: 13px;
    color: var(--text-2);
  }
  .loss {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    color: var(--c);
  }
  .loss small {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 11.5px;
    white-space: nowrap;
  }
  .sources b {
    font-weight: 500;
    color: var(--text);
  }
  .sources {
    display: flex;
    flex-wrap: wrap;
    gap: 2px 12px;
    margin: 6px 0 0;
    font-size: 12.5px;
    color: var(--text-2);
  }
  ol {
    display: grid;
    gap: 14px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  ol > li {
    display: grid;
    grid-template-columns: 22px minmax(0, 1fr);
    gap: 10px;
  }
  .rank {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    background: var(--surface-3);
    color: var(--text);
    font-size: 12.5px;
    font-weight: 600;
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: 5px;
    min-width: 0;
  }
  .row {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
  }
  .spot {
    font-size: 14.5px;
    font-weight: 600;
  }
  .cost {
    flex-shrink: 0;
    color: var(--neg);
    font-size: 13.5px;
    font-weight: 600;
  }
  .meter {
    display: block;
    height: 4px;
    border-radius: 2px;
    background: var(--surface-2);
  }
  .meter i {
    transform-origin: left;
    animation: grow-x var(--dur-slow) var(--ease-out) both;
    display: block;
    height: 100%;
    border-radius: 2px;
    background: color-mix(in srgb, var(--neg) 75%, transparent);
  }
  .hint {
    color: var(--text-2);
    font-size: 13px;
    line-height: 1.5;
  }
  .links {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 1px;
  }
  .links .btn:not(.ghost) {
    border-color: color-mix(in srgb, var(--text) 30%, transparent);
    background: var(--surface-3);
    font-weight: 600;
  }
  .links .btn {
    height: 28px;
    padding: 0 10px;
    font-size: 13px;
  }
  .costly {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .hand {
    transition: background var(--dur-fast) var(--ease-out);
    display: grid;
    grid-template-columns: 46px minmax(0, 1fr) auto;
    align-items: center;
    gap: 10px;
    padding: 7px 8px;
    margin: 0 -8px;
    border: 0;
    border-radius: var(--r-sm);
    background: transparent;
    text-align: left;
    color: var(--text);
    cursor: pointer;
  }
  .hand:hover {
    background: var(--surface-2);
  }
  .hand:hover .what small {
    color: var(--text-2);
  }
  .hole {
    display: flex;
    gap: 2px;
  }
  .what {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .what > span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .what small {
    color: var(--text-2);
    font-size: 12.5px;
  }
  .band {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 10px;
    font-size: 13.5px;
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
    font-size: 13px;
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
  @container (max-width: 1180px) {
    .summary {
      grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    }
    .focus {
      grid-column: 1 / -1;
      grid-row: 2;
    }
  }
</style>
