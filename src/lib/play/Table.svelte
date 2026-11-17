<script lang="ts">
  import PlayingCard from "../PlayingCard.svelte";
  import { GRADE, choiceShort, num } from "./labels";
  import type { HandView } from "./types";

  /** The table at one moment of the hand: `at` indexes `view.snapshots`. */
  let { view, at, final }: { view: HandView; at: number; final: boolean } = $props();

  const snap = $derived(view.snapshots[Math.min(at, view.snapshots.length - 1)]);
  const n = $derived(view.players);
  const button = $derived(n - 3);

  /** Seats clockwise from the hero at the bottom, in % of the stage. */
  function place(seat: number, r = 1) {
    const rel = (seat - view.hero + n) % n;
    const a = Math.PI / 2 + (rel * 2 * Math.PI) / n;
    return { x: 50 + 43 * r * Math.cos(a), y: 50 + 39 * r * Math.sin(a) };
  }

  /** Each seat's last move on the street shown, with the grade of a hero move. */
  const last = $derived.by(() => {
    const out: ({ text: string; grade: string | null } | null)[] = new Array(n).fill(null);
    for (const e of view.events) {
      if (e.snapshot > at) break;
      if (e.type === "board") out.fill(null);
      else if (e.street === snap.street || e.choice.kind === "fold") {
        const v = e.verdict !== null ? view.verdicts[e.verdict] : null;
        out[e.seat] = { text: choiceShort(e.choice), grade: v?.judgement ? GRADE[v.judgement.grade].color : null };
      }
    }
    return out;
  });

  const middle = $derived(snap.pot - snap.bets.reduce((a, b) => a + b, 0));
  const shown = $derived(final && view.outcome ? view.outcome.shown : []);
  const winners = $derived(final && view.outcome ? view.outcome.winners : []);
</script>

<div class="stage" aria-label="Pokertisch">
  <div class="rail"><div class="cloth"></div></div>

  <div class="center">
    <div class="board" aria-label="Board">
      {#each [0, 1, 2, 3, 4] as i (i)}
        {#if snap.board[i]}
          <span class="deal"><PlayingCard card={snap.board[i]} size={50} /></span>
        {:else}
          <span class="slot"></span>
        {/if}
      {/each}
    </div>
    <div class="pot num">Pot {num(middle)} BB</div>
  </div>

  {#each Array.from({ length: n }, (_, s) => s) as s (s)}
    {@const p = place(s)}
    {@const b = place(s, s === view.hero ? 0.36 : 0.58)}
    {@const hero = s === view.hero}
    {@const out = snap.folded[s]}
    {#if snap.bets[s] > 0}
      <div class="bet num" style="left:{b.x}%; top:{b.y}%">
        <span class="chip" aria-hidden="true"></span>{num(snap.bets[s])}
      </div>
    {/if}
    {#if s === button}
      {@const d = place(s, 0.7)}
      <span class="dealer" style="left:{d.x + (s === view.hero ? 11 : 5)}%; top:{d.y}%" aria-label="Dealer-Button">D</span>
    {/if}
    <div class="seat" class:hero class:out class:acting={snap.to_act === s} class:won={winners.includes(s)} style="left:{p.x}%; top:{p.y}%">
      {#if hero}
        <div class="hole big">
          <PlayingCard card={view.hero_cards[0]} size={58} />
          <PlayingCard card={view.hero_cards[1]} size={58} />
        </div>
      {:else if !out || shown[s]}
        <div class="hole">
          <PlayingCard card={shown[s]?.[0]} hidden={!shown[s]} size={26} />
          <PlayingCard card={shown[s]?.[1]} hidden={!shown[s]} size={26} />
        </div>
      {/if}
      <div class="plate">
        <b>{view.positions[s]}{hero ? " · Du" : ""}</b>
        <span class="num">{num(snap.stacks[s])} BB</span>
      </div>
      {#if last[s]}
        <span class="move" style:--g={last[s]?.grade ?? "transparent"} class:graded={!!last[s]?.grade}>{last[s]?.text}</span>
      {/if}
      {#if final && view.outcome?.made[s]}
        <span class="made">{view.outcome.made[s]}</span>
      {/if}
    </div>
  {/each}
</div>

<style>
  .stage {
    position: relative;
    width: 100%;
    aspect-ratio: 16 / 8.6;
    min-height: 330px;
    max-height: 56vh;
    margin: 0 auto;
    container-type: inline-size;
  }
  .rail {
    position: absolute;
    inset: 11% 9%;
    border-radius: 999px;
    padding: 1.1%;
    background: #1c1f24;
    box-shadow:
      inset 0 0 0 1px rgba(236, 235, 231, 0.06),
      0 22px 50px rgba(0, 0, 0, 0.5);
  }
  .cloth {
    height: 100%;
    border-radius: 999px;
    background: radial-gradient(ellipse at 50% 40%, #26372f 0%, #1c2925 60%, #16201d 100%);
    box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.45), inset 0 0 60px rgba(0, 0, 0, 0.35);
  }
  .center {
    position: absolute;
    left: 50%;
    top: 46%;
    transform: translate(-50%, -50%);
    display: grid;
    justify-items: center;
    gap: 10px;
  }
  .board {
    display: flex;
    gap: 6px;
  }
  .slot {
    width: 50px;
    height: 70px;
    border-radius: 6px;
    border: 1px dashed rgba(236, 235, 231, 0.1);
  }
  .deal {
    animation: deal 0.28s ease-out;
  }
  @keyframes deal {
    from {
      opacity: 0;
      transform: translateY(-6px);
    }
  }
  .pot {
    font-size: 15px;
    color: var(--text);
    background: rgba(10, 12, 14, 0.55);
    padding: 3px 12px;
    border-radius: 999px;
  }
  .seat {
    position: absolute;
    transform: translate(-50%, -50%);
    display: grid;
    justify-items: center;
    gap: 4px;
    transition: opacity 0.2s;
  }
  .seat.out {
    opacity: 0.42;
  }
  .hole {
    display: flex;
    gap: 3px;
    margin-bottom: -10px;
    z-index: 0;
  }
  .hole.big {
    gap: 5px;
    margin-bottom: -14px;
  }
  .plate {
    position: relative;
    z-index: 1;
    display: grid;
    justify-items: center;
    min-width: 96px;
    padding: 5px 12px;
    border-radius: 9px;
    background: var(--plate);
    box-shadow:
      inset 0 0 0 1px var(--plate-edge),
      0 6px 16px rgba(0, 0, 0, 0.4);
    font-size: 12px;
    line-height: 1.3;
  }
  .plate b {
    font-weight: 600;
    letter-spacing: 0.02em;
  }
  .plate span {
    color: var(--text-2);
    font-size: 13px;
  }
  .hero .plate {
    min-width: 128px;
    background: #31353d;
  }
  .acting .plate {
    box-shadow:
      inset 0 0 0 1.5px var(--focus),
      0 0 0 4px color-mix(in srgb, var(--focus) 18%, transparent),
      0 6px 16px rgba(0, 0, 0, 0.4);
  }
  .won .plate {
    box-shadow:
      inset 0 0 0 1.5px var(--pos),
      0 6px 16px rgba(0, 0, 0, 0.4);
  }
  .move {
    font-size: 11.5px;
    padding: 1px 8px;
    border-radius: 999px;
    background: rgba(10, 12, 14, 0.7);
    color: var(--text-2);
    white-space: nowrap;
  }
  .move.graded {
    color: var(--text);
    box-shadow: inset 0 0 0 1.5px var(--g);
  }
  .made {
    font-size: 11px;
    color: var(--amber);
  }
  .bet {
    position: absolute;
    transform: translate(-50%, -50%);
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 13px;
    padding: 2px 8px 2px 4px;
    border-radius: 999px;
    background: rgba(10, 12, 14, 0.6);
  }
  .chip {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: radial-gradient(circle, #f2b12e 0 38%, #c6831a 40% 62%, #f2b12e 64%);
    box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.5);
  }
  .dealer {
    position: absolute;
    transform: translate(-50%, -50%);
    width: 20px;
    height: 20px;
    border-radius: 50%;
    background: var(--text);
    color: var(--bg);
    font-size: 11px;
    font-weight: 700;
    display: grid;
    place-items: center;
  }
  @container (max-width: 640px) {
    .plate {
      min-width: 76px;
      padding: 4px 8px;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .deal {
      animation: none;
    }
    .seat {
      transition: none;
    }
  }
</style>
