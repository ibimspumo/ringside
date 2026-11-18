<script lang="ts">
  import { cardFace } from "./cards";

  /**
   * A playing card. `reveal` turns it face up when it appears (after `delay` ms), for dealt
   * boards and showdowns; reduced motion shows it at once.
   */
  let {
    card,
    size = 34,
    hidden = false,
    reveal = false,
    delay = 0,
  }: { card?: string; size?: number; hidden?: boolean; reveal?: boolean; delay?: number } = $props();

  const face = $derived(card ? cardFace(card) : undefined);
  const back = $derived(hidden || !face);
</script>

<span
  class="card"
  class:back
  class:ten={face?.rank === "10"}
  class:reveal={reveal && !back}
  style:--w="{size}px"
  style:--c={face?.suit.color}
  style:--delay="{delay}ms"
  role="img"
  aria-label={back ? "verdeckte Karte" : `${face?.rank} ${face?.suit.name}`}
>
  {#if !back && face}
    <b>{face.rank}</b>
    <svg viewBox="0 0 100 100" aria-hidden="true"><path d={face.suit.path} /></svg>
  {/if}
</span>

<style>
  .card {
    display: inline-flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: calc(var(--w) * 0.04);
    width: var(--w);
    height: calc(var(--w) * 1.4);
    border-radius: max(3px, calc(var(--w) * 0.12));
    background: var(--card-face);
    box-shadow:
      0 0 0 1px rgba(0, 0, 0, 0.35),
      0 1px 3px rgba(0, 0, 0, 0.35);
    color: var(--c);
    flex-shrink: 0;
    user-select: none;
  }
  b {
    font-family: var(--font-num);
    font-size: calc(var(--w) * 0.56);
    font-weight: 700;
    line-height: 0.9;
  }
  .ten b {
    font-size: calc(var(--w) * 0.48);
    letter-spacing: -0.06em;
    margin-left: -0.06em;
  }
  svg {
    width: calc(var(--w) * 0.42);
    height: calc(var(--w) * 0.42);
    fill: var(--c);
  }
  .back {
    background:
      repeating-linear-gradient(135deg, rgba(236, 235, 231, 0.06) 0 1px, transparent 1px 4px),
      var(--surface-3);
    box-shadow:
      0 0 0 1px rgba(0, 0, 0, 0.5),
      inset 0 0 0 max(2px, calc(var(--w) * 0.08)) var(--surface-2),
      inset 0 0 0 max(3px, calc(var(--w) * 0.1)) rgba(236, 235, 231, 0.14),
      0 1px 3px rgba(0, 0, 0, 0.35);
  }
  .reveal {
    animation: flip var(--dur-slow) var(--ease-enter) var(--delay) both;
  }
  @keyframes flip {
    from {
      opacity: 0;
      transform: perspective(400px) rotateY(80deg) translateY(-4px);
    }
  }
</style>
