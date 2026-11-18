<script lang="ts">
  /**
   * Keyboard shortcut. One key ("R", "Leertaste", "←") or a combination given as a list or as
   * "Strg + K". The keycap look and centring come from the global `kbd` rule in theme.css.
   */
  let { keys }: { keys: string | string[] } = $props();

  const parts = $derived(Array.isArray(keys) ? keys : keys.split(/\s+\+\s+/));
</script>

{#if parts.length === 1}
  <kbd>{parts[0]}</kbd>
{:else}
  <span class="combo" aria-label="Tastenkürzel {parts.join(' plus ')}">
    {#each parts as key, k (k)}
      {#if k > 0}<span class="plus" aria-hidden="true">+</span>{/if}<kbd>{key}</kbd>
    {/each}
  </span>
{/if}

<style>
  .combo {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    vertical-align: middle;
  }
  .plus {
    font-size: var(--fs-xs);
    opacity: 0.6;
  }
</style>
