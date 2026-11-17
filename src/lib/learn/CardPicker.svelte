<script lang="ts">
  import PlayingCard from "../PlayingCard.svelte";
  import type { Highlight } from "../types";

  /** The deck as four suit rows; only cards in `from` can be picked. After grading, `highlight` marks them. */
  let {
    from,
    picked = $bindable([]),
    highlight = null,
  }: { from: string[]; picked?: string[]; highlight?: Highlight[] | null } = $props();

  const RANKS = "AKQJT98765432";
  const SUITS = ["s", "h", "d", "c"];
  const SUIT_NAME: Record<string, string> = { s: "Pik", h: "Herz", d: "Karo", c: "Kreuz" };
  const open = $derived(new Set(from));
  const marks = $derived(new Map((highlight ?? []).map((h) => [h.card, h.tag])));
  const done = $derived(highlight !== null);

  let width = $state(600);
  const size = $derived(Math.max(20, Math.min(42, Math.floor((width - 12 * 5) / 13))));

  function toggle(c: string) {
    if (done || !open.has(c)) return;
    picked = picked.includes(c) ? picked.filter((x) => x !== c) : [...picked, c];
  }

  /** Arrow keys move between the cards like in a grid. */
  function nav(e: KeyboardEvent) {
    const dirs: Record<string, [number, number]> = { ArrowLeft: [0, -1], ArrowRight: [0, 1], ArrowUp: [-1, 0], ArrowDown: [1, 0] };
    const d = dirs[e.key];
    const el = e.target as HTMLElement;
    if (!d || !el.dataset.card) return;
    e.preventDefault();
    e.stopPropagation();
    let r = SUITS.indexOf(el.dataset.card[1]);
    let c = RANKS.indexOf(el.dataset.card[0]);
    for (let i = 0; i < 13; i++) {
      r = (r + d[0] + 4) % 4;
      c = (c + d[1] + 13) % 13;
      const next = el.closest(".deck")?.querySelector<HTMLElement>(`[data-card="${RANKS[c]}${SUITS[r]}"]:not(:disabled)`);
      if (next) {
        next.focus();
        return;
      }
    }
  }

  const LEGEND: { tag: Highlight["tag"]; label: string }[] = [
    { tag: "out", label: "Out" },
    { tag: "missed", label: "Übersehen" },
    { tag: "half_out", label: "Halbes Out" },
    { tag: "wrong", label: "Kein Out" },
    { tag: "dirty", label: "Schmutzig: verbessert, gewinnt aber nicht" },
  ];
  const counts = $derived(LEGEND.map((l) => ({ ...l, n: (highlight ?? []).filter((h) => h.tag === l.tag).length })).filter((l) => l.n));
</script>

<div class="picker">
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div class="deck" bind:clientWidth={width} role="group" aria-label="Kartendeck: Outs antippen" onkeydown={nav}>
    {#each SUITS as s (s)}
      <div class="suit" role="presentation">
        {#each RANKS as r (r)}
          {@const c = r + s}
          {@const tag = marks.get(c)}
          <button
            class="slot {tag ?? ''}"
            class:sel={picked.includes(c)}
            class:gone={!open.has(c)}
            disabled={!open.has(c) || (done && !tag)}
            data-card={c}
            aria-pressed={picked.includes(c)}
            aria-label="{r === 'T' ? '10' : r} {SUIT_NAME[s]}{tag ? `, ${LEGEND.find((l) => l.tag === tag)?.label}` : ''}"
            onclick={() => toggle(c)}
          >
            <PlayingCard card={c} {size} />
          </button>
        {/each}
      </div>
    {/each}
  </div>
  <div class="foot">
    {#if done}
      <div class="legend">
        {#each counts as l (l.tag)}
          <span class="key {l.tag}"><i></i>{l.label} <b class="num">{l.n}</b></span>
        {/each}
      </div>
    {:else}
      <span class="dim"><b class="num">{picked.length}</b> {picked.length === 1 ? "Karte" : "Karten"} gewählt</span>
      {#if picked.length}<button class="btn ghost" onclick={() => (picked = [])}>Auswahl leeren</button>{/if}
    {/if}
  </div>
</div>

<style>
  .picker {
    display: grid;
    gap: 10px;
    min-width: 0;
  }
  .deck {
    display: grid;
    gap: 6px;
    min-width: 0;
  }
  .suit {
    display: grid;
    grid-template-columns: repeat(13, minmax(0, 1fr));
    gap: 5px;
    justify-items: center;
  }
  .slot {
    position: relative;
    display: inline-flex;
    padding: 0;
    border: 0;
    border-radius: 5px;
    background: none;
    cursor: pointer;
    opacity: 0.55;
    transition:
      transform 0.14s,
      opacity 0.14s,
      box-shadow 0.14s;
  }
  .slot:hover:not(:disabled) {
    opacity: 0.9;
    transform: translateY(-2px);
  }
  .slot.sel {
    opacity: 1;
    transform: translateY(-4px);
    box-shadow: 0 0 0 2px var(--text);
  }
  .slot.gone {
    visibility: hidden;
  }
  .slot:disabled {
    cursor: default;
  }
  .slot:disabled:not(.out, .missed, .half_out, .wrong, .dirty) {
    opacity: 0.18;
  }
  .slot.out,
  .slot.missed,
  .slot.half_out,
  .slot.wrong,
  .slot.dirty {
    opacity: 1;
    animation: pop 0.35s cubic-bezier(0.2, 0.9, 0.3, 1.4) both;
  }
  .out {
    box-shadow: 0 0 0 2.5px var(--pos);
  }
  .slot.missed {
    box-shadow: 0 0 0 2.5px var(--amber);
    animation-name: pulse;
  }
  .half_out {
    box-shadow:
      0 0 0 2.5px var(--amber),
      inset 0 -999px 0 rgba(242, 177, 46, 0.18);
  }
  .slot.wrong {
    box-shadow: 0 0 0 2.5px var(--neg);
    animation-name: shake;
  }
  .dirty {
    box-shadow: 0 0 0 2px var(--neg);
    filter: saturate(0.4) brightness(0.8);
  }
  .slot.sel.out,
  .slot.sel.wrong,
  .slot.sel.dirty {
    transform: translateY(-4px);
  }
  @keyframes pop {
    from {
      transform: scale(0.85);
    }
  }
  @keyframes pulse {
    40% {
      transform: scale(1.12);
    }
  }
  @keyframes shake {
    25% {
      transform: translateX(-3px);
    }
    75% {
      transform: translateX(3px);
    }
  }
  .foot {
    display: flex;
    align-items: center;
    gap: 12px;
    min-height: 32px;
    font-size: 13px;
  }
  .legend {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 16px;
    font-size: 12.5px;
    color: var(--text-2);
  }
  .key {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .key i {
    width: 11px;
    height: 11px;
    border-radius: 3px;
    border: 2px solid var(--c);
  }
  .key b {
    color: var(--text);
    font-weight: 600;
  }
  .key.out {
    --c: var(--pos);
  }
  .key.missed,
  .key.half_out {
    --c: var(--amber);
  }
  .key.wrong,
  .key.dirty {
    --c: var(--neg);
  }
</style>
