<script lang="ts">
  import { splitCards } from "./cards";
  import MiniCard from "./MiniCard.svelte";

  /** Text in which every card notation ("Th Tc 9h", "A♠K♠") shows as a small card. */
  let { text }: { text: string } = $props();

  const parts = $derived(splitCards(text));
</script>

<span class="ui-card-text"
  >{#each parts as p, i (i)}{#if "card" in p}<MiniCard card={p.card} />{:else}{p.text}{/if}{/each}</span
>

<style>
  .ui-card-text {
    white-space: normal;
  }
  .ui-card-text :global(.ui-mini) {
    margin-inline: 1px;
  }
</style>
