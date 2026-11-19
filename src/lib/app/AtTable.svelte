<script lang="ts">
  import EmptyState from "../ui/EmptyState.svelte";
  import { int } from "./format";

  /** Stands in for ratings and recommendations while PokerStars runs. */
  let { hands = null, compact = false }: { hands?: number | null; compact?: boolean } = $props();
</script>

<EmptyState
  {compact}
  title="Am Tisch – Auswertung, sobald du den Poker-Client schließt"
  text={hands
    ? `${int(hands)} ${hands === 1 ? "Hand" : "Hände"} gespeichert. Ergebnisse, Statistiken und HUD laufen weiter.`
    : "Ergebnisse, Statistiken und HUD laufen weiter."}
>
  {#snippet icon()}<rect x="5" y="10.5" width="14" height="9.5" rx="2" /><path d="M8.5 10.5V8a3.5 3.5 0 0 1 7 0v2.5" />{/snippet}
</EmptyState>
