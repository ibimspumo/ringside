<script lang="ts">
  import { int } from "../ui/format";

  /**
   * Open tables and stored hands in the top bar: is Ringside watching, and how much does it know.
   * `words` adds "im Blick" and "gespeichert", `short` leaves the dot and the number of tables. The
   * hand count stays in the page even when hidden.
   */
  let {
    tables,
    hands,
    words,
    showHands,
    short = false,
  }: { tables: number; hands: number; words: boolean; showHands: boolean; short?: boolean } = $props();
</script>

<span class="meta">
  <span class="live" class:active={tables > 0} title={short ? (tables === 0 ? "Kein Tisch offen" : `${tables} ${tables === 1 ? "Tisch" : "Tische"} im Blick`) : undefined}>
    <span class="dot"></span>
    <span
      >{#if short}{tables}{:else if tables === 0}Kein Tisch offen{:else}{tables === 1 ? "1 Tisch" : `${tables} Tische`}{#if words}&nbsp;im Blick{/if}{/if}</span
    >
  </span>
  <span class="stored" class:gone={!showHands}><b class="num">{int(hands)}</b> Hände{#if words}&nbsp;gespeichert{/if}</span>
</span>

<style>
  .meta {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
    font-size: var(--fs-sm);
    color: var(--text-2);
    white-space: nowrap;
  }
  .live {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    border: 1.5px solid var(--text-3);
  }
  .active {
    color: var(--text);
  }
  .active .dot {
    background: var(--pos);
    border-color: var(--pos);
  }
  b {
    font-family: var(--font-num);
    font-weight: var(--fw-regular);
    color: var(--text);
  }
  .gone {
    display: none;
  }
</style>
