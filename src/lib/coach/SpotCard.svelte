<script lang="ts">
  import Felt from "../Felt.svelte";
  import PlayingCard from "../PlayingCard.svelte";
  import { conceptName } from "./concepts";
  import type { CoachSpot, Verdict } from "./types";

  let {
    spot,
    index,
    onHand,
    onTraining,
  }: { spot: CoachSpot; index: number; onHand: (id: number) => void; onTraining: () => void } = $props();

  const VERDICTS: Record<Verdict, string> = { stark: "Stark", ok: "Okay", fraglich: "Fraglich", fehler: "Fehler" };
  const stack = $derived(spot.stack_bb.toLocaleString("de-DE", { maximumFractionDigits: 1 }));
  const time = $derived(spot.played_at.slice(11, 16));
</script>

<article class="spot v-{spot.verdict}">
  <div class="visual">
    <div class="table" aria-label="Board und deine Karten">
      <Felt>
        <div class="board">
          {#each [0, 1, 2, 3, 4] as i (i)}
            {#if spot.board[i]}
              <PlayingCard card={spot.board[i]} size={26} />
            {:else}
              <span class="slot"></span>
            {/if}
          {/each}
        </div>
        <div class="hero">
          {#each spot.hero_cards as c (c)}<PlayingCard card={c} size={34} />{/each}
        </div>
      </Felt>
    </div>
    <div class="where">
      <span>{spot.position ?? "–"}</span>
      <span class="num">{stack} bb</span>
      <span class="num faint">{time}</span>
    </div>
  </div>

  <div class="text">
    <header>
      <span class="idx num">{index}</span>
      <span class="verdict">{VERDICTS[spot.verdict]}</span>
      <button class="concept" onclick={onTraining} title="Dieses Thema im Lernbereich üben">
        {conceptName(spot.concept)} <span aria-hidden="true">· Üben →</span>
      </button>
    </header>
    <p class="action">{spot.action}</p>
    <p class="why">{spot.why}</p>
    {#if spot.better}
      <p class="better"><span>Besser:</span> {spot.better}</p>
    {/if}
    {#if spot.numbers.length}
      <dl class="figures">
        {#each spot.numbers as n, i (i)}
          <div class="fig">
            <dt>{n.label}</dt>
            <dd>
              <b class:num={n.source === "berechnet"}>{n.value}</b>
              <span
                class="src {n.source === 'berechnet' ? 'calc' : 'guess'}"
                title={n.source === "berechnet"
                  ? "Von Ringsides Rechenkern berechnet, nicht vom Sprachmodell"
                  : "Einschätzung des Sprachmodells, nicht nachgerechnet"}>{n.source === "berechnet" ? "berechnet" : "Einschätzung"}</span
              >
            </dd>
          </div>
        {/each}
      </dl>
    {/if}
    <div class="actions">
      <button class="btn" onclick={() => onHand(spot.hand_id)}>
        <svg viewBox="0 0 24 24" width="15" height="15" aria-hidden="true"><path d="M8 5.5v13l10-6.5z" /></svg>
        Im Replayer ansehen
      </button>
    </div>
  </div>
</article>

<style>
  .spot {
    --accent: var(--text-3);
    display: grid;
    grid-template-columns: 236px minmax(0, 1fr);
    gap: 18px;
    padding: 16px 18px 16px 16px;
    border: 1px solid var(--line);
    border-left: 3px solid var(--accent);
    border-radius: var(--r-lg);
    background: var(--surface);
  }
  .v-stark {
    --accent: var(--pos);
  }
  .v-ok {
    --accent: var(--text-3);
  }
  .v-fraglich {
    --accent: var(--amber);
  }
  .v-fehler {
    --accent: var(--neg);
  }
  .visual {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .table {
    position: relative;
    aspect-ratio: 1.45;
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
  .slot {
    width: 26px;
    height: 36px;
    border-radius: 3px;
    border: 1px dashed rgba(236, 235, 231, 0.12);
  }
  .hero {
    position: absolute;
    left: 50%;
    bottom: 6%;
    transform: translateX(-50%);
    display: flex;
    gap: 3px;
  }
  .where {
    display: flex;
    justify-content: space-between;
    font-size: 12.5px;
    color: var(--text-2);
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
    flex-wrap: wrap;
    gap: 8px 10px;
  }
  .idx {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    background: var(--surface-3);
    font-size: 12px;
  }
  .verdict {
    font-weight: 600;
    font-size: 13px;
    color: var(--accent);
  }
  .v-ok .verdict {
    color: var(--text-2);
  }
  .concept {
    margin-left: auto;
    padding: 3px 9px;
    border: 1px solid var(--line-2);
    border-radius: 999px;
    background: transparent;
    color: var(--text-2);
    font-size: 12px;
    cursor: pointer;
  }
  .concept:hover {
    color: var(--text);
    background: var(--surface-2);
  }
  p {
    margin: 0;
  }
  .action {
    font-weight: 600;
    line-height: 1.4;
  }
  .why {
    color: var(--text-2);
    line-height: 1.5;
  }
  .better span {
    color: var(--text-3);
  }
  .figures {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(210px, 1fr));
    gap: 6px;
    margin: 2px 0 0;
  }
  .fig {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 7px 10px;
    border-radius: var(--r-sm);
    background: var(--bg);
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
    font-size: 16px;
  }
  .src {
    flex-shrink: 0;
    padding: 1px 6px;
    border-radius: 4px;
    font-size: 11px;
  }
  .src.calc {
    background: var(--surface-3);
    color: var(--text);
  }
  .src.guess {
    border: 1px dashed var(--line-2);
    color: var(--text-3);
  }
  .actions {
    display: flex;
    gap: 8px;
    margin-top: 2px;
  }
  .actions svg {
    fill: currentColor;
  }

  @container (max-width: 640px) {
    .spot {
      grid-template-columns: minmax(0, 1fr);
    }
    .visual {
      max-width: 300px;
    }
  }
</style>
