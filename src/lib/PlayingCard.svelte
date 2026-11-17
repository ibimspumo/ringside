<script lang="ts">
  let { card, size = 34, hidden = false }: { card?: string; size?: number; hidden?: boolean } = $props();

  // Four-colour deck. Suits are drawn as paths: the Unicode glyphs turn into emoji on some Windows fonts.
  const SUITS: Record<string, { color: string; path: string; name: string }> = {
    s: {
      name: "Pik",
      color: "#1d2026",
      path: "M50 4C64 25 93 39 93 61c0 13-10 21-21 21-8 0-14-4-17-9 1 9 5 16 11 21H34c6-5 10-12 11-21-3 5-9 9-17 9C17 82 7 74 7 61 7 39 36 25 50 4z",
    },
    h: {
      name: "Herz",
      color: "#c8312a",
      path: "M50 92C20 68 5 51 5 32 5 17 16 7 29 7c9 0 16 5 21 13 5-8 12-13 21-13 13 0 24 10 24 25 0 19-15 36-45 60z",
    },
    d: { name: "Karo", color: "#2462c8", path: "M50 3l38 47-38 47-38-47z" },
    c: {
      name: "Kreuz",
      color: "#1f8446",
      path: "M32 30a18 18 0 1 0 36 0a18 18 0 1 0-36 0zM11 60a18 18 0 1 0 36 0a18 18 0 1 0-36 0zM53 60a18 18 0 1 0 36 0a18 18 0 1 0-36 0zM38 52a12 12 0 1 0 24 0a12 12 0 1 0-24 0zM46 58c-1 17-5 28-13 37h34c-8-9-12-20-13-37z",
    },
  };

  const rank = $derived(card ? (card[0] === "T" ? "10" : card[0].toUpperCase()) : "");
  const suit = $derived(card ? SUITS[card[1]?.toLowerCase()] : undefined);
  const back = $derived(hidden || !card || !suit);
</script>

<span
  class="card"
  class:back
  class:ten={rank === "10"}
  style="--w:{size}px; --c:{suit?.color ?? '#1d2026'}"
  role="img"
  aria-label={back ? "verdeckte Karte" : `${rank} ${suit?.name}`}
>
  {#if !back && suit}
    <b>{rank}</b>
    <svg viewBox="0 0 100 100" aria-hidden="true"><path d={suit.path} /></svg>
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
    background: #f3f2ee;
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
      #2b3038;
    box-shadow:
      0 0 0 1px rgba(0, 0, 0, 0.5),
      inset 0 0 0 max(2px, calc(var(--w) * 0.08)) #22262c,
      inset 0 0 0 max(3px, calc(var(--w) * 0.1)) rgba(236, 235, 231, 0.14),
      0 1px 3px rgba(0, 0, 0, 0.35);
  }
</style>
