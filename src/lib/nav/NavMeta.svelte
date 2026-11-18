<script lang="ts">
  import { int } from "../ui/format";

  /** Open tables and stored hands: is Ringside watching, and how much does it know. */
  let { tables, hands, layout = "side" }: { tables: number; hands: number; layout?: "side" | "bar" } = $props();
</script>

<div class="meta {layout}">
  <span class="live" class:active={tables > 0}>
    <span class="dot"></span>
    <span>{#if tables === 0}Kein Tisch offen{:else}{tables === 1 ? "1 Tisch" : `${tables} Tische`}<span class="long">&nbsp;im Blick</span>{/if}</span>
  </span>
  <span class="stored"><b>{int(hands)}</b> Hände<span class="long">&nbsp;gespeichert</span></span>
</div>

<style>
  .meta {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .side {
    flex-direction: row;
    justify-content: space-between;
    gap: var(--sp-2);
    padding: 0 10px;
  }
  .side .long {
    display: none;
  }
  .bar {
    flex-direction: row;
    align-items: center;
    gap: var(--sp-4);
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
  /* The open tables already show on the Gegner tab; only very wide bars add the counts. */
  @media (max-width: 1799px) {
    .bar {
      display: none;
    }
  }
</style>
