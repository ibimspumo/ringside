<script lang="ts">
  import { int } from "../ui/format";

  /**
   * Open tables and stored hands at the foot of the side bar, in one line: is Ringside watching,
   * and how much does it know. `rail` leaves the dot and the number of tables; the tooltip says all.
   */
  let { tables, hands, rail = false }: { tables: number; hands: number; rail?: boolean } = $props();

  const watching = $derived(tables === 0 ? "Kein Tisch offen" : `${tables === 1 ? "1 Tisch" : `${tables} Tische`} im Blick`);
</script>

<div class="meta" class:rail title="{watching} · {int(hands)} Hände gespeichert">
  <span class="live" class:active={tables > 0}>
    <span class="dot"></span>
    <span>{rail ? tables : tables === 0 ? "Kein Tisch offen" : tables === 1 ? "1 Tisch" : `${tables} Tische`}</span>
  </span>
  {#if !rail}<span class="stored"><b class="num">{int(hands)}</b> Hände</span>{/if}
</div>

<style>
  .meta {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-2);
    padding: 0 var(--sp-2) 0 var(--sp-3);
    font-size: var(--fs-sm);
    color: var(--text-2);
    white-space: nowrap;
  }
  .rail {
    justify-content: center;
    padding: 0;
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
</style>
