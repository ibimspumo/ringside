<script lang="ts">
  import type { LearnTarget } from "../app/Leaks.svelte";
  import NextSteps from "../next/NextSteps.svelte";
  import type { NextStep, StepHands } from "../next/types";
  import EmptyState from "../ui/EmptyState.svelte";
  import SectionHead from "../ui/SectionHead.svelte";
  import { preview } from "./preview";

  /**
   * Where the user really loses, from „Was als Nächstes?“ – the same list and order as the
   * overview and the weak-spot page. Renders nothing without entries unless `empty` asks for a hint.
   */
  let {
    steps,
    limit = 4,
    primary = false,
    empty = false,
    onLearn,
    onHands,
  }: {
    steps: NextStep[];
    limit?: number;
    /** The first row's practice button is the page's main action. */
    primary?: boolean;
    /** Show a hint while there is nothing to show yet. */
    empty?: boolean;
    onLearn: (target: LearnTarget) => void;
    onHands?: (hands: StepHands) => void;
  } = $props();

  let section = $state<HTMLElement>();
  $effect(() => {
    void steps.length;
    // After the page around it has settled, which the plan above changes once the list is in.
    if (section && preview("weak")) requestAnimationFrame(() => section?.scrollIntoView({ block: "start" }));
  });
</script>

{#if steps.length}
  <section class="weak" aria-label="Deine Schwachstellen" bind:this={section}>
    <SectionHead title="Deine Schwachstellen" sub="Aus deinen bewerteten Händen, deinen Werten und deinen Coach-Berichten. Was dich am meisten kostet, steht oben." />
    <NextSteps {steps} {limit} {primary} {onLearn} {onHands} />
  </section>
{:else if empty}
  <EmptyState
    compact
    title="Deine Schwachstellen erscheinen nach der Analyse"
    text="Sobald Ringside deine Hände bewertet hat (Seite „Hände“, „Hände analysieren“), steht hier, welche Spots du oft spielst und welche dich am meisten kosten."
  />
{/if}

<style>
  .weak {
    display: grid;
    gap: 10px;
  }
</style>
