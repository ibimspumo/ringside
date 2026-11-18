<script lang="ts">
  import GradeIcon from "./GradeIcon.svelte";
  import { GRADE, GRADES, type Grade } from "./poker";

  /**
   * How a session went, grade by grade: icon, word, count and a bar of its share. Grades without
   * decisions stay visible but faded, so the scale is always complete.
   */
  let { counts, label = "Bewertungen" }: { counts: Partial<Record<Grade, number>>; label?: string } = $props();

  const total = $derived(GRADES.reduce((s, g) => s + (counts[g] ?? 0), 0));
</script>

<ul class="ui-grade-bars" aria-label={label}>
  {#each GRADES as g (g)}
    {@const n = counts[g] ?? 0}
    <li class:none={n === 0} style:--c="var(--grade-{g})">
      <GradeIcon grade={g} size={15} />
      <span class="word">{GRADE[g].label}</span>
      <b class="num">{n}</b>
      <span class="track"><i style:width="{total ? (n / total) * 100 : 0}%"></i></span>
    </li>
  {/each}
</ul>

<style>
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
