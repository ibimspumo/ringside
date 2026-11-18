<script lang="ts">
  import { requestAnalysis } from "../analysis/store.svelte";
  import type { LearnTarget } from "../app/Leaks.svelte";
  import Felt from "../Felt.svelte";
  import PlayingCard from "../PlayingCard.svelte";
  import { conceptName } from "./concepts";
  import { positionHint, termsIn, verdictOf } from "./meaning";
  import type { Practice } from "./ReportView.svelte";
  import type { CoachSpot } from "./types";

  let {
    spot,
    index,
    practice,
    onHand,
    onTraining,
  }: {
    spot: CoachSpot;
    index: number;
    practice: Practice;
    onHand: (id: number) => void;
    onTraining: (target: LearnTarget | null) => void;
  } = $props();

  const verdict = $derived(verdictOf(spot.verdict));
  const terms = $derived(termsIn(spot.numbers.map((n) => n.label)));
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
      <span class="chip stack num" title="Dein Stack in Big Blinds, als du entschieden hast">{stack} bb</span>
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
      <span class="time num" title="Gespielt um {time} Uhr">{time}</span>
    </header>
    <p class="action" id="spot-{index}-title">{spot.action}</p>
    {#if spot.better}
      <p class="better"><span>Besser</span><b>{spot.better}</b></p>
    {/if}
    <p class="why">{spot.why}</p>
    {#if spot.numbers.length}
      <dl class="figures">
        {#each spot.numbers as n, i (i)}
          <div class="fig" class:guess={n.source !== "berechnet"}>
            <dt>{n.label}</dt>
            <dd>
              <b class:num={n.source === "berechnet"}>{n.value}</b>
              <span
                class="src"
                title={n.source === "berechnet"
                  ? "Von Ringsides Rechenkern berechnet, nicht vom Sprachmodell"
                  : "Einschätzung des Sprachmodells, nicht nachgerechnet"}>{n.source === "berechnet" ? "berechnet" : "Einschätzung"}</span
              >
            </dd>
          </div>
        {/each}
      </dl>
      {#if terms.length}
        <ul class="terms" aria-label="Begriffe">
          {#each terms as t (t.term)}<li><b>{t.term}</b> {t.meaning}</li>{/each}
        </ul>
      {/if}
    {/if}
    <footer>
      <div class="learn">
        <span class="topic"><span class="faint">Thema</span> {conceptName(spot.concept)}</span>
        {#if practice.train}
          <button class="btn small" onclick={() => onTraining(practice.train)} title="Passende Übung im Lernbereich öffnen">
            Üben
            <svg viewBox="0 0 24 24" width="13" height="13" aria-hidden="true"><path d="M5 12h14M13 6l6 6-6 6" /></svg>
          </button>
        {/if}
        {#if practice.article}
          <button class="link" onclick={() => onTraining({ article: practice.article!.id })}>Nachlesen</button>
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
    color: var(--text-3);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .time {
    margin-left: auto;
    font-size: 12px;
    color: var(--text-3);
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
  .figures {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: 6px;
    margin: 2px 0 0;
  }
  .fig {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 7px 10px 8px;
    border-radius: var(--r-sm);
    background: var(--bg);
    border: 1px solid transparent;
  }
  .fig.guess {
    background: transparent;
    border: 1px dashed var(--line-2);
  }
  dt {
    font-size: 12px;
    color: var(--text-3);
  }
  dd {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 8px;
    margin: 0;
  }
  dd b {
    font-weight: 600;
    font-size: 13.5px;
    min-width: 0;
  }
  dd b.num {
    font-size: 17px;
  }
  .src {
    flex-shrink: 0;
    padding: 1px 6px;
    border-radius: 4px;
    font-size: 11px;
    background: var(--surface-3);
    color: var(--text);
  }
  .guess .src {
    background: none;
    padding: 0;
    color: var(--text-3);
  }

  .terms {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin: 0;
    padding: 0;
    font-size: 12.5px;
    line-height: 1.5;
    color: var(--text-2);
  }
  .terms b {
    display: inline-block;
    min-width: 9.5em;
    margin-right: 6px;
    font-weight: 600;
    color: var(--text);
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
    height: 28px;
    padding: 0 10px;
    font-size: 12.5px;
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

  @container (max-width: 700px) {
    .spot {
      grid-template-columns: minmax(0, 1fr);
    }
    .visual {
      max-width: 300px;
    }
  }
</style>
