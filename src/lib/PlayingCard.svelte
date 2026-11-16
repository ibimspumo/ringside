<script lang="ts">
  let { card, size = 34, hidden = false }: { card?: string; size?: number; hidden?: boolean } = $props();

  const SUITS: Record<string, { glyph: string; color: string }> = {
    s: { glyph: "♠", color: "#1c2230" },
    h: { glyph: "♥", color: "#d8343f" },
    d: { glyph: "♦", color: "#2a6fdb" },
    c: { glyph: "♣", color: "#1f9a57" },
  };

  const rank = $derived(card ? (card[0] === "T" ? "10" : card[0]) : "");
  const suit = $derived(card ? SUITS[card[1]] : undefined);
</script>

<span class="card" class:back={hidden || !card} style="--w:{size}px; --c:{suit?.color ?? '#333'}">
  {#if !hidden && card}
    <b>{rank}</b><i>{suit?.glyph}</i>
  {/if}
</span>

<style>
  .card {
    display: inline-flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    width: var(--w);
    height: calc(var(--w) * 1.38);
    border-radius: calc(var(--w) * 0.14);
    background: linear-gradient(180deg, #ffffff, #e9ecf2);
    box-shadow:
      0 2px 6px rgba(0, 0, 0, 0.45),
      0 0 0 1px rgba(0, 0, 0, 0.25);
    color: var(--c);
    line-height: 0.95;
    font-family: var(--font-num);
    flex-shrink: 0;
  }
  b {
    font-size: calc(var(--w) * 0.5);
    font-weight: 700;
    letter-spacing: -0.04em;
  }
  i {
    font-style: normal;
    font-size: calc(var(--w) * 0.46);
  }
  .back {
    background:
      repeating-linear-gradient(45deg, rgba(255, 255, 255, 0.07) 0 3px, transparent 3px 7px),
      linear-gradient(180deg, #ff8a3d, #c4451a);
  }
</style>
