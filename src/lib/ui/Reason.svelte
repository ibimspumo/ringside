<script lang="ts">
  import CardText from "./CardText.svelte";

  /** "Warum?": why the best move is best, in the numbers its source measured. */
  let { text, label = "Warum?" }: { text: string; label?: string } = $props();
</script>

<div class="ui-reason">
  <span class="label">{label}</span>
  <p><CardText {text} /></p>
</div>

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
  p {
    margin: 0;
    color: var(--text);
    font-size: var(--fs-md);
    line-height: var(--lh-base);
  }
</style>
