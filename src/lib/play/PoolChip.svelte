<script lang="ts">
  import Button from "../ui/Button.svelte";
  import { etaText } from "./labels";
  import type { PoolStatus } from "./types";

  /** The stock of postflop spots: how many are ready, what is being prepared, and a stop. */
  /** `compact`: at the table only the meter and the stop; the words are in the tooltip. */
  let { pool, compact = false, onCancel, onResume }: { pool: PoolStatus | null; compact?: boolean; onCancel: () => void; onResume: () => void } = $props();

  const text = $derived.by(() => {
    if (!pool) return "";
    switch (pool.status) {
      case "running":
        return `${pool.title} ${etaText(pool.eta_secs)}`.trim();
      case "paused_client":
        return "Pausiert, solange PokerStars läuft";
      case "done":
        return `${pool.ready} Spots bereit`;
      case "cancelled":
        return "Vorbereitung abgebrochen";
      case "error":
        return pool.error ?? "Vorbereitung gestoppt";
      default:
        return "Wartet auf die erste Hand";
    }
  });
</script>

{#if pool}
  <div class="pool {pool.status}" class:compact role="status" title="{compact ? `${text}. ` : ''}Postflop-Spots werden im Hintergrund gerechnet, damit du nie warten musst. Das läuft weiter, auch wenn du die Seite wechselst.">
    <span class="lbl">{compact ? "Spots" : "Vorbereitete Flop-Spots"}</span>
    <span class="meter" aria-hidden="true">
      {#each Array.from({ length: pool.target }, (_, i) => i) as i (i)}
        <i class:full={i < pool.ready} class:fill={pool.status === "running" && i === pool.ready} style:--f={pool.status === "running" && i === pool.ready ? (pool.fraction * pool.target - pool.ready) : 0}></i>
      {/each}
    </span>
    {#if !compact}<span class="txt">{text}</span>{/if}
    {#if pool.status === "running" || pool.status === "paused_client"}
      <Button variant="quiet" size="sm" onclick={onCancel}>Anhalten</Button>
    {:else if pool.status === "cancelled" || pool.status === "error"}
      <Button variant="quiet" size="sm" onclick={onResume}>Weiter vorbereiten</Button>
    {/if}
  </div>
{/if}

<style>
  .pool {
    display: inline-flex;
    align-items: center;
    gap: 9px;
    font-size: var(--fs-sm);
    color: var(--text-2);
    padding: 2px 4px 2px 12px;
    min-height: 32px;
    border-radius: 999px;
    background: var(--surface);
    border: 1px solid var(--line);
  }
  .lbl {
    color: var(--text-3);
  }
  .meter {
    display: inline-flex;
    gap: 3px;
  }
  .meter i {
    position: relative;
    width: 16px;
    height: 6px;
    border-radius: 3px;
    background: var(--surface-3);
    overflow: hidden;
  }
  .meter i.full {
    background: var(--pos);
  }
  .meter i.fill::after {
    content: "";
    position: absolute;
    inset: 0;
    width: calc(var(--f) * 100%);
    background: color-mix(in srgb, var(--pos) 70%, transparent);
    transition: width var(--dur-slow) var(--ease-standard);
  }
  .error .txt {
    color: var(--neg);
  }
  @media (prefers-reduced-motion: reduce) {
    .meter i.fill::after {
      transition: none;
    }
  }
</style>
