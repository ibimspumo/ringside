<script lang="ts">
  import { termsIn } from "./meaning";
  import type { Figure } from "./types";

  /** A spot's figures as rows (label, value, source), with the poker terms they use explained. */
  let { numbers }: { numbers: Figure[] } = $props();

  const terms = $derived(termsIn(numbers.map((n) => n.label)));
</script>

<div class="wrap">
  <dl class="figures">
    {#each numbers as n, i (i)}
      {@const calc = n.source === "berechnet"}
      <div class="fig" class:guess={!calc}>
        <dt>{n.label}</dt>
        <dd>
          <b class:num={calc}>{n.value}</b>
          <span
            class="src"
            title={calc ? "Von Ringsides Rechenkern berechnet, nicht vom Sprachmodell" : "Einschätzung des Sprachmodells, nicht nachgerechnet"}
            >{calc ? "berechnet" : "Einschätzung"}</span
          >
        </dd>
      </div>
    {/each}
  </dl>
  {#if terms.length}
    <ul class="terms" aria-label="Begriffe">
      {#each terms as t (t.term)}<li><b>{t.term}</b><span>{t.meaning}</span></li>{/each}
    </ul>
  {/if}
</div>

<style>
  .wrap {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .figures {
    display: flex;
    flex-direction: column;
    margin: 0;
    border-radius: var(--r-sm);
    background: var(--bg);
    border: 1px solid var(--line);
  }
  .fig {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 6px 16px;
    min-height: 38px;
    padding: 7px 12px;
    border-top: 1px solid var(--line);
  }
  .fig:first-child {
    border-top: 0;
  }
  dt {
    min-width: 0;
    font-size: 13px;
    color: var(--text-2);
  }
  dd {
    display: flex;
    align-items: baseline;
    gap: 10px;
    margin: 0;
    flex-shrink: 0;
    max-width: 60%;
    text-align: right;
  }
  dd b {
    font-weight: 600;
    font-size: 13.5px;
  }
  dd b.num {
    font-size: 17px;
  }
  .src {
    flex-shrink: 0;
    width: 86px;
    padding: 1px 0;
    border-radius: 4px;
    font-size: 11.5px;
    text-align: center;
    background: var(--surface-3);
    color: var(--text);
    cursor: help;
  }
  .guess .src {
    background: none;
    border: 1px dashed var(--line-2);
    color: var(--text-2);
  }

  .terms {
    list-style: none;
    display: grid;
    grid-template-columns: max-content minmax(0, 1fr);
    gap: 2px 14px;
    margin: 0;
    padding: 0;
    font-size: 12.5px;
    line-height: 1.5;
    color: var(--text-2);
  }
  .terms li {
    display: contents;
  }
  .terms b {
    font-weight: 600;
    color: var(--text);
  }
</style>
