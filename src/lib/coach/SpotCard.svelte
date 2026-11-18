<script lang="ts">
  import { requestAnalysis } from "../analysis/store.svelte";
  import Felt from "../Felt.svelte";
  import PlayingCard from "../PlayingCard.svelte";
  import { conceptName } from "./concepts";
  import CardText from "./CardText.svelte";
  import Figures from "./Figures.svelte";
  import { positionHint, verdictOf } from "./meaning";
  import type { Practice } from "./ReportView.svelte";
  import { CHOICE_NAME, type Choice } from "./drill";
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

<article class="spot v-{spot.verdict}" id="spot-{index}" tabindex="-1" aria-labelledby="spot-{index}-title">
  <div class="visual" aria-label="Deine Karten {street}">
    <Felt>
      <span class="chip pos" title={positionHint(spot.position)}>{spot.position ?? "–"}</span>
      <span class="chip stack num" title="Dein Stack in Big Blinds zu Beginn der Hand">{stack} BB</span>
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
      <span class="verdict">{verdict.name}</span>
      <span class="meaning">{verdict.meaning}</span>
      {#if practised}
        <span class="done" class:good={practised.right === true} class:off={practised.right === false}
          title="In dieser Sitzung geübt">Geübt: {CHOICE_NAME[practised.answer]}{practised.right === true
            ? " · wie der Coach"
            : practised.right === false
              ? " · anders als der Coach"
              : ""}</span
        >
      {/if}
      <span class="time num" title="Gespielt um {time} Uhr">{time}</span>
    </header>
    <p class="action" id="spot-{index}-title"><CardText text={spot.action} /></p>
    {#if spot.better}
      <p class="better"><span>Besser</span><b><CardText text={spot.better} /></b></p>
    {/if}
    <p class="why"><CardText text={spot.why} /></p>
    {#if spot.numbers.length}
      <Figures numbers={spot.numbers} />
    {/if}
    <footer>
      <div class="learn">
        <span class="topic"><span class="faint">Thema</span> {conceptName(spot.concept)}</span>
        <button class="btn small" onclick={onDrill} title="Die Hand hält vor deiner Entscheidung an; du entscheidest selbst">
          Diesen Spot üben
          <svg viewBox="0 0 24 24" width="13" height="13" aria-hidden="true"><path d="M5 12h14M13 6l6 6-6 6" /></svg>
        </button>
        {#if practice.articles.length}
          <button class="link" onclick={onRead} title={practice.articles[0].title}>Nachlesen</button>
        {/if}
      </div>
      <div class="hand">
        <button class="btn ghost small" onclick={() => onHand(spot.hand_id)}>
          <svg class="fill" viewBox="0 0 24 24" width="13" height="13" aria-hidden="true"><path d="M8 5.5v13l10-6.5z" /></svg>
          Im Replayer ansehen
        </button>
        <button class="btn ghost small" onclick={() => requestAnalysis(spot.hand_id)}>Diese Hand analysieren</button>
      </div>
    </footer>
  </div>
</article>

<style>
  .spot {
    --accent: var(--text-3);
    display: grid;
    grid-template-columns: 252px minmax(0, 1fr);
    gap: 20px;
    padding: 16px 20px 16px 16px;
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
  .v-stark {
    --accent: var(--pos);
  }
  .v-fraglich {
    --accent: var(--amber);
  }
  .v-fehler {
    --accent: var(--neg);
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
    font-size: 11.5px;
    letter-spacing: 0.04em;
    color: rgba(236, 235, 231, 0.38);
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
  .chip {
    position: absolute;
    top: 7px;
    padding: 1px 7px;
    border-radius: 999px;
    background: rgba(18, 20, 24, 0.82);
    border: 1px solid var(--line-2);
    font-size: 11.5px;
    color: var(--text);
    cursor: help;
    z-index: 1;
  }
  .chip.pos {
    left: 7px;
    font-weight: 600;
  }
  .chip.stack {
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
    font-size: 12px;
    color: var(--text-2);
  }
  .verdict {
    font-weight: 600;
    font-size: 13px;
    color: var(--accent);
  }
  .v-ok .verdict {
    color: var(--text);
  }
  .meaning {
    font-size: 13px;
    color: var(--text-2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .done {
    flex-shrink: 0;
    padding: 1px 8px;
    border-radius: 999px;
    border: 1px solid var(--line-2);
    font-size: 12px;
    color: var(--text-2);
    white-space: nowrap;
  }
  .done.good {
    color: var(--pos);
    border-color: color-mix(in srgb, var(--pos) 45%, transparent);
  }
  .done.off {
    color: var(--amber);
    border-color: color-mix(in srgb, var(--amber) 45%, transparent);
  }
  .time {
    margin-left: auto;
    font-size: 12.5px;
    color: var(--text-2);
  }
  p {
    margin: 0;
  }
  .action {
    font-size: 15px;
    font-weight: 600;
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
    font-size: 12px;
    color: var(--text-3);
  }
  .better b {
    font-weight: 600;
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
    font-size: 13px;
  }
  .small {
    height: 30px;
    padding: 0 10px;
    font-size: 13px;
    gap: 6px;
  }
  .ghost {
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
  .link {
    padding: 0;
    border: 0;
    background: none;
    color: var(--text-2);
    font-size: 12.5px;
    text-decoration: underline;
    text-decoration-color: var(--line-2);
    text-underline-offset: 3px;
    cursor: pointer;
  }
  .link:hover {
    color: var(--text);
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
