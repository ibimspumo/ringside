<script lang="ts">
  import type { LearnTarget } from "../app/Leaks.svelte";
  import Replayer from "../app/Replayer.svelte";
  import PlayingCard from "../ui/PlayingCard.svelte";
  import type { Hand } from "../types";
  import { coachApi, errorText } from "./api";
  import CardText from "../ui/CardText.svelte";
  import { conceptName } from "./concepts";
  import { CHOICE_NAME, accepted, bb, behind, choices, decisionIndex, played, scene, type Choice } from "./drill";
  import Figures from "./Figures.svelte";
  import { positionHint, verdictOf, say } from "./meaning";
  import { appear, rise } from "./motion";
  import Badge from "../ui/Badge.svelte";
  import Button from "../ui/Button.svelte";
  import Card from "../ui/Card.svelte";
  import GradeBadge from "../ui/GradeBadge.svelte";
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

  /** The reason's first sentence, next to the verdict; the whole reason follows below the table. */
  const gist = $derived(spot.why.match(/^.*?[.!?](?=\s|$)/)?.[0] ?? spot.why);

  const KEY: Record<Choice, string> = { fold: "F", check: "X", call: "C", raise: "R", allin: "A" };

  function choose(c: Choice) {
    answer = c;
    onAnswer?.(c, ok ? ok.includes(c) : null);
  }

  /** F, X, C, R, A answer like the buttons; N decides again; Esc goes back to the report. */
  function onKey(e: KeyboardEvent) {
    if (e.altKey || e.ctrlKey || e.metaKey) return;
    if ((e.target as HTMLElement | null)?.closest("input, textarea, select, [contenteditable]")) return;
    const k = e.key.toUpperCase();
    if (k === "ESCAPE") return onBack();
    if (answer) {
      if (k === "N") answer = null;
      return;
    }
    const pick = options.find((o) => KEY[o.id] === k);
    if (pick) {
      e.preventDefault();
      choose(pick.id);
    }
  }
</script>

<svelte:window onkeydown={onKey} />

{#snippet backIcon()}<svg viewBox="0 0 24 24" width="14" height="14" aria-hidden="true"><path d="M19 12H5M11 6l-6 6 6 6" /></svg>{/snippet}
{#snippet arrow()}<svg viewBox="0 0 24 24" width="14" height="14" aria-hidden="true"><path d="M5 12h14M13 6l6 6-6 6" /></svg>{/snippet}

<Card pad="lg" aria-labelledby="drill-title">
<div class="drill">
  <header class="top">
    <Button variant="quiet" icon={backIcon} onclick={onBack} kbd="Esc">Zurück zum Bericht</Button>
    <span class="where">{title} · Entscheidung <span class="num">{n}</span> von <span class="num">{total}</span></span>
  </header>

  <div class="stage">
    <div class="felt-box">
      {#if failed}
        <p class="msg" role="alert">{failed}</p>
      {:else if hand && at !== null}
        <Replayer {hand} upTo={answer ? null : at} quiz={!answer} inBigBlinds quietPlay>
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
        <p class="msg">Hand wird geladen …</p>
      {/if}
    </div>


    <aside class="side">
      <div class="ask">
        <p class="eyebrow">Diesen Spot üben · {conceptName(spot.concept)}</p>
        <h2 id="drill-title">
          Du sitzt im <abbr title={positionHint(spot.position)}>{spot.position ?? "–"}</abbr> mit <CardText hands
            text={spot.hero_cards.join("")}
          />, noch <span class="num">{stack}</span> im Stack. Was spielst du?
        </h2>
        {#if view}
          <p class="scene">
            <span class="street">{view.street}</span>
            {#if view.board.length}
              <span class="board">{#each view.board as c (c)}<PlayingCard card={c} size={20} />{/each}</span>
            {/if}
            <span>{view.betIn ? "Pot mit seiner Bet" : "Pot"} <b class="num">{view.pot}</b></span>
            <span class="facing">{view.facing}</span>
          </p>
        {/if}
      </div>

      <div class="act">
      {#if hand && at !== null}
        {#if !answer}
          <div class="choices" role="group" aria-label="Deine Entscheidung" in:appear>
            {#each options as o (o.id)}
              <Button size="lg" full kbd={KEY[o.id]} onclick={() => choose(o.id)}>
                {o.label}{#if o.hint}<span class="hint num">{o.hint}</span>{/if}
              </Button>
            {/each}
          </div>
        {:else}
          <div class="result v-{spot.verdict}" in:rise>
            <div class="verdict-line">
              {#if right === true}
                <Badge tone="good">Passt</Badge>
                <b>Du liegst wie der Coach.</b>
              {:else if right === false}
                <Badge tone="warn">Anders als der Coach</Badge>
                <b>Der Coach empfiehlt {ok?.map((c) => CHOICE_NAME[c]).join(" oder ")}.</b>
              {:else}
                <Badge>Zum Vergleichen</Badge>
                <b>Der Coach nennt hier keine eindeutige Aktion.</b>
              {/if}
            </div>
            <p class="gist"><CardText hands text={say(gist)} /></p>
            <dl class="compare">
              <div class="mine"><dt>Jetzt</dt><dd>{CHOICE_NAME[answer]}</dd></div>
              <div>
                <dt>Im Turnier</dt>
                <dd class="played">{actual ? CHOICE_NAME[actual] : "–"} <span title="Urteil des Coaches: {verdict.name}"><GradeBadge grade={verdict.grade} size="sm" variant="icon" /></span></dd>
              </div>
              {#if spot.better}
                <div><dt>Coach</dt><dd><CardText hands text={say(spot.better)} /></dd></div>
              {/if}
            </dl>
            <div class="next">
              {#if practice.train}
                <Button variant="primary" icon={arrow} onclick={() => onTraining(practice.train)}>Ähnliche Spots üben</Button>
              {:else}
                <Button variant="primary" onclick={onBack}>Zurück zum Bericht</Button>
              {/if}
              {#if practice.articles.length}
                <Button onclick={onRead}>Nachlesen: {practice.articles[0].title}</Button>
              {/if}
              <Button variant="quiet" kbd="N" onclick={() => (answer = null)}>Nochmal entscheiden</Button>
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
        <p class="why"><CardText hands text={say(spot.why)} /></p>
      </div>
      {#if spot.numbers.length}
        <div class="detail-figs">
          <h3>Ringsides Zahlen</h3>
          <Figures numbers={spot.numbers} />
        </div>
      {/if}
    </div>
  {/if}
</div>
</Card>

<style>
  .drill {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
  }
  .top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 8px 16px;
    margin: calc(-1 * var(--sp-2)) 0 0 calc(-1 * var(--sp-3));
  }
  svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .where {
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .eyebrow {
    margin: 0 0 4px;
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  h2 {
    margin: 0;
    font-family: var(--font-num);
    font-size: var(--fs-2xl);
    font-weight: var(--fw-semibold);
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
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .scene .street {
    font-weight: var(--fw-semibold);
    color: var(--text);
  }
  .scene .board {
    display: flex;
    gap: 2px;
  }
  .scene b {
    color: var(--text);
    font-weight: var(--fw-semibold);
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
  .felt-box {
    display: flex;
    height: clamp(640px, 76vh, 780px);
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
    border-radius: var(--r-pill);
    border: 1px solid var(--line-2);
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .stop.done {
    color: var(--text);
    border-color: var(--text-2);
  }
  .msg {
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
    font-size: var(--fs-lg);
  }
  .gist {
    margin: 0;
    font-size: var(--fs-base);
    line-height: 1.5;
    color: var(--text);
  }
  .compare {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: var(--sp-2);
    margin: 0;
  }
  .compare > div {
    padding: var(--sp-2) var(--sp-3);
    border-radius: var(--r-sm);
    background: var(--surface-2);
    border: 1px solid var(--line);
  }
  .compare dt {
    white-space: nowrap;
  }
  .compare > .mine {
    border-color: var(--text-2);
  }
  dt {
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  dd {
    margin: 2px 0 0;
    font-weight: var(--fw-semibold);
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
    font-size: var(--fs-md);
    font-weight: var(--fw-semibold);
    color: var(--text-2);
  }
  .detail .why {
    color: var(--text);
    font-size: var(--fs-base);
  }
  .next {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .hint {
    margin-left: var(--sp-2);
    font-weight: var(--fw-regular);
    color: var(--text-2);
  }
  .played {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2);
  }
  .fine {
    margin: 0;
    font-size: var(--fs-md);
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
    .felt-box {
      height: 600px;
    }
    .choices {
      grid-template-columns: repeat(auto-fit, minmax(130px, 1fr));
    }
  }
</style>
