<script lang="ts">
  import { api } from "../api";
  import type { LearnTarget } from "../app/Leaks.svelte";
  import PlayingCard from "../ui/PlayingCard.svelte";
  import type { Filter } from "../types";
  import { int } from "../app/format";
  import Button from "../ui/Button.svelte";
  import Card from "../ui/Card.svelte";
  import GradeBars from "../ui/GradeBars.svelte";
  import GradeIcon from "../ui/GradeIcon.svelte";
  import type { Grade } from "../ui/poker";
  import Term from "../ui/Term.svelte";
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
  /** The analysis says "inaccuracy" where the kit's grade scale says "inaccurate". */
  const grade = (r: Rating): Grade => (r === "inaccuracy" ? "inaccurate" : r);
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
  {@const top = focus[0]?.loss_bb ?? 1}
  {@const ratings = data.ratings}
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
      <GradeBars
        label="Bewertungen deiner Entscheidungen"
        grades={RATINGS.map(grade)}
        counts={Object.fromEntries(RATINGS.map((r, i) => [grade(r), ratings[i]]))}
      />
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
                      <Button size="sm" variant={j === 0 ? "secondary" : "quiet"} onclick={() => onLearn(l.target)}>{l.label}</Button>
                    {/each}
                  {/if}
                  {#if onSpot}
                    <Button size="sm" variant="quiet" onclick={() => onSpot(spot)}>Hände ansehen</Button>
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
          <Card pad="sm" onclick={() => onOpen(c.hand_id, c.action_index)}>
            <span class="hand">
            <span class="hole">{#each cards(c.hero_cards) as card (card)}<PlayingCard {card} size={20} />{/each}</span>
            <span class="what">
              <span>{c.played} statt {c.best}</span>
              <small>{c.position ? `${c.position} · ` : ""}Hand ansehen</small>
            </span>
            <span class="loss" style="--c:{c.rating ? `var(--grade-${grade(c.rating)})` : 'var(--text)'}">
              <b class="num">−{bb(c.ev_loss_bb ?? 0, 2)}</b>
              {#if c.rating}<small><GradeIcon grade={grade(c.rating)} size={13} />{RATING[c.rating].label}</small>{/if}
            </span>
            </span>
          </Card>
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
    font-size: var(--fs-base);
  }
  h3 {
    margin: 0 0 10px;
    font-size: var(--fs-base);
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
    font-size: var(--fs-md);
  }
  abbr {
    text-decoration: underline dotted;
    text-underline-offset: 3px;
    cursor: help;
  }
  .big {
    font-family: var(--font-num);
    font-size: var(--fs-4xl);
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
    margin: 4px 0 4px;
    font-size: var(--fs-md);
    font-weight: 400;
    color: var(--text-2);
  }
  .flawed {
    margin: 2px 0 0;
    font-size: var(--fs-md);
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
    font-size: var(--fs-md);
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
    font-size: var(--fs-md);
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
    font-size: var(--fs-md);
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
    font-size: var(--fs-base);
    font-weight: 600;
  }
  .cost {
    flex-shrink: 0;
    color: var(--neg);
    font-size: var(--fs-base);
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
    animation: grow-x var(--dur-slow) var(--ease-enter) both;
    display: block;
    height: 100%;
    border-radius: 2px;
    background: color-mix(in srgb, var(--neg) 75%, transparent);
  }
  .hint {
    color: var(--text-2);
    font-size: var(--fs-md);
    line-height: 1.5;
  }
  .links {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 1px;
  }
  .costly {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }
  .hand {
    display: grid;
    grid-template-columns: 46px minmax(0, 1fr) auto;
    align-items: center;
    gap: 10px;
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
    line-height: 1.3;
  }
  .what small {
    color: var(--text-2);
    font-size: var(--fs-md);
  }
  .band {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 10px;
    font-size: var(--fs-base);
    color: var(--text-2);
  }
  .lead b {
    margin-left: 4px;
    font-family: var(--font-num);
    font-size: var(--fs-lg);
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
    font-size: var(--fs-md);
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
