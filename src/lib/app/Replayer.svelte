<script lang="ts">
  import Felt from "../Felt.svelte";
  import PlayingCard from "../PlayingCard.svelte";
  import { money } from "../stats";
  import type { Action, Hand, Street } from "../types";

  let { hand }: { hand: Hand } = $props();

  type Event =
    | { type: "action"; action: Action }
    | { type: "street"; street: Street }
    | { type: "showdown" }
    | { type: "collect" };

  const STREETS: Street[] = ["preflop", "flop", "turn", "river"];
  const BOARD_AT: Record<Street, number> = { preflop: 0, flop: 3, turn: 4, river: 5 };

  const events = $derived.by(() => {
    const out: Event[] = [];
    let street: Street = "preflop";
    for (const action of hand.actions) {
      while (action.street !== street) {
        street = STREETS[STREETS.indexOf(street) + 1];
        out.push({ type: "street", street });
      }
      out.push({ type: "action", action });
    }
    while (STREETS.indexOf(street) < 3 && hand.board.length > BOARD_AT[street]) {
      street = STREETS[STREETS.indexOf(street) + 1];
      out.push({ type: "street", street });
    }
    if (hand.shown.length) out.push({ type: "showdown" });
    out.push({ type: "collect" });
    return out;
  });

  let step = $state(0);
  let playing = $state(false);
  let inBB = $state(false);

  $effect(() => {
    void hand.id;
    step = 0;
    playing = false;
  });

  $effect(() => {
    if (!playing) return;
    const t = setInterval(() => {
      if (step >= events.length) playing = false;
      else step++;
    }, 850);
    return () => clearInterval(t);
  });

  const LABEL: Record<string, string> = {
    post_small_blind: "SB",
    post_big_blind: "BB",
    post_ante: "Ante",
    post_both: "SB+BB",
    fold: "Fold",
    check: "Check",
    call: "Call",
    bet: "Bet",
    raise: "Raise",
  };

  const table = $derived.by(() => {
    const stacks = new Map(hand.seats.map((s) => [s.name, s.stack]));
    const put = new Map<string, number>();
    const label = new Map<string, string>();
    const folded = new Set<string>();
    let pot = 0;
    let board = 0;
    let revealed = false;
    let winners = new Map<string, number>();
    let street: Street = "preflop";
    let last: string | null = null;

    const sweep = () => {
      for (const v of put.values()) pot += v;
      put.clear();
    };

    for (const e of events.slice(0, step)) {
      last = null;
      if (e.type === "action") {
        const a = e.action;
        stacks.set(a.player, (stacks.get(a.player) ?? 0) - a.amount);
        if (a.kind === "post_ante") pot += a.amount;
        else put.set(a.player, (put.get(a.player) ?? 0) + a.amount);
        if (a.kind === "fold") folded.add(a.player);
        label.set(a.player, a.all_in ? "All-in" : LABEL[a.kind]);
        last = a.player;
      } else if (e.type === "street") {
        sweep();
        street = e.street;
        board = BOARD_AT[e.street];
        for (const name of label.keys()) if (!folded.has(name)) label.delete(name);
      } else if (e.type === "showdown") {
        sweep();
        revealed = true;
      } else {
        sweep();
        for (const [name, amount] of hand.returned) {
          stacks.set(name, (stacks.get(name) ?? 0) + amount);
          pot -= amount;
        }
        winners = new Map(hand.collected);
        for (const [name, amount] of hand.collected) stacks.set(name, (stacks.get(name) ?? 0) + amount);
        pot = 0;
      }
    }
    return { stacks, put, label, folded, pot, board, revealed, winners, street, last };
  });

  const heroSeat = $derived(hand.seats.find((s) => s.name === hand.hero)?.seat ?? 1);

  function place(seat: number) {
    const n = Math.max(hand.max_seats, 2);
    const visual = (((seat - heroSeat) % n) + n) % n;
    const a = Math.PI / 2 + (visual * 2 * Math.PI) / n;
    return { x: 0.5 + 0.4 * Math.cos(a), y: 0.47 + 0.36 * Math.sin(a) };
  }

  function amount(v: number) {
    if (inBB && hand.big_blind > 0) return `${+(v / hand.big_blind).toFixed(1)} BB`;
    return money(v, hand.currency, hand.game);
  }

  function cardsOf(name: string): string[] | null {
    if (name === hand.hero) return hand.hero_cards;
    if (table.revealed) return hand.shown.find(([n]) => n === name)?.[1] ?? null;
    return null;
  }

  function key(e: KeyboardEvent) {
    if (e.key === "ArrowRight") step = Math.min(step + 1, events.length);
    if (e.key === "ArrowLeft") step = Math.max(step - 1, 0);
    if (e.key === " ") playing = !playing;
  }

  const STREET_NAME: Record<Street, string> = { preflop: "Preflop", flop: "Flop", turn: "Turn", river: "River" };
</script>

<svelte:window onkeydown={key} />

<div class="replayer">
  <div class="stage">
    <Felt>
      <div class="center">
        <div class="board">
          {#each hand.board.slice(0, table.board) as c (c)}
            <PlayingCard card={c} size={44} />
          {/each}
        </div>
        {#if table.pot > 0}
          <div class="pot num">Pot {amount(table.pot)}</div>
        {/if}
        <div class="street">{STREET_NAME[table.street]}</div>
      </div>

      {#each hand.seats.filter((s) => !s.sitting_out) as s (s.seat)}
        {@const p = place(s.seat)}
        {@const cards = cardsOf(s.name)}
        {@const bet = table.put.get(s.name) ?? 0}
        {@const won = table.winners.get(s.name)}
        <div
          class="seat"
          class:folded={table.folded.has(s.name)}
          class:acting={table.last === s.name}
          class:winner={won !== undefined}
          style="left:{p.x * 100}%; top:{p.y * 100}%"
        >
          <div class="cards">
            {#if cards}
              {#each cards as c (c)}<PlayingCard card={c} size={30} />{/each}
            {:else if !table.folded.has(s.name)}
              <PlayingCard hidden size={30} /><PlayingCard hidden size={30} />
            {/if}
          </div>
          <div class="pod">
            <span class="name">{s.name}</span>
            <span class="num stack">{amount(table.stacks.get(s.name) ?? 0)}</span>
            {#if table.label.get(s.name)}
              <span class="act">{table.label.get(s.name)}</span>
            {/if}
            {#if won !== undefined}
              <span class="act win num">+{amount(won)}</span>
            {/if}
          </div>
          {#if s.seat === hand.button_seat}<span class="dealer">D</span>{/if}
          {#if bet > 0}
            <div class="bet num" style="--dx:{(0.5 - p.x) * 220}px; --dy:{(0.47 - p.y) * 150}px">
              <i></i>{amount(bet)}
            </div>
          {/if}
        </div>
      {/each}
    </Felt>
  </div>

  <div class="controls">
    <button class="btn" onclick={() => (step = 0)} title="Anfang">⏮</button>
    <button class="btn" onclick={() => (step = Math.max(step - 1, 0))} title="Zurück (←)">◀</button>
    <button class="btn primary play" onclick={() => (playing = !playing)} title="Abspielen (Leertaste)"
      >{playing ? "Pause" : "Abspielen"}</button
    >
    <button class="btn" onclick={() => (step = Math.min(step + 1, events.length))} title="Weiter (→)">▶</button>
    <button class="btn" onclick={() => (step = events.length)} title="Ende">⏭</button>
    <input type="range" min="0" max={events.length} bind:value={step} />
    <div class="seg">
      <button class:on={!inBB} onclick={() => (inBB = false)}>{hand.game === "tournament" ? "Chips" : "Geld"}</button>
      <button class:on={inBB} onclick={() => (inBB = true)}>BB</button>
    </div>
  </div>
</div>

<style>
  .replayer {
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-width: 0;
  }
  .stage {
    position: relative;
    aspect-ratio: 16 / 10;
    border-radius: 14px;
    overflow: hidden;
    border: 1px solid var(--line);
  }
  .center {
    position: absolute;
    left: 50%;
    top: 46%;
    transform: translate(-50%, -50%);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
  }
  .board {
    display: flex;
    gap: 5px;
    min-height: 61px;
  }
  .pot {
    padding: 3px 12px;
    border-radius: 99px;
    background: rgba(0, 0, 0, 0.45);
    font-size: 14px;
  }
  .street {
    font-size: 11px;
    letter-spacing: 0.2em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.35);
  }
  .seat {
    position: absolute;
    transform: translate(-50%, -50%);
    display: flex;
    flex-direction: column;
    align-items: center;
    transition: opacity 0.25s;
  }
  .seat.folded {
    opacity: 0.38;
  }
  .cards {
    display: flex;
    gap: 3px;
    height: 42px;
    margin-bottom: -10px;
    z-index: 1;
  }
  .pod {
    position: relative;
    min-width: 112px;
    padding: 12px 12px 6px;
    border-radius: 10px;
    background: linear-gradient(180deg, #1d2230, #10131a);
    border: 1px solid rgba(255, 255, 255, 0.1);
    text-align: center;
    box-shadow: 0 6px 18px rgba(0, 0, 0, 0.5);
    transition:
      box-shadow 0.2s,
      border-color 0.2s;
  }
  .acting .pod {
    border-color: var(--brand);
    box-shadow: 0 0 0 3px rgba(255, 138, 61, 0.2);
  }
  .winner .pod {
    border-color: var(--win);
    box-shadow: 0 0 22px rgba(61, 220, 151, 0.25);
  }
  .name {
    display: block;
    max-width: 130px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12.5px;
    color: var(--text-dim);
  }
  .stack {
    display: block;
    font-size: 15px;
  }
  .act {
    position: absolute;
    left: 50%;
    bottom: -11px;
    transform: translateX(-50%);
    padding: 1px 8px;
    border-radius: 99px;
    background: var(--brand);
    color: #1b0d03;
    font-size: 11px;
    font-weight: 700;
    white-space: nowrap;
  }
  .act.win {
    background: var(--win);
  }
  .dealer {
    position: absolute;
    right: -12px;
    top: 34px;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    background: #f4f4f4;
    color: #111;
    font-size: 11px;
    font-weight: 800;
    display: grid;
    place-items: center;
    box-shadow: 0 2px 4px rgba(0, 0, 0, 0.5);
  }
  .bet {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(calc(-50% + var(--dx)), calc(-50% + var(--dy)));
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 2px 8px 2px 3px;
    border-radius: 99px;
    background: rgba(0, 0, 0, 0.55);
    font-size: 13px;
    white-space: nowrap;
  }
  .bet i {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: radial-gradient(circle, #fff 0 25%, #ff8a3d 26% 60%, #fff 61% 70%, #ff8a3d 71%);
  }
  .controls {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .controls .btn {
    padding: 0 11px;
  }
  .play {
    min-width: 104px;
    justify-content: center;
  }
  input[type="range"] {
    flex: 1;
    margin: 0 8px;
    accent-color: var(--brand);
  }
</style>
