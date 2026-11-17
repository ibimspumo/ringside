<script lang="ts">
  import PlayingCard from "../PlayingCard.svelte";
  import { RANKS } from "./range";

  /** Three to five board cards, picked from the deck in dealing order. */
  let { board = $bindable([]), onEdit }: { board: string[]; onEdit?: () => void } = $props();

  const SUITS = ["s", "h", "d", "c"];
  const STREET = ["Flop", "Flop", "Flop", "Turn", "River"];

  function toggle(card: string) {
    if (board.includes(card)) board = board.filter((c) => c !== card);
    else if (board.length < 5) board = [...board, card];
    else return;
    onEdit?.();
  }
</script>

<div class="picker">
  <div class="slots" aria-label="Board">
    {#each STREET as street, i (i)}
      <div class="slot" class:gap={i === 3 || i === 4}>
        {#if board[i]}
          <button class="picked" onclick={() => toggle(board[i])} title="{board[i]} entfernen">
            <PlayingCard card={board[i]} size={40} />
          </button>
        {:else}
          <span class="empty" class:need={i < 3}>{street}</span>
        {/if}
      </div>
    {/each}
    {#if board.length}
      <button class="btn ghost clear" onclick={() => ((board = []), onEdit?.())}>Leeren</button>
    {/if}
  </div>
  <div class="deck" role="group" aria-label="Karten">
    {#each SUITS as s (s)}
      {#each RANKS.split("") as r (r)}
        {@const card = r + s}
        {@const used = board.includes(card)}
        <button
          class="pick"
          class:used
          disabled={!used && board.length >= 5}
          onclick={() => toggle(card)}
          aria-pressed={used}
          aria-label={card}
        >
          <PlayingCard {card} size={24} />
        </button>
      {/each}
    {/each}
  </div>
</div>

<style>
  .picker {
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-width: 0;
  }
  .slots {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }
  .slot.gap {
    margin-left: 6px;
  }
  .picked {
    padding: 0;
    border: 0;
    background: none;
    cursor: pointer;
  }
  .empty {
    display: grid;
    place-items: center;
    width: 40px;
    height: 56px;
    border: 1px dashed var(--line-2);
    border-radius: 5px;
    color: var(--text-3);
    font-size: 10px;
  }
  .empty.need {
    border-color: color-mix(in srgb, var(--text-2) 50%, transparent);
  }
  .clear {
    height: 26px;
    margin-left: auto;
  }
  .deck {
    display: grid;
    grid-template-columns: repeat(13, minmax(0, 1fr));
    gap: 3px;
    justify-items: center;
  }
  .pick {
    padding: 0;
    border: 0;
    background: none;
    cursor: pointer;
    border-radius: 4px;
    line-height: 0;
  }
  .pick:hover:not(:disabled) {
    transform: translateY(-2px);
  }
  .pick.used {
    opacity: 0.22;
  }
  .pick:disabled {
    cursor: default;
    opacity: 0.55;
  }
</style>
