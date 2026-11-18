<script lang="ts">
  import type { LearnTarget } from "../app/Leaks.svelte";
  import Replayer from "../app/Replayer.svelte";
  import PlayingCard from "../PlayingCard.svelte";
  import type { Hand } from "../types";
  import { coachApi, errorText } from "./api";
  import CardText from "./CardText.svelte";
  import { conceptName } from "./concepts";
  import { CHOICE_NAME, accepted, bb, behind, choices, decisionIndex, played, scene, type Choice } from "./drill";
  import Figures from "./Figures.svelte";
  import { positionHint, verdictOf } from "./meaning";
  import { appear, rise } from "./motion";
  import type { Practice } from "./ReportView.svelte";
  import type { CoachSpot } from "./types";

  let {
    spot,
    n,
    total,
    title,
    practice,
    onBack,
    onTraining,
    onRead,
    onAnswer,
  }: {
    spot: CoachSpot;
    /** The spot's number in the report. */
    n: number;
    total: number;
    title: string;
    practice: Practice;
    onBack: () => void;
    onTraining: (target: LearnTarget | null) => void;
    onRead: () => void;
    /** The answer and whether it matches the coach (null when the coach names no clear play). */
    onAnswer?: (answer: Choice, right: boolean | null) => void;
  } = $props();

  let hand = $state<Hand | null>(null);
  let failed = $state<string | null>(null);
  let answer = $state<Choice | null>(null);

  $effect(() => {
    const s = spot;
    hand = null;
    answer = null;
    failed = null;
    coachApi.spotHand(s).then(
      (h) => {
        if (s !== spot) return;
        hand = h;
        if (!h) failed = "Diese Hand ist nicht mehr gespeichert.";
      },
      (e) => (failed = errorText(e)),
    );
  });

  const at = $derived(hand ? decisionIndex(hand, spot) : null);
  const options = $derived(hand && at !== null ? choices(hand, at) : []);
  const view = $derived(hand && at !== null ? scene(hand, at) : null);
  const actual = $derived(hand && at !== null ? played(hand, at) : null);
  const ok = $derived(actual ? accepted(spot, actual) : null);
  const verdict = $derived(verdictOf(spot.verdict));
  const right = $derived(answer && ok ? ok.includes(answer) : null);
  /** The stack at the decision, as the table shows it; the spot's own figure is the stack at the start of the hand. */
  const stack = $derived(hand && at !== null ? bb(behind(hand, at)) : bb(spot.stack_bb));
</script>

<section class="panel drill" aria-labelledby="drill-title">
  <header class="top">
    <button class="btn ghost back" onclick={onBack}>
      <svg viewBox="0 0 24 24" width="14" height="14" aria-hidden="true"><path d="M19 12H5M11 6l-6 6 6 6" /></svg>
      Zurück zum Bericht
    </button>
    <span class="where">{title} · Entscheidung <span class="num">{n}</span> von <span class="num">{total}</span></span>
  </header>

  <div class="stage">
    <div class="table">
      {#if failed}
        <p class="msg" role="alert">{failed}</p>
      {:else if hand && at !== null}
        <Replayer {hand} upTo={answer ? null : at} quiz={!answer} inBigBlinds>
          <div class="hand-title">
            {#if answer}
              <span class="stop done">Aufgelöst: so ging die Hand weiter</span>
            {:else}
              <span class="stop">Stopp vor deiner Entscheidung</span>
            {/if}
          </div>
        </Replayer>
      {:else if hand}
        <p class="msg">In dieser Hand findet Ringside deine Entscheidung nicht.</p>
      {:else}
        <p class="faint loading">Hand wird geladen …</p>
      {/if}
    </div>


    <aside class="side">
      <div class="ask">
        <p class="eyebrow">Diesen Spot üben · {conceptName(spot.concept)}</p>
        <h2 id="drill-title">
          Du sitzt im <abbr title={positionHint(spot.position)}>{spot.position ?? "–"}</abbr> mit <CardText
            text={spot.hero_cards.join("")}
          />, noch <span class="num">{stack}</span> im Stack. Was spielst du?
        </h2>
        {#if view}
          <p class="scene">
            <span class="street">{view.street}</span>
            {#if view.board.length}
              <span class="board">{#each view.board as c (c)}<PlayingCard card={c} size={20} />{/each}</span>
            {/if}
            <span>Pot <b class="num">{view.pot}</b></span>
            <span class="facing">{view.facing}</span>
          </p>
        {/if}
      </div>

      <div class="act">
      {#if hand && at !== null}
        {#if !answer}
          <div class="choices" role="group" aria-label="Deine Entscheidung" in:appear>
            {#each options as o (o.id)}
              <button
                class="choice c-{o.id}"
                onclick={() => {
                  answer = o.id;
                  onAnswer?.(o.id, ok ? ok.includes(o.id) : null);
                }}
              >
                <b>{o.label}</b>
                {#if o.hint}<span class="num">{o.hint}</span>{/if}
              </button>
            {/each}
          </div>
        {:else}
          <div class="result v-{spot.verdict}" in:rise>
            <div class="verdict-line">
              {#if right === true}
                <span class="badge good">Passt</span>
                <b>Du liegst wie der Coach.</b>
              {:else if right === false}
                <span class="badge bad">Anders als der Coach</span>
                <b>Der Coach empfiehlt {ok?.map((c) => CHOICE_NAME[c]).join(" oder ")}.</b>
              {:else}
                <span class="badge">Zum Vergleichen</span>
                <b>Der Coach nennt hier keine eindeutige Aktion.</b>
              {/if}
            </div>
            <dl class="compare">
              <div><dt>Du jetzt</dt><dd>{CHOICE_NAME[answer]}</dd></div>
              <div>
                <dt>Damals gespielt</dt>
                <dd>{actual ? CHOICE_NAME[actual] : "–"} <span class="vtag">{verdict.name}</span></dd>
              </div>
              {#if spot.better}
                <div><dt>Besser laut Coach</dt><dd><CardText text={spot.better} /></dd></div>
              {/if}
            </dl>
            <div class="next">
              {#if practice.train}
                <button class="btn primary" onclick={() => onTraining(practice.train)}>
                  Ähnliche Spots üben
                  <svg viewBox="0 0 24 24" width="14" height="14" aria-hidden="true"><path d="M5 12h14M13 6l6 6-6 6" /></svg>
                </button>
              {:else}
                <button class="btn primary" onclick={onBack}>Zurück zum Bericht</button>
              {/if}
              {#if practice.articles.length}
                <button class="btn" onclick={onRead}>Nachlesen: {practice.articles[0].title}</button>
              {/if}
              <button class="btn ghost" onclick={() => (answer = null)}>Nochmal entscheiden</button>
            </div>
            {#if !practice.train}
              <p class="fine">Für dieses Thema gibt es noch keine eigene Übung; hier übst du genau diesen einen Spot.</p>
            {/if}
          </div>
        {/if}
      {/if}
      </div>
    </aside>
  </div>

  {#if answer}
    <div class="detail" in:rise>
      <div class="detail-why">
        <h3>Warum</h3>
        <p class="why"><CardText text={spot.why} /></p>
      </div>
      {#if spot.numbers.length}
        <div class="detail-figs">
          <h3>Ringsides Zahlen</h3>
          <Figures numbers={spot.numbers} />
        </div>
      {/if}
    </div>
  {/if}
</section>

<style>
  .drill {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 14px 22px 22px;
  }
  .top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 8px 16px;
    margin-left: -10px;
  }
  .back svg,
  .next svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .where {
    font-size: 12.5px;
    color: var(--text-2);
  }
  .eyebrow {
    margin: 0 0 4px;
    font-size: 12.5px;
    color: var(--text-2);
  }
  h2 {
    margin: 0;
    font-family: var(--font-num);
    font-size: 21px;
    font-weight: 600;
    line-height: 1.35;
  }
  .scene {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 14px;
    margin: 12px 0 0;
    padding: 9px 12px;
    border-radius: var(--r-sm);
    background: var(--surface-2);
    font-size: 13px;
    color: var(--text-2);
  }
  .scene .street {
    font-weight: 600;
    color: var(--text);
  }
  .scene .board {
    display: flex;
    gap: 2px;
  }
  .scene b {
    color: var(--text);
    font-weight: 600;
  }
  .scene .facing {
    color: var(--text);
  }
  abbr {
    text-decoration: underline dotted var(--text-3);
    text-underline-offset: 4px;
    cursor: help;
  }
  .stage {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(300px, min(440px, 34%));
    gap: 16px;
    align-items: start;
  }
  .side {
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-width: 0;
  }
  .table {
    display: flex;
    height: clamp(560px, 70vh, 780px);
    min-width: 0;
    overflow: hidden;
    padding: 10px;
    border-radius: var(--r-sm);
    background: var(--bg);
    border: 1px solid var(--line);
  }
  .hand-title {
    display: flex;
    gap: 10px;
    white-space: nowrap;
  }
  .stop {
    padding: 2px 9px;
    border-radius: 999px;
    border: 1px solid var(--line-2);
    font-size: 12.5px;
    color: var(--text-2);
  }
  .stop.done {
    color: var(--text);
    border-color: var(--text-2);
  }
  .msg,
  .loading {
    flex: 1;
    align-self: center;
    margin: 0;
    padding: 24px;
    text-align: center;
    color: var(--text-2);
  }

  .choices {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 8px;
  }
  .choice {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    padding: 14px 12px;
    border-radius: var(--r-sm);
    border: 1px solid var(--line-2);
    background: var(--surface-2);
    cursor: pointer;
    transition:
      background var(--dur-fast, 120ms) var(--ease-standard, cubic-bezier(0.2, 0, 0, 1)),
      transform var(--dur-fast, 120ms) var(--ease-standard, cubic-bezier(0.2, 0, 0, 1)),
      border-color var(--dur-fast, 120ms) var(--ease-standard, cubic-bezier(0.2, 0, 0, 1));
  }
  .choice:hover {
    background: var(--surface-3);
    border-color: var(--text-2);
    transform: translateY(-1px);
  }
  @media (prefers-reduced-motion: reduce) {
    .choice {
      transition: none;
    }
    .choice:hover {
      transform: none;
    }
  }
  .choice:active {
    transform: translateY(0);
  }
  .choice b {
    font-size: 16px;
    font-weight: 600;
  }
  .choice span {
    font-size: 12.5px;
    color: var(--text-2);
  }

  .result {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 16px 18px;
    border-radius: var(--r-sm);
    background: var(--bg);
    border: 1px solid var(--line);
  }
  .verdict-line {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px 12px;
    font-size: 16px;
  }
  .badge {
    padding: 2px 10px;
    border-radius: 999px;
    border: 1px solid var(--line-2);
    font-size: 12.5px;
    font-weight: 600;
    color: var(--text-2);
  }
  .badge.good {
    color: var(--pos);
    border-color: color-mix(in srgb, var(--pos) 45%, transparent);
  }
  .badge.bad {
    color: var(--amber);
    border-color: color-mix(in srgb, var(--amber) 45%, transparent);
  }
  .compare {
    display: flex;
    flex-wrap: wrap;
    gap: 10px 28px;
    margin: 0;
  }
  dt {
    font-size: 12.5px;
    color: var(--text-2);
  }
  dd {
    margin: 2px 0 0;
    font-weight: 600;
  }
  .vtag {
    margin-left: 6px;
    font-size: 12.5px;
    font-weight: 600;
  }
  .v-fehler .vtag {
    color: var(--neg);
  }
  .v-fraglich .vtag {
    color: var(--amber);
  }
  .v-stark .vtag {
    color: var(--pos);
  }
  .why {
    margin: 0;
    line-height: 1.55;
    color: var(--text-2);
    max-width: 80ch;
  }
  .detail {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1.2fr);
    gap: 16px 28px;
    padding: 16px 18px;
    border-radius: var(--r-sm);
    background: var(--bg);
    border: 1px solid var(--line);
  }
  .detail h3 {
    margin: 0 0 8px;
    font-size: 13px;
    font-weight: 600;
    color: var(--text-2);
  }
  .detail .why {
    color: var(--text);
    font-size: 14.5px;
  }
  .next {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .fine {
    margin: 0;
    font-size: 12.5px;
    color: var(--text-2);
  }

  .act {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  @container (max-width: 980px) {
    .detail {
      grid-template-columns: minmax(0, 1fr);
    }
    .stage,
    .side {
      display: contents;
    }
    .top {
      order: -4;
    }
    .ask {
      order: -3;
    }
    .act {
      order: -2;
    }
    .detail {
      order: -1;
    }
    .table {
      height: 600px;
    }
    .choices {
      grid-template-columns: repeat(auto-fit, minmax(130px, 1fr));
    }
  }
</style>
