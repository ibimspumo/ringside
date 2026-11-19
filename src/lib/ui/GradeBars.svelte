<script lang="ts">
  import GradeIcon from "./GradeIcon.svelte";
  import { int } from "./format";
  import { GRADE, GRADES, type Grade } from "./poker";

  /**
   * How a session went, grade by grade: icon, word, count and a bar of its share. Grades without
   * decisions stay visible but faded, so the scale is always complete. `grades` narrows the scale
   * for sources that never give a grade (the hand analysis has no "mix"). `stacked` puts all
   * grades in one bar with a wrapping key of the grades that occur, for a summary tile.
   */
  let {
    counts,
    label = "Bewertungen",
    grades = GRADES,
    stacked = false,
  }: {
    counts: Partial<Record<Grade, number>>;
    label?: string;
    /** The scale shown; a source that only grades in some steps (the coach) passes those. */
    grades?: Grade[];
    stacked?: boolean;
  } = $props();

  const total = $derived(grades.reduce((s, g) => s + (counts[g] ?? 0), 0));
</script>

{#if stacked}
  <div class="ui-grade-stack" role="group" aria-label={label}>
    <span class="stack" aria-hidden="true">
      {#each grades as g (g)}
        {#if counts[g]}<i style:flex-grow={counts[g]} style:background="var(--grade-{g})"></i>{/if}
      {/each}
    </span>
    <ul class="keys">
      {#each grades as g (g)}
        {#if counts[g]}
          <li><GradeIcon grade={g} size={14} /><span>{GRADE[g].label}</span><b class="num">{int(counts[g] ?? 0)}</b></li>
        {/if}
      {/each}
    </ul>
  </div>
{:else}
<ul class="ui-grade-bars" aria-label={label}>
  {#each grades as g (g)}
    {@const n = counts[g] ?? 0}
    <li class:none={n === 0} style:--c="var(--grade-{g})">
      <GradeIcon grade={g} size={15} />
      <span class="word">{GRADE[g].label}</span>
      <b class="num">{int(n)}</b>
      <span class="track"><i style:width="{total ? (n / total) * 100 : 0}%"></i></span>
    </li>
  {/each}
</ul>
{/if}

<style>
  .ui-grade-stack {
    display: grid;
    gap: var(--sp-3);
  }
  .stack {
    display: flex;
    gap: 2px;
    height: 10px;
    border-radius: var(--r-pill);
    overflow: hidden;
    background: var(--surface-3);
  }
  .stack i {
    min-width: 3px;
  }
  .keys {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-2) var(--sp-4);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .keys li {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-md);
  }
  .keys b {
    font-weight: var(--fw-semibold);
  }
  .ui-grade-bars {
    display: grid;
    grid-template-columns: auto 1fr auto minmax(60px, 1.2fr);
    align-items: center;
    gap: 6px var(--sp-2);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  li {
    display: contents;
  }
  .word {
    font-size: var(--fs-md);
  }
  b {
    font-size: var(--fs-md);
    font-weight: var(--fw-semibold);
    text-align: right;
  }
  .none .word,
  .none b {
    color: var(--text-3);
  }
  .none :global(svg) {
    opacity: 0.4;
  }
  .track {
    height: 6px;
    border-radius: var(--r-pill);
    background: var(--surface-3);
    overflow: hidden;
  }
  i {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: var(--c);
    transition: width var(--dur-slow) var(--ease-standard);
  }
</style>
