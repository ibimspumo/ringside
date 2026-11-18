<script lang="ts">
  import { bestFive, CATEGORY, categoryOf } from "../learn/wissen/poker";
  import { cardFace } from "./cards";
  import MiniCard from "./MiniCard.svelte";
  import PlayingCard from "./PlayingCard.svelte";

  /**
   * A player's hand at showdown: hole cards and board side by side, the five cards of the best
   * hand lit and lifted, the rest faded, and the hand's name with its five cards below.
   */
  let {
    hole,
    board,
    winner = false,
    size = 38,
    reveal = false,
  }: {
    hole: string[];
    board: string[];
    winner?: boolean;
    size?: number;
    reveal?: boolean;
  } = $props();

  const all = $derived([...hole, ...board]);
  const best = $derived(bestFive(all));
  const lit = $derived(new Set(best.map((c) => cardFace(c)?.key)));
  const name = $derived(CATEGORY[categoryOf(all)]);
  const on = (c: string) => lit.has(cardFace(c)?.key);
</script>

<div class="ui-best" class:winner>
  <div class="cards" aria-hidden="true">
    <span class="group">
      {#each hole as c, k (c)}<span class="slot" class:on={on(c)}><PlayingCard card={c} {size} {reveal} delay={k * 60} /></span>{/each}
    </span>
    <span class="group">
      {#each board as c (c)}<span class="slot" class:on={on(c)}><PlayingCard card={c} size={size - 4} /></span>{/each}
    </span>
  </div>
  <p class="name">
    <b>{name}</b>
    <span class="five">{#each best as c (c)}<MiniCard card={c} />{/each}</span>
  </p>
</div>

<style>
  .ui-best {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    min-width: 0;
  }
  .cards {
    display: flex;
    align-items: flex-end;
    gap: var(--sp-3);
  }
  .group {
    display: flex;
    gap: 3px;
  }
  .slot {
    display: inline-flex;
    border-radius: 5px;
    opacity: 0.32;
    filter: saturate(0.4);
    transition:
      opacity var(--dur-slow) var(--ease-standard),
      transform var(--dur-slow) var(--ease-spring),
      filter var(--dur-slow) var(--ease-standard);
  }
  .slot.on {
    opacity: 1;
    filter: none;
    transform: translateY(-4px);
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--text) 70%, transparent);
  }
  .winner .slot.on {
    box-shadow:
      0 0 0 2px var(--pos),
      0 4px 14px color-mix(in srgb, var(--pos) 35%, transparent);
  }
  .name {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2);
    margin: 0;
    font-size: var(--fs-md);
  }
  .name b {
    font-weight: var(--fw-semibold);
  }
  .winner .name b {
    color: var(--pos);
  }
  .five {
    display: inline-flex;
    gap: 2px;
    font-size: var(--fs-sm);
  }
</style>
