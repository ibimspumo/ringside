<script lang="ts">
  import { splitCards, splitHands } from "./cards";
  import MiniCard from "./MiniCard.svelte";

  /**
   * Text in which every card notation ("Th Tc 9h", "A♠K♠") shows as a small card. With `hands`,
   * hand classes ("K6o", "99", "T9s") show as rank cards too, for coaching texts.
   */
  let { text, hands = false }: { text: string; hands?: boolean } = $props();

  const parts = $derived(hands ? splitHands(text) : splitCards(text));
</script>

<span class="ui-card-text"
  >{#each parts as p, i (i)}{#if "card" in p}<MiniCard card={p.card} />{:else if "hand" in p}<span class="hand" title={p.title}
        ><MiniCard card={p.hand[0]} /><MiniCard card={p.hand[1]} />{#if p.kind}<span class="kind">{p.kind}</span>{/if}</span
      >{:else}{p.text}{/if}{/each}</span
>

<style>
  .ui-card-text {
    white-space: normal;
  }
  .ui-card-text :global(.ui-mini) {
    margin-inline: 1px;
  }
  .hand {
    white-space: nowrap;
    cursor: help;
  }
  .kind {
    margin-left: 1px;
    font-weight: var(--fw-semibold);
    color: var(--text-2);
  }
</style>
