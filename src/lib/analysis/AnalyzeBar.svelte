<script lang="ts">
  import { api } from "../api";
  import { int } from "../app/format";
  import { duration } from "./labels";
  import { analysis, watchAnalysis } from "./store.svelte";

  /** The one-click run over the whole history, with its progress. */

  watchAnalysis();
  const s = $derived(analysis.status);
  const c = $derived(s?.counts);
  const started = $derived(!!s && (s.enabled || (c?.analyzed ?? 0) > 0));
  const share = $derived(c && c.hands > 0 ? c.analyzed / c.hands : 0);
  const p = $derived(s?.progress);
  const detail = $derived.by(() => {
    if (!p) return "";
    return p.eta_secs && p.eta_secs > 1 ? `${p.detail} · noch etwa ${duration(p.eta_secs)}` : p.detail;
  });
  let busy = $state(false);

  async function act(f: () => Promise<void>) {
    busy = true;
    try {
      await f();
    } finally {
      busy = false;
    }
  }
</script>

{#if s && c}
  <div class="bar">
    {#if !started}
      <div class="text">
        <b>Wo verlierst du Chips?</b>
        <span>
          Ringside bewertet jede deiner Entscheidungen: vor dem Flop sofort, nach dem Flop nach und nach im Hintergrund. Du
          siehst danach deine teuersten Fehler zuerst.
        </span>
      </div>
      <button class="btn primary go" disabled={busy || s.state === "locked"} onclick={() => act(() => api.analysisRun(true))}>
        {int(c.hands)} Hände analysieren
      </button>
    {:else if p}
      <div class="text" role="status">
        <b>{p.title}</b>
        <span class="num">{detail}</span>
        <div class="track" aria-hidden="true">
          <i style="width:{share * 100}%"></i>
          {#if c.pending > 0 && c.decisions > 0}
            <i class="post" style="width:{(c.pending / c.decisions) * 100}%"></i>
          {/if}
        </div>
      </div>
      {#if p.state === "running"}
        <button
          class="btn ghost"
          disabled={busy}
          onclick={() => act(api.analysisCancel)}
          title="Hält die Analyse an. Bewertetes bleibt gespeichert; mit „Fortsetzen“ geht es dort weiter."
        >
          Abbrechen
        </button>
      {:else if p.state !== "paused_pokerstars" && p.state !== "done"}
        <button class="btn primary" disabled={busy} onclick={() => act(() => api.analysisRun(true))}>Fortsetzen</button>
      {/if}
    {/if}
  </div>
{/if}

<style>
  .bar {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 12px 16px;
    border: 1px solid var(--line);
    border-radius: var(--r-lg);
    background: var(--surface);
  }
  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: 13px;
    color: var(--text-2);
  }
  .text b {
    font-size: 14px;
    font-weight: 600;
    color: var(--text);
  }
  .go {
    flex-shrink: 0;
  }
  .track {
    position: relative;
    height: 4px;
    margin-top: 5px;
    border-radius: 2px;
    background: var(--surface-3);
    overflow: hidden;
  }
  .track i {
    position: absolute;
    inset: 0 auto 0 0;
    background: var(--pos);
    border-radius: 2px;
    transition: width 0.4s ease;
  }
  .track i.post {
    left: auto;
    right: 0;
    background: repeating-linear-gradient(-45deg, var(--amber) 0 4px, transparent 4px 7px);
    opacity: 0.7;
  }
  @media (prefers-reduced-motion: reduce) {
    .track i {
      transition: none;
    }
  }
</style>
