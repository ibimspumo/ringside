<script lang="ts">
  import { termsIn } from "./meaning";
  import type { Figure } from "./types";

  /** A spot's figures, each marked computed or estimated, with the poker terms they use explained. */
  let { numbers }: { numbers: Figure[] } = $props();

  const terms = $derived(termsIn(numbers.map((n) => n.label)));
</script>

<div class="wrap">
  <dl class="figures">
    {#each numbers as n, i (i)}
      <div class="fig" class:guess={n.source !== "berechnet"}>
        <dt>{n.label}</dt>
        <dd>
          <b class:num={n.source === "berechnet"}>{n.value}</b>
          <span
            class="src"
            title={n.source === "berechnet"
              ? "Von Ringsides Rechenkern berechnet, nicht vom Sprachmodell"
              : "Einschätzung des Sprachmodells, nicht nachgerechnet"}>{n.source === "berechnet" ? "berechnet" : "Einschätzung"}</span
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
    gap: 8px;
  }
  .figures {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(160px, 1fr));
    gap: 6px;
    margin: 0;
  }
  .fig {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 7px 10px 8px;
    border-radius: var(--r-sm);
    background: var(--bg);
    border: 1px solid transparent;
  }
  .fig.guess {
    background: transparent;
    border: 1px dashed var(--line-2);
  }
  dt {
    font-size: 12px;
    color: var(--text-3);
  }
  dd {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 8px;
    margin: 0;
  }
  dd b {
    font-weight: 600;
    font-size: 13.5px;
    min-width: 0;
  }
  dd b.num {
    font-size: 17px;
  }
  .src {
    flex-shrink: 0;
    padding: 1px 6px;
    border-radius: 4px;
    font-size: 11px;
    background: var(--surface-3);
    color: var(--text);
  }
  .guess .src {
    background: none;
    padding: 0;
    color: var(--text-3);
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
