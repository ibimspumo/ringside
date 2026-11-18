<script lang="ts">
  import { cardFace } from "./cards";

  /**
   * A card inside text, tables and tags: rank and suit on a small face, four-colour deck. A rank
   * alone ("K") is a card of any suit, as in hand classes like K6o; it shows without a suit.
   */
  let { card, dim = false }: { card: string; dim?: boolean } = $props();

  const f = $derived(cardFace(card));
  const rank = $derived(/^(10|[2-9TJQKA])$/i.test(card.trim()) ? (/^(T|10)$/i.test(card.trim()) ? "10" : card.trim().toUpperCase()) : null);
</script>

{#if rank}
  <span class="ui-mini any" class:faded={dim} class:ten={rank === "10"} role="img" aria-label="{rank}, beliebige Farbe">
    <b>{rank}</b>
  </span>
{:else if f}
  <span class="ui-mini" class:faded={dim} class:ten={f.rank === "10"} style:--c={f.suit.color} role="img" aria-label="{f.rank} {f.suit.name}">
    <b>{f.rank}</b><svg viewBox="0 0 100 100" aria-hidden="true"><path d={f.suit.path} /></svg>
  </span>
{:else}
  <span>{card}</span>
{/if}

<style>
  .ui-mini {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 1px;
    height: 1.45em;
    min-width: 1.75em;
    padding: 0 3px;
    border-radius: var(--r-xs);
    background: var(--card-face);
    box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.25);
    color: var(--c);
    font-family: var(--font-num);
    font-weight: var(--fw-bold);
    line-height: 1;
    vertical-align: -0.28em;
    transition: opacity var(--dur) var(--ease-standard);
  }
  b {
    font-size: 0.92em;
  }
  .ten b {
    letter-spacing: -0.06em;
  }
  svg {
    width: 0.72em;
    height: 0.72em;
    fill: var(--c);
  }
  .any {
    --c: var(--bg);
    min-width: 1.3em;
  }
  .faded {
    opacity: 0.4;
  }
</style>
