<script lang="ts">
  import CardText from "../ui/CardText.svelte";
  import SourceNote from "../ui/SourceNote.svelte";
  import { ownCards, say, termsIn } from "./meaning";
  import type { Figure } from "./types";

  /** A spot's figures as rows (label, value, source), with the poker terms they use explained. */
  let { numbers, hero }: { numbers: Figure[]; hero?: string[] } = $props();

  const terms = $derived(termsIn(numbers.map((n) => n.label)));
</script>

<div class="wrap">
  <div class="inner">
  <dl class="figures">
    {#each numbers as n, i (i)}
      {@const calc = n.source === "berechnet"}
      <div class="fig" class:guess={!calc}>
        <dt><CardText hands text={say(hero ? ownCards(n.label, hero) : n.label)} /></dt>
        <dd>
          <b class:num={calc} class:big={calc}>{say(n.value)}</b>
          <span class="src"><SourceNote kind={calc ? "computed" : "opinion"} /></span>
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
</div>

<style>
  .wrap {
    container-type: inline-size;
  }
  .inner {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }
  /* Wide cards: the terms stand beside the figures instead of below. */
  @container (min-width: 900px) {
    .inner {
      display: grid;
      grid-template-columns: minmax(0, 1.4fr) minmax(0, 1fr);
      align-items: start;
      gap: var(--sp-5);
    }
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
    gap: 6px var(--sp-4);
    min-height: 40px;
    padding: 7px var(--sp-3);
    border-top: 1px solid var(--line);
  }
  .fig:first-child {
    border-top: 0;
  }
  dt {
    min-width: 0;
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  dd {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    margin: 0;
    flex-shrink: 0;
    max-width: 60%;
    text-align: right;
  }
  dd b {
    font-weight: var(--fw-semibold);
    font-size: var(--fs-base);
  }
  dd b.big {
    font-size: var(--fs-xl);
  }
  .src {
    display: flex;
    justify-content: flex-end;
    width: 104px;
    flex-shrink: 0;
  }

  /* Words read from the left under their label; only numbers sit on the right. */
  .guess {
    flex-direction: column;
    align-items: stretch;
    gap: var(--sp-1);
  }
  .guess dd {
    max-width: none;
    justify-content: space-between;
    text-align: left;
  }
  @container (max-width: 460px) {
    .fig {
      flex-direction: column;
      align-items: stretch;
    }
    dd {
      max-width: none;
      justify-content: space-between;
      text-align: left;
    }
  }

  .terms {
    list-style: none;
    display: grid;
    grid-template-columns: max-content minmax(0, 1fr);
    gap: 2px var(--sp-4);
    margin: 0;
    padding: 0;
    font-size: var(--fs-md);
    line-height: 1.5;
    color: var(--text-2);
  }
  .terms li {
    display: contents;
  }
  .terms b {
    font-weight: var(--fw-semibold);
    color: var(--text);
  }
</style>
