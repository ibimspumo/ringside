<script lang="ts">
  import { api } from "../api";
  import { int } from "../app/format";
  import Button from "../ui/Button.svelte";
  import ProgressRing from "../ui/ProgressRing.svelte";
  import SourceNote from "../ui/SourceNote.svelte";
  import { duration } from "./labels";
  import { analysis, watchAnalysis } from "./store.svelte";

  /** The run over the whole history in one line, for pages that are about something else. `short`: the ring alone, its line on hover. */
  let { short = false }: { short?: boolean } = $props();

  watchAnalysis();
  const s = $derived(analysis.status);
  const c = $derived(s?.counts);
  const p = $derived(s?.progress);
  const started = $derived(!!s && (s.enabled || (c?.analyzed ?? 0) > 0));
  const preDone = $derived(!!c && c.hands > 0 && c.analyzed >= c.hands);
  const eta = $derived(p?.eta_secs && p.eta_secs > 1 ? ` · etwa ${duration(p.eta_secs)}` : "");
  const line = $derived.by(() => {
    if (!c || !p) return "";
    if (p.state === "done" || (preDone && !c.queued)) return `Alle ${int(c.hands)} Hände bewertet`;
    if (!preDone) return `Analyse: ${int(c.analyzed)} von ${int(c.hands)} Händen${eta}`;
    return `${int(c.queued)} Flop-Spots in Arbeit${eta}`;
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
  <div class="status" role="status" title={line || undefined}>
    {#if !started}
      <span class="text">Noch keine Hand bewertet.</span>
      <Button size="sm" variant="primary" disabled={busy || s.state === "locked"} onclick={() => act(() => api.analysisRun(true))}>
        {int(c.hands)} Hände analysieren
      </Button>
    {:else if p?.state === "paused_pokerstars" || s.state === "locked"}
      <SourceNote kind="locked" hint="Die Analyse macht weiter, sobald du PokerStars schließt." />
    {:else if p}
      <ProgressRing
        size={20}
        stroke={3}
        tone={p.state === "running" ? "good" : "neutral"}
        value={p.state === "done" ? 1 : preDone ? (p.share ?? null) : c.analyzed / Math.max(c.hands, 1)}
        label="Fortschritt der Analyse"
      />
      {#if short}<span class="text num">{preDone ? (c.queued ? `${int(c.queued)} offen` : "fertig") : `${Math.round((c.analyzed / Math.max(c.hands, 1)) * 100)} %`}</span>{/if}
      <span class="text num" class:hidden={short} title="Vor dem Flop ist alles sofort bewertet; die Spots nach dem Flop rechnet der Solver nach und nach. {p.detail}">{line}</span>
      {#if p.state === "running"}
        <Button size="sm" variant="quiet" disabled={busy} onclick={() => act(api.analysisCancel)} title="Hält die Analyse an. Bewertetes bleibt gespeichert.">
          Anhalten
        </Button>
      {:else if p.state !== "done"}
        <Button size="sm" disabled={busy} onclick={() => act(() => api.analysisRun(true))}>Fortsetzen</Button>
      {/if}
    {/if}
  </div>
{/if}

<style>
  .status {
    flex-shrink: 1;
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    min-width: 0;
    color: var(--text-2);
    font-size: var(--fs-sm);
  }
  .text {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .hidden {
    display: none;
  }
  .status > :global(button),
  .status > :global(.ui-ring) {
    flex-shrink: 0;
  }
</style>
