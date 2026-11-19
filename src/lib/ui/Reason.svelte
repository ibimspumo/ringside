<script lang="ts">
  import CardText from "./CardText.svelte";
  import Term from "./Term.svelte";
  import { inlineTerms } from "./words";

  /**
   * "Warum?": why the best move is best, in the numbers its source measured. `collapsed` keeps it
   * one click away where the verdict already says what to do and the table needs the room.
   */
  let { text, label = "Warum?", collapsed = false }: { text: string; label?: string; collapsed?: boolean } = $props();
</script>

{#snippet body()}{#each inlineTerms(text) as p, i (i)}{#if p.id}<Term id={p.id}>{p.text}</Term>{:else}<CardText text={p.text} />{/if}{/each}{/snippet}

{#if collapsed}
  <details class="ui-reason">
    <summary class="label">{label}</summary>
    <p>{@render body()}</p>
  </details>
{:else}
  <div class="ui-reason">
    <span class="label">{label}</span>
    <p>{@render body()}</p>
  </div>
{/if}

<style>
  .ui-reason {
    display: grid;
    gap: 2px;
    padding: var(--sp-2) var(--sp-3);
    border-left: 2px solid var(--line-2);
    border-radius: 0 var(--r-sm) var(--r-sm) 0;
    background: color-mix(in srgb, var(--surface-2) 60%, transparent);
  }
  .label {
    color: var(--text-2);
    font-size: var(--fs-sm);
    font-weight: var(--fw-semibold);
  }
  summary {
    cursor: pointer;
  }
  summary:hover {
    color: var(--text);
  }
  details[open] summary {
    margin-bottom: 2px;
  }
  p {
    margin: 0;
    color: var(--text);
    font-size: var(--fs-md);
    line-height: var(--lh-base);
  }
</style>
