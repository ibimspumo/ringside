<script lang="ts">
  import { onMount } from "svelte";
  import { floating, type Anchor } from "../app/format";
  import PlayingCard from "../PlayingCard.svelte";
  import { RANKS } from "./range";

  /** One card from the deck, next to the slot that asked for it. */
  let {
    anchor,
    title,
    available,
    current = null,
    onPick,
    onRandom,
    onClear,
    onClose,
    side = "bottom",
  }: {
    anchor: Anchor;
    /** Where it opens; "top" keeps a seat at the bottom of the table in view. */
    side?: "top" | "bottom";
    title: string;
    /** Cards that can be picked; the rest is shown dimmed. */
    available: string[];
    current?: string | null;
    onPick: (card: string) => void;
    onRandom?: () => void;
    onClear?: () => void;
    onClose: () => void;
  } = $props();

  const SUITS = ["s", "h", "d", "c"];
  const open = $derived(new Set(available));
  let el = $state<HTMLElement>();

  onMount(() => {
    el?.querySelector<HTMLElement>("button.pick:not(:disabled)")?.focus();
  });

  function key(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.stopPropagation();
      onClose();
    }
  }

  function outside(e: PointerEvent) {
    if (el && !el.contains(e.target as Node)) onClose();
  }
</script>

<svelte:window onpointerdown={outside} />

<!-- Dims what lies behind, so seats half under the popover do not read as part of it. -->
<div class="scrim" aria-hidden="true"></div>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div class="popover deck-pop" bind:this={el} use:floating={{ anchor, side }} role="dialog" aria-label={title} tabindex="-1" onkeydown={key}>
  <header>
    <b>{title}</b>
    <span class="tools">
      {#if onRandom}<button class="btn ghost small" onclick={onRandom}>Zufällig</button>{/if}
      {#if onClear && current}<button class="btn ghost small" onclick={onClear}>Entfernen</button>{/if}
    </span>
  </header>
  <div class="grid">
    {#each SUITS as s (s)}
      {#each RANKS.split("") as r (r)}
        {@const c = r + s}
        <button class="pick" class:on={c === current} disabled={!open.has(c)} onclick={() => onPick(c)} aria-label={c}>
          <PlayingCard card={c} size={23} />
        </button>
      {/each}
    {/each}
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 49;
    background: color-mix(in srgb, var(--bg) 45%, transparent);
    animation: fade var(--dur) var(--ease-enter);
  }
  @keyframes fade {
    from {
      opacity: 0;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .scrim {
      animation: none;
    }
  }
  .deck-pop {
    z-index: 50;
    padding: 10px;
    border-radius: var(--r);
    background: var(--surface);
    border: 1px solid var(--line-2);
    box-shadow: 0 14px 40px rgba(0, 0, 0, 0.55);
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 8px;
    font-size: 12.5px;
  }
  .tools {
    display: flex;
    gap: 4px;
  }
  .small {
    height: 24px;
    padding: 0 8px;
    font-size: 12px;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(13, max-content);
    gap: 3px;
  }
  .pick {
    padding: 1px;
    border: 0;
    border-radius: 4px;
    background: none;
    line-height: 0;
    cursor: pointer;
  }
  .pick:hover:not(:disabled) {
    transform: translateY(-2px);
  }
  .pick.on {
    outline: 2px solid var(--text);
  }
  .pick:disabled {
    opacity: 0.15;
    cursor: default;
  }
</style>
