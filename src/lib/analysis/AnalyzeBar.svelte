<script lang="ts">
  import { api } from "../api";
  import Button from "../ui/Button.svelte";
  import { int } from "../ui/format";
  import { duration } from "./labels";
  import { analysis, watchAnalysis } from "./store.svelte";

  /** The one-click run over the whole history, with its progress. */
  let { compact = false }: { compact?: boolean } = $props();

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
  /** Phase one rates every hand before the flop; phase two solves the flop spots in the background. */
  const preDone = $derived(!!c && c.hands > 0 && c.analyzed >= c.hands);
  const postShare = $derived(preDone && p?.share != null ? p.share : 0);
  const eta = $derived(p?.eta_secs && p.eta_secs > 1 ? `noch etwa ${duration(p.eta_secs)}` : "");
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
  <div class="bar" class:compact>
    {#if !started}
      <div class="text">
        <b>Wo verlierst du Chips?</b>
        <span>
          Ringside bewertet jede deiner Entscheidungen: vor dem Flop sofort, nach dem Flop nach und nach im Hintergrund. Du
          siehst danach deine teuersten Fehler zuerst.
        </span>
      </div>
      <Button variant="primary" disabled={busy || s.state === "locked"} onclick={() => act(() => api.analysisRun(true))}>
        {int(c.hands)} Hände analysieren
      </Button>
    {:else if p}
      <div class="text" role="status">
        <b
          >{p.state === "running" && !p.title.includes("#")
            ? preDone
              ? "Vor dem Flop fertig · nach dem Flop läuft"
              : "Analyse läuft – vor dem Flop"
            : p.title}</b
        >
        {#if p.state !== "running"}<span class="num">{detail}</span>{/if}
        <div class="phases">
          <div class="phase" class:done={preDone}>
            <span class="ph-head">
              <span class="step num" aria-hidden="true">{#if preDone}✓{:else}1{/if}</span>Vor dem Flop
            </span>
            <span class="track" aria-hidden="true"><i style="width:{share * 100}%"></i></span>
            <small
              >{preDone ? `fertig · alle ${int(c.hands)} gespeicherten Hände` : `${int(c.analyzed)} von ${int(c.hands)} Händen`}</small
            >
          </div>
          <div class="phase" class:waiting={!preDone}>
            <span class="ph-head"><span class="step num" aria-hidden="true">2</span>Nach dem Flop</span>
            <span class="track" aria-hidden="true">
              <i class="post" class:busy={preDone && !!c.queued && p.state === "running"} style="width:{postShare * 100}%"></i>
            </span>
            <small
              >{!preDone
                ? "startet danach"
                : c.queued
                  ? `${Math.floor(postShare * 100)}\u00a0%${eta ? ` · ${eta}` : ""} · ${int(c.queued)} Flop-Spots im Solver`
                  : "alles gerechnet"}</small
            >
          </div>
        </div>
      </div>
      {#if p.state === "running"}
        <Button
          variant="quiet"
          disabled={busy}
          onclick={() => act(api.analysisCancel)}
          title="Hält die Analyse an. Bewertetes bleibt gespeichert; mit „Fortsetzen“ geht es dort weiter."
        >
          Abbrechen
        </Button>
      {:else if p.state !== "paused_pokerstars" && p.state !== "done"}
        <Button variant="primary" disabled={busy} onclick={() => act(() => api.analysisRun(true))}>Fortsetzen</Button>
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
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .text b {
    font-size: var(--fs-base);
    font-weight: 600;
    color: var(--text);
  }
  .phases {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 6px 24px;
    margin-top: 6px;
  }
  .phase {
    display: grid;
    gap: 4px;
    min-width: 0;
  }
  .ph-head {
    display: flex;
    align-items: center;
    gap: 7px;
    font-size: var(--fs-md);
    font-weight: 600;
    color: var(--text);
  }
  .step {
    display: grid;
    place-items: center;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: var(--surface-3);
    font-size: var(--fs-xs);
  }
  .done .step {
    background: color-mix(in srgb, var(--pos) 25%, transparent);
    color: var(--pos);
  }
  .waiting {
    opacity: 0.6;
  }
  .phase small {
    line-height: 1.35;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .track {
    position: relative;
    height: 4px;
    border-radius: 2px;
    background: var(--surface-3);
    overflow: hidden;
  }
  .track i {
    position: absolute;
    inset: 0 auto 0 0;
    background: var(--pos);
    border-radius: 2px;
    transition: width var(--dur-slow) var(--ease-enter);
  }
  .track i.post {
    background: var(--pos);
  }
  /* The flop spots still run: the filled part keeps moving so the bar never reads as finished. */
  .track i.busy {
    background: repeating-linear-gradient(
      -45deg,
      var(--amber) 0 6px,
      color-mix(in srgb, var(--amber) 55%, transparent) 6px 12px
    );
    background-size: 17px 100%;
    animation: stripes var(--dur-loop) var(--ease-standard) infinite;
  }
  @keyframes stripes {
    to {
      background-position: 17px 0;
    }
  }
  .compact {
    padding: 8px 14px;
  }
  .compact .text {
    flex-direction: row;
    flex-wrap: wrap;
    align-items: baseline;
    column-gap: 12px;
  }
  .compact .phases {
    flex-basis: 100%;
    margin-top: 2px;
  }
  @media (prefers-reduced-motion: reduce) {
    .track i {
      transition: none;
    }
  }
</style>
