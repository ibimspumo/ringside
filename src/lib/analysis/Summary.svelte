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
  import SourceNote from "../ui/SourceNote.svelte";
  import { EV_LOSS_EXPLAIN, GRADE_OF, KIT_SOURCE, RATING, RATING_RULE, RATINGS, SOURCE, bb, reliabilityText } from "./labels";
  import NextSteps from "../next/NextSteps.svelte";
  import type { NextStep, StepHands } from "../next/types";
  import { analysis } from "./store.svelte";
  import AtTable from "../app/AtTable.svelte";
  import { adviceLocked, watchPokerClient } from "../app/client.svelte";
  import type { AnalysisSummary, Rating, SpotType } from "./types";

  /**
   * What the analysis found: the loss and how the decisions were rated, the first three steps of
   * „Was als Nächstes?“ (the same list as everywhere), then the costliest single decisions.
   */
  let {
    filter,
    refresh,
    onOpen,
    onSpots,
    onLearn,
    onStepHands,
    strip = false,
  }: {
    filter: Filter;
    refresh: number;
    onOpen: (handId: number, actionIndex: number) => void;
    /** Narrows the hand list to these spot types. */
    onSpots?: (spots: SpotType[]) => void;
    onLearn?: (target: LearnTarget) => void;
    onStepHands?: (hands: StepHands) => void;
    /** One low band above a list that already shows the costliest hands. */
    strip?: boolean;
  } = $props();

  let data = $state<AnalysisSummary | null>(null);
  let steps = $state<NextStep[]>([]);
  watchPokerClient();
  const locked = $derived(adviceLocked());

  $effect(() => {
    if (locked) {
      data = null;
      steps = [];
      return;
    }
    void refresh;
    void analysis.solved;
    void analysis.status?.counts.analyzed;
    const f = { ...filter };
    let live = true;
    api.analysisSummary(f).then(
      (s) => live && (data = s),
      () => {},
    );
    api.nextSteps(f).then(
      (s) => live && (steps = s),
      () => live && (steps = []),
    );
    return () => (live = false);
  });

  /** Steps the hand list can be narrowed to. */
  const focus = $derived(steps.filter((s) => s.spots.length).slice(0, 3));
  const flawed = $derived(data ? data.ratings.slice(RATINGS.indexOf("inaccuracy")).reduce((a, b) => a + b, 0) : 0);
  const cards = (s: string) => s.split(" ").filter(Boolean);
  const grade = (r: Rating): Grade => GRADE_OF[r];
</script>


{#if locked}
  {#if !strip}<AtTable compact />{/if}
{:else if data && data.rated > 0 && strip}
  <section class="band">
    <span class="lead"><abbr title={EV_LOSS_EXPLAIN}>Deine Fehler kosten dich auf Dauer</abbr> <b class="num">{bb(data.loss_bb)}</b></span>
    {#if focus.length}
      <span class="faint">Fang hier an:</span>
      {#each focus as f (f.id)}
        {@const practice = f.practice}
        <span class="pair">
          <button class="name-chip" onclick={() => onSpots?.(f.spots)} title="{f.why} Klick zeigt nur diese Hände.">{f.title}</button>
          {#if onLearn && practice}
            <button class="learn" onclick={() => onLearn(practice)} title="{f.title} üben">Üben</button>
          {/if}
        </span>
      {/each}
    {/if}
  </section>
{:else if data && data.rated > 0}
  {@const ratings = data.ratings}
  <section class="summary">
    <div class="totals">
      <span class="label"
        ><Term id="ev_loss">Deine Fehler kosten dich auf Dauer</Term>{#if analysis.status?.progress?.state === "running"}&nbsp;– bisher{/if}</span
      >
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

    {#if steps.length && onLearn}
      <div class="focus">
        <h3>Fang hier an</h3>
        <NextSteps {steps} limit={3} compact primary={false} {onLearn} onHands={onStepHands} />
      </div>
    {/if}

    {#if data.costliest.length}
      <div class="costly">
        <h3>Teuerste Entscheidungen</h3>
        {#each data.costliest.slice(0, 3) as c (c.hand_id + ":" + c.action_index)}
          <Card pad="sm" aria-label="Hand ansehen: {c.played} statt {c.best}" title="Hand ansehen" onclick={() => onOpen(c.hand_id, c.action_index)}>
            <span class="hand">
            <span class="hole">{#each cards(c.hero_cards) as card (card)}<PlayingCard {card} size={20} />{/each}</span>
            <span class="what" style="--c:{c.rating ? `var(--grade-${grade(c.rating)})` : 'var(--text)'}">
              <span class="detail">
                <span class="decision">{c.played} statt {c.best}</span>
                <b class="num loss">−{bb(c.ev_loss_bb ?? 0, 2)}</b>
              </span>
              <span class="detail">
                <small>{c.position ?? ""}</small>
                {#if c.source && KIT_SOURCE[c.source]}
                  <SourceNote kind={KIT_SOURCE[c.source]!} hint={c.source === "estimate" && c.reliability !== null ? reliabilityText(c.reliability, c.street) : SOURCE[c.source].explain} />
                {/if}
                {#if c.rating}<small class="loss"><GradeIcon grade={grade(c.rating)} size={13} />{RATING[c.rating].label}</small>{/if}
              </span>
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
    align-items: center;
    gap: 4px;
    color: var(--c);
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
  .costly {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }
  .hand {
    display: grid;
    grid-template-columns: 46px minmax(0, 1fr);
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
  .decision {
    line-height: 1.3;
  }
  .detail {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: var(--sp-2);
  }
  .what small {
    color: var(--text-2);
    font-size: var(--fs-md);
  }
  .what small.loss {
    color: var(--c);
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
