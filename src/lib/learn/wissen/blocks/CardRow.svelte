<script lang="ts">
  import PlayingCard from "../../../ui/PlayingCard.svelte";

  let { cards, size = 40, highlight = null }: { cards: string[]; size?: number; highlight?: string[] | null } = $props();
</script>

<span class="row">
  {#each cards as c (c)}
    <span class="slot" class:off={highlight && !highlight.includes(c)} class:on={highlight?.includes(c)}>
      <PlayingCard card={c} {size} />
    </span>
  {/each}
</span>

<style>
  .row {
    display: inline-flex;
    gap: 5px;
  }
  .slot {
    display: inline-flex;
    border-radius: 6px;
    transition: opacity 0.15s;
  }
  .slot.off {
    opacity: 0.38;
  }
  .slot.on {
    box-shadow: 0 0 0 2px var(--text);
  }
</style>
