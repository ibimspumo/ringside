<script lang="ts">
  import PlayingCard from "./PlayingCard.svelte";

  /**
   * A place for a card that you pick: dashed while empty with a short `label` ("Flop", "?"),
   * the card once chosen. With `onpick` it is a button that hands over itself, so a deck
   * popover can open next to it. `tone` "next" marks the place waiting for a card, "optional"
   * a place that may stay empty.
   */
  let {
    card = null,
    size = 42,
    label = "",
    tone = "idle",
    onpick,
    ariaLabel,
  }: {
    card?: string | null;
    size?: number;
    label?: string;
    tone?: "idle" | "next" | "optional";
    onpick?: (el: HTMLElement) => void;
    ariaLabel?: string;
  } = $props();
</script>

{#if onpick}
  <button type="button" class="ui-slot {tone}" class:filled={!!card} style:--w="{size}px" aria-label={ariaLabel} onclick={(e) => onpick(e.currentTarget)}>
    {#if card}<PlayingCard {card} {size} />{:else}<span class="lbl" class:glyph={label.length === 1}>{label}</span>{/if}
  </button>
{:else}
  <span class="ui-slot {tone} static" class:filled={!!card} style:--w="{size}px">
    {#if card}<PlayingCard {card} {size} />{/if}
  </span>
{/if}

<style>
  .ui-slot {
    display: grid;
    place-items: center;
    width: var(--w);
    height: calc(var(--w) * 1.4);
    padding: 0;
    border: 0;
    border-radius: max(3px, calc(var(--w) * 0.12));
    background: none;
    color: inherit;
    font: inherit;
    line-height: 0;
    flex-shrink: 0;
  }
  button.ui-slot {
    cursor: pointer;
    transition:
      transform var(--dur-fast) var(--ease-standard),
      border-color var(--dur-fast) var(--ease-standard),
      color var(--dur-fast) var(--ease-standard);
  }
  .ui-slot:not(.filled) {
    border: 1.5px dashed color-mix(in srgb, var(--text) 40%, transparent);
    background: color-mix(in srgb, var(--bg) 30%, transparent);
    color: color-mix(in srgb, var(--text) 70%, transparent);
  }
  .lbl {
    font-size: calc(var(--w) * 0.26);
    font-weight: var(--fw-semibold);
    line-height: 1;
  }
  .lbl.glyph {
    font-size: calc(var(--w) * 0.5);
  }
  .ui-slot.optional:not(.filled) {
    border-color: color-mix(in srgb, var(--text) 18%, transparent);
    color: color-mix(in srgb, var(--text) 40%, transparent);
  }
  .ui-slot.static:not(.filled) {
    border-color: color-mix(in srgb, var(--text) 8%, transparent);
    background: color-mix(in srgb, var(--bg) 15%, transparent);
  }
  .ui-slot.next:not(.filled) {
    border-style: solid;
    border-color: var(--text);
    background: color-mix(in srgb, var(--text) 8%, transparent);
    color: var(--text);
  }
  button.ui-slot:not(.filled):hover {
    border-color: var(--text);
    color: var(--text);
  }
  button.ui-slot.filled:hover {
    transform: translateY(-2px);
  }
  button.ui-slot:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: 2px;
  }
  @media (prefers-reduced-motion: reduce) {
    button.ui-slot {
      transition: none;
    }
  }
</style>
