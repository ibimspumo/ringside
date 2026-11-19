<script lang="ts">
  import Button from "../ui/Button.svelte";
  import EmptyState from "../ui/EmptyState.svelte";
  import { go } from "../nav/go";
  import type { Page } from "../nav/pages";
  import { int } from "./format";

  /**
   * Stands in for ratings, recommendations and locked pages while PokerStars runs: one look for
   * every locked place. As a page it says why and links to what still works.
   */
  let { hands = null, compact = false }: { hands?: number | null; compact?: boolean } = $props();

  const OPEN: { page: Page; label: string }[] = [
    { page: "live", label: "Live-Tische" },
    { page: "overview", label: "Übersicht" },
    { page: "tournaments", label: "Turniere" },
  ];
  const kept = $derived(
    hands ? `${int(hands)} ${hands === 1 ? "Hand" : "Hände"} gespeichert – Ergebnisse, Statistiken und HUD laufen weiter.` : "Ergebnisse, Statistiken und HUD laufen weiter.",
  );
</script>

{#snippet lock()}<rect x="5" y="10.5" width="14" height="9.5" rx="2" /><path d="M8.5 10.5V8a3.5 3.5 0 0 1 7 0v2.5" />{/snippet}

{#if compact}
  <EmptyState compact title="Am Tisch – Auswertung, sobald du PokerStars schließt" text={kept} icon={lock} />
{:else}
  <EmptyState page title="Wieder da, sobald du PokerStars schließt" text="Diese Seite ist gesperrt, solange PokerStars läuft – so verlangen es die PokerStars-Regeln für Hilfsprogramme. {kept}" icon={lock}>
    {#snippet action()}
      <div class="open">
        <span>Solange du spielst:</span>
        {#each OPEN as o, i (o.page)}<Button variant={i === 0 ? "primary" : "secondary"} size="sm" onclick={() => go(o.page)}>{o.label}</Button>{/each}
      </div>
    {/snippet}
  </EmptyState>
{/if}

<style>
  .open {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: center;
    gap: var(--sp-2);
    color: var(--text-3);
    font-size: var(--fs-sm);
  }
  .open span {
    margin-right: var(--sp-1);
  }
</style>
