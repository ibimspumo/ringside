<script lang="ts">
  import { etaText } from "./labels";
  import type { PoolStatus } from "./types";

  /** The stock of postflop spots: how many are ready, what is being prepared, and a stop. */
  let { pool, onCancel, onResume }: { pool: PoolStatus | null; onCancel: () => void; onResume: () => void } = $props();

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
  <div class="pool {pool.status}" role="status" title="Postflop-Spots werden im Hintergrund gerechnet, damit du nie warten musst. Das läuft weiter, auch wenn du die Seite wechselst.">
    <span class="lbl">Postflop-Vorrat</span>
    <span class="meter" aria-hidden="true">
      {#each Array.from({ length: pool.target }, (_, i) => i) as i (i)}
        <i class:full={i < pool.ready} class:fill={pool.status === "running" && i === pool.ready} style:--f={pool.status === "running" && i === pool.ready ? (pool.fraction * pool.target - pool.ready) : 0}></i>
      {/each}
    </span>
    <span class="txt">{text}</span>
    {#if pool.status === "running" || pool.status === "paused_client"}
      <button class="link" onclick={onCancel}>Abbrechen</button>
    {:else if pool.status === "cancelled" || pool.status === "error"}
      <button class="link" onclick={onResume}>Weiter vorbereiten</button>
    {/if}
  </div>
{/if}

<style>
  .pool {
    display: inline-flex;
    align-items: center;
    gap: 9px;
    font-size: 12px;
    color: var(--text-2);
    padding: 5px 10px;
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
    transition: width 0.4s;
  }
  .error .txt {
    color: var(--neg);
  }
  .link {
    border: 0;
    background: none;
    color: var(--text);
    text-decoration: underline;
    text-underline-offset: 3px;
    font-size: 12px;
    cursor: pointer;
    padding: 0;
  }
  @media (prefers-reduced-motion: reduce) {
    .meter i.fill::after {
      transition: none;
    }
  }
</style>
