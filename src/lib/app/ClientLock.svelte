<script lang="ts">
  import { onMount, type Snippet } from "svelte";
  import { api, on } from "../api";

  /** Shows `children` only while the PokerStars client is closed. */
  let { feature = "Training", children }: { feature?: string; children: Snippet } = $props();

  // Unknown until the backend answers; nothing is shown meanwhile, so locked content never flashes.
  let running = $state<boolean | null>(null);

  onMount(() => {
    const sub = on<boolean>("poker-client", (r) => (running = r));
    sub.then(() => api.pokerClientRunning().then((r) => (running = r)));
    return () => sub.then((f) => f());
  });
</script>

{#if running === false}
  {@render children()}
{:else if running}
  <div class="lock" role="status">
    <div class="card">
      <svg viewBox="0 0 24 24" width="28" height="28" aria-hidden="true">
        <rect x="5" y="10.5" width="14" height="9.5" rx="2" />
        <path d="M8.5 10.5V8a3.5 3.5 0 0 1 7 0v2.5M12 14.5v2" />
      </svg>
      <p class="msg">
        {feature} ist gesperrt, solange PokerStars läuft – so verlangen es die PokerStars-Regeln für Hilfsprogramme. HUD und
        Statistiken laufen normal weiter.
      </p>
      <p class="dim">Sobald du PokerStars schließt, ist alles wieder da.</p>
    </div>
  </div>
{/if}

<style>
  .lock {
    display: grid;
    place-items: center;
    min-height: 100%;
    padding: 32px 24px;
  }
  .card {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    max-width: 460px;
    text-align: center;
  }
  svg {
    fill: none;
    stroke: var(--text-2);
    stroke-width: 1.5;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  p {
    margin: 0;
  }
  .msg {
    font-size: 15px;
    line-height: 1.55;
    color: var(--text);
    text-wrap: pretty;
  }
  .dim {
    font-size: 13px;
  }
</style>
