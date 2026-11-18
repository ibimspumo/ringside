<script lang="ts">
  import type { Mark } from "./labels";

  /** Small round symbol for where a spot solved here stands in the cloud. */
  let { mark, label = null }: { mark: Mark; label?: string | null } = $props();
</script>

<span class="mark {mark}" role={label ? "img" : undefined} aria-label={label ?? undefined} aria-hidden={label ? undefined : "true"}>
  <svg viewBox="0 0 12 12" width="12" height="12">
    {#if mark === "only_here"}
      <circle cx="6" cy="6" r="3.2" fill="none" stroke="currentColor" stroke-width="1.5" />
    {:else if mark === "submitted"}
      <circle cx="6" cy="6" r="4.4" fill="none" stroke="currentColor" stroke-width="1.4" />
      <path d="M6 3.6V6l1.7 1.1" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" />
    {:else if mark === "verified"}
      <circle cx="6" cy="6" r="5.2" fill="currentColor" />
      <path d="M3.6 6.1l1.6 1.6 3.2-3.3" fill="none" stroke="var(--bg)" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
    {:else if mark === "rejected"}
      <circle cx="6" cy="6" r="5.2" fill="currentColor" />
      <path d="M4.2 4.2l3.6 3.6M7.8 4.2L4.2 7.8" stroke="var(--bg)" stroke-width="1.5" stroke-linecap="round" />
    {:else}
      <path d="M9.3 4.6A3.6 3.6 0 1 0 9.5 7" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" />
      <path d="M9.6 2.4v2.5H7.1" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round" />
    {/if}
  </svg>
</span>

<style>
  .mark {
    display: inline-grid;
    place-items: center;
    width: 14px;
    height: 14px;
    flex: none;
    line-height: 0;
  }
  .only_here {
    color: var(--text-3);
  }
  .submitted {
    color: var(--amber);
  }
  .verified {
    color: var(--pos);
  }
  .rejected {
    color: var(--neg);
  }
  .failed {
    color: var(--text-2);
  }
</style>
