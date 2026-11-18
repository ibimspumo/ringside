<script lang="ts">
  import { requestAnalysis } from "../analysis/store.svelte";
  import Felt from "../Felt.svelte";
  import Badge from "../ui/Badge.svelte";
  import Button from "../ui/Button.svelte";
  import CardText from "../ui/CardText.svelte";
  import GradeBadge from "../ui/GradeBadge.svelte";
  import PlayingCard from "../ui/PlayingCard.svelte";
  import { conceptName } from "./concepts";
  import { CHOICE_NAME, type Choice } from "./drill";
  import Figures from "./Figures.svelte";
  import { positionHint, verdictOf } from "./meaning";
  import type { Practice } from "./ReportView.svelte";
  import type { CoachSpot } from "./types";

  let {
    spot,
    index,
    practice,
    onHand,
    onDrill,
    onRead,
    practised,
  }: {
    spot: CoachSpot;
    index: number;
    practice: Practice;
    onHand: (id: number) => void;
    onDrill: () => void;
    onRead: () => void;
    practised?: { answer: Choice; right: boolean | null };
  } = $props();

  const verdict = $derived(verdictOf(spot.verdict));
  const stack = $derived(spot.stack_bb.toLocaleString("de-DE", { maximumFractionDigits: 1 }));
  const time = $derived(spot.played_at.slice(11, 16));
  const street = $derived(
    spot.board.length === 0 ? "vor dem Flop" : spot.board.length === 3 ? "am Flop" : spot.board.length === 4 ? "am Turn" : "am River",
  );
</script>

{#snippet arrow()}<svg viewBox="0 0 24 24" width="14" height="14" aria-hidden="true"><path d="M5 12h14M13 6l6 6-6 6" /></svg>{/snippet}
{#snippet play()}<svg class="fill" viewBox="0 0 24 24" width="13" height="13" aria-hidden="true"><path d="M8 5.5v13l10-6.5z" /></svg>{/snippet}

<article class="spot" style:--accent="var(--grade-{verdict.grade})" id="spot-{index}" tabindex="-1" aria-labelledby="spot-{index}-title">
  <div class="visual" aria-label="Deine Karten {street}">
    <Felt>
      <span class="seat-tag at-left" title={positionHint(spot.position)}>{spot.position ?? "–"}</span>
      <span class="seat-tag at-right num" title="Dein Stack in Big Blinds zu Beginn der Hand">{stack} BB</span>
      {#if spot.board.length}
        <div class="board">
          {#each spot.board as c (c)}<PlayingCard card={c} size={26} />{/each}
        </div>
      {:else}
        <span class="street">vor dem Flop</span>
      {/if}
      <div class="hero">
        {#each spot.hero_cards as c (c)}<PlayingCard card={c} size={36} />{/each}
      </div>
    </Felt>
  </div>

  <div class="text">
    <header>
      <span class="idx num">{index}</span>
      <GradeBadge grade={verdict.grade} size="sm" />
      <span class="meaning">{verdict.meaning}</span>
      {#if practised}
        <Badge size="sm" tone={practised.right === true ? "good" : practised.right === false ? "warn" : "neutral"} title="In dieser Sitzung geübt"
          >Geübt: {CHOICE_NAME[practised.answer]}{practised.right === true
            ? " · wie der Coach"
            : practised.right === false
              ? " · anders als der Coach"
              : ""}</Badge
        >
      {/if}
      <span class="time num" title="Gespielt um {time} Uhr">{time}</span>
    </header>
    <p class="action" id="spot-{index}-title"><CardText hands text={spot.action} /></p>
    {#if spot.better}
      <p class="better"><span>Besser</span><b><CardText hands text={spot.better} /></b></p>
    {/if}
    <p class="why"><CardText hands text={spot.why} /></p>
    {#if spot.numbers.length}
      <Figures numbers={spot.numbers} />
    {/if}
    <footer>
      <div class="learn">
        <span class="topic"><span class="label">Thema</span> {conceptName(spot.concept)}</span>
        <Button size="sm" onclick={onDrill} icon={arrow} title="Die Hand hält vor deiner Entscheidung an; du entscheidest selbst">Diesen Spot üben</Button>
        {#if practice.articles.length}
          <Button size="sm" variant="quiet" onclick={onRead} title={practice.articles[0].title}>Nachlesen</Button>
        {/if}
      </div>
      <div class="hand">
        <Button size="sm" variant="quiet" icon={play} onclick={() => onHand(spot.hand_id)}>Im Replayer ansehen</Button>
        <Button size="sm" variant="quiet" onclick={() => requestAnalysis(spot.hand_id)}>Diese Hand analysieren</Button>
      </div>
    </footer>
  </div>
</article>

<style>
  .spot {
    display: grid;
    grid-template-columns: 252px minmax(0, 1fr);
    gap: var(--sp-5);
    padding: var(--sp-4) var(--sp-5) var(--sp-4) var(--sp-4);
    border: 1px solid var(--line);
    border-radius: var(--r-lg);
    background: var(--surface);
    box-shadow: inset 3px 0 0 var(--accent);
    scroll-margin-top: 16px;
  }
  .spot:focus {
    outline: none;
  }
  .spot:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: 2px;
  }
  .visual {
    position: relative;
    align-self: start;
    aspect-ratio: 1.42;
    border-radius: var(--r-sm);
    overflow: hidden;
  }
  .board {
    position: absolute;
    left: 50%;
    top: 40%;
    transform: translate(-50%, -50%);
    display: flex;
    gap: 3px;
  }
  .street {
    position: absolute;
    left: 50%;
    top: 40%;
    transform: translate(-50%, -50%);
    font-size: var(--fs-sm);
    letter-spacing: 0.04em;
    color: color-mix(in srgb, var(--text) 40%, transparent);
    white-space: nowrap;
  }
  .hero {
    position: absolute;
    left: 50%;
    bottom: 5%;
    transform: translateX(-50%);
    display: flex;
    gap: 3px;
  }
  .seat-tag {
    position: absolute;
    top: 7px;
    padding: 1px 7px;
    border-radius: var(--r-pill);
    background: color-mix(in srgb, var(--bg) 82%, transparent);
    border: 1px solid var(--line-2);
    font-size: var(--fs-sm);
    color: var(--text);
    cursor: help;
    z-index: 1;
  }
  .at-left {
    left: 7px;
    font-weight: var(--fw-semibold);
  }
  .at-right {
    right: 7px;
  }

  .text {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
  }
  header {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  .idx {
    display: grid;
    place-items: center;
    flex-shrink: 0;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    background: var(--surface-3);
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .meaning {
    font-size: var(--fs-md);
    color: var(--text-2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .time {
    margin-left: auto;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  p {
    margin: 0;
  }
  .action {
    font-size: var(--fs-lg);
    font-weight: var(--fw-semibold);
    line-height: 1.4;
  }
  .better {
    display: flex;
    align-items: baseline;
    gap: 10px;
    align-self: flex-start;
    padding: 6px 12px 6px 10px;
    border-radius: var(--r-sm);
    background: var(--surface-2);
    border-left: 2px solid var(--text);
  }
  .better span {
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .better b {
    font-weight: var(--fw-semibold);
  }
  .why {
    color: var(--text-2);
    line-height: 1.55;
    max-width: 78ch;
  }
  footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 8px 16px;
    margin-top: 4px;
    padding-top: 10px;
    border-top: 1px solid var(--line);
  }
  .learn,
  .hand {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px 10px;
  }
  .hand {
    gap: 4px;
    margin-left: auto;
  }
  .topic {
    font-size: var(--fs-md);
  }
  .label {
    color: var(--text-2);
  }
  svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  svg.fill {
    fill: currentColor;
    stroke: none;
  }

  @container (max-width: 820px) {
    .spot {
      grid-template-columns: minmax(0, 1fr);
      gap: 14px;
    }
    .visual {
      width: 100%;
      max-width: 340px;
    }
  }
</style>
