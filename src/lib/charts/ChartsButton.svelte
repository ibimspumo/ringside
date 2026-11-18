<script lang="ts">
  import { onMount } from "svelte";
  import { api, on } from "../api";
  import type { Decision } from "../analysis/types";
  import { openInCharts, type ChartTarget } from "./handoff.svelte";
  import Button from "../ui/Button.svelte";

  /** "Range ansehen" for a rated preflop decision: opens its spot on the ranges page with the hand. */
  let { decision: d, variant = "secondary" }: { decision: Decision; variant?: "primary" | "secondary" } = $props();

  let target = $state<ChartTarget | null>(null);
  let locked = $state(false);

  $effect(() => {
    if (!chartable(d)) {
      target = null;
      return;
    }
    let alive = true;
    api
      .analysisChartTarget(d.hand_id, d.action_index)
      .then((t) => alive && (target = t))
      .catch(() => alive && (target = null));
    return () => (alive = false);
  });

  onMount(() => {
    const sub = on<boolean>("poker-client", (r) => (locked = r));
    api.pokerClientRunning().then((r) => (locked = r));
    return () => sub.then((f) => f());
  });
</script>

<script lang="ts" module>
  /** Preflop decisions rated against a bundled chart; each has a chart target (tested in the backend). */
  export function chartable(d: Decision): boolean {
    return d.street === "preflop" && d.status === "rated" && (d.source === "equilibrium" || d.source === "model");
  }
</script>

{#if target}
  <Button
    {variant}
    disabled={locked}
    title={locked
      ? "Die Ranges sind gesperrt, solange PokerStars läuft – so verlangen es die PokerStars-Regeln."
      : "Öffnet „Ranges nach Stack“ mit genau diesem Spot und zeigt, wo deine Hand liegt."}
    onclick={() => target && openInCharts(target, d.hand_id, { played: d.played, best: d.best, loss: d.ev_loss_bb })}
  >
    Range ansehen
  </Button>
  {#if locked}<span class="locked">gesperrt, solange PokerStars läuft</span>{/if}
{/if}

<style>
  .locked {
    align-self: center;
    font-size: var(--fs-sm);
    color: var(--text-3);
  }
</style>
