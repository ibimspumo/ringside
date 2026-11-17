<script lang="ts">
  /** A table seen from above. Seats are numbered clockwise from the bottom. */
  let {
    size,
    occupied,
    button,
    smallBlind = null,
    bigBlind = null,
    target = null,
    pickable = false,
    picked = null,
    truth = null,
    onPick,
  }: {
    size: number;
    occupied: number[];
    button: number;
    smallBlind?: number | null;
    bigBlind?: number | null;
    /** Seat the question is about. */
    target?: number | null;
    pickable?: boolean;
    picked?: number | null;
    /** Right seat after grading. */
    truth?: number | null;
    onPick?: (seat: number) => void;
  } = $props();

  const seats = $derived(Array.from({ length: size }, (_, i) => i + 1));
  const taken = $derived(new Set(occupied));

  function place(seat: number, r = 1) {
    const a = Math.PI / 2 + ((seat - 1) * 2 * Math.PI) / size;
    return { x: 50 + 42 * r * Math.cos(a), y: 50 + 38 * r * Math.sin(a) };
  }
</script>

<div class="ring" role="group" aria-label="Tisch mit {size} Sitzen">
  <div class="cloth"></div>
  {#each seats as s (s)}
    {@const p = place(s)}
    {@const m = place(s, 0.64)}
    {@const empty = !taken.has(s)}
    <button
      class="seat"
      class:empty
      class:target={target === s}
      class:picked={picked === s}
      class:truth={truth === s}
      class:miss={truth !== null && picked === s && truth !== s}
      style="left:{p.x}%; top:{p.y}%"
      disabled={!pickable || empty}
      aria-label="Sitz {s}{empty ? ', leer' : ''}{s === button ? ', Button' : ''}"
      onclick={() => onPick?.(s)}
    >
      <span class="n num">{s}</span>
    </button>
    {#if s === button || s === smallBlind || s === bigBlind}
      <span class="marks" style="left:{m.x}%; top:{m.y}%">
        {#if s === button}<span class="mk d" title="Dealer-Button">D</span>{/if}
        {#if s === smallBlind}<span class="mk">SB</span>{/if}
        {#if s === bigBlind}<span class="mk">BB</span>{/if}
      </span>
    {/if}
  {/each}
</div>

<style>
  .ring {
    position: relative;
    width: 100%;
    max-width: 520px;
    aspect-ratio: 16 / 10;
    margin: 0 auto;
  }
  .cloth {
    position: absolute;
    inset: 16% 14%;
    border-radius: 999px;
    background: radial-gradient(ellipse at 50% 42%, #23312d 0%, #1b2623 62%, #16201d 100%);
    box-shadow:
      0 0 0 6px #1c1f24,
      0 0 0 7px rgba(236, 235, 231, 0.06),
      0 14px 30px rgba(0, 0, 0, 0.4);
  }
  .seat {
    position: absolute;
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    transform: translate(-50%, -50%);
    border-radius: 50%;
    border: 1.5px solid var(--line-2);
    background: var(--surface-2);
    color: var(--text);
    cursor: pointer;
    transition:
      transform 0.15s,
      background 0.15s,
      border-color 0.15s;
  }
  .seat:disabled {
    cursor: default;
  }
  .seat:hover:not(:disabled) {
    background: var(--surface-3);
    border-color: var(--text-2);
    transform: translate(-50%, -50%) scale(1.08);
  }
  .seat.empty {
    background: transparent;
    border-style: dashed;
    color: var(--text-3);
  }
  .seat.target {
    border-color: var(--text);
    box-shadow: 0 0 0 4px rgba(236, 235, 231, 0.12);
    animation: glow 1.6s ease-in-out infinite;
  }
  .seat.picked {
    background: var(--text);
    color: var(--bg);
    border-color: var(--text);
  }
  .seat.truth {
    background: var(--pos);
    border-color: var(--pos);
    color: var(--bg);
    animation: pop 0.35s cubic-bezier(0.2, 0.9, 0.3, 1.4);
  }
  .seat.miss {
    background: var(--neg);
    border-color: var(--neg);
    color: #fff;
  }
  .n {
    font-size: 16px;
    font-weight: 600;
  }
  .marks {
    position: absolute;
    display: flex;
    gap: 3px;
    transform: translate(-50%, -50%);
    pointer-events: none;
  }
  .mk {
    padding: 1px 6px;
    border-radius: 99px;
    background: var(--surface-3);
    border: 1px solid var(--line-2);
    font-size: 11px;
    font-weight: 600;
    color: var(--text-2);
  }
  .mk.d {
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    padding: 0;
    background: var(--text);
    color: var(--bg);
    border: 0;
  }
  @keyframes glow {
    50% {
      box-shadow: 0 0 0 7px rgba(236, 235, 231, 0.05);
    }
  }
  @keyframes pop {
    from {
      transform: translate(-50%, -50%) scale(0.8);
    }
  }
</style>
