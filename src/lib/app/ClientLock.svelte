<script lang="ts">
  import type { Snippet } from "svelte";
  import AtTable from "./AtTable.svelte";
  import { pokerClient, watchPokerClient } from "./client.svelte";

  /** Shows `children` only while no poker client runs (PokerStars or GGPoker). */
  let { children }: { children: Snippet } = $props();

  // Unknown until the backend answers; nothing is shown meanwhile, so locked content never flashes.
  watchPokerClient();
  const running = $derived(pokerClient.running);
</script>

{#if running === false}
  {@render children()}
{:else if running}
  <div class="page"><AtTable /></div>
{/if}
