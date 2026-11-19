<script lang="ts">
  import { api } from "../api";
  import Button from "../ui/Button.svelte";
  import ProgressBar from "../ui/ProgressBar.svelte";
  import { duration } from "../ui/format";
  import { precision } from "./explain";
  import { solver } from "./store.svelte";
  import Term from "../ui/Term.svelte";

  /** A running solve: how far it is, how exact it is so far, and a way to stop it. */
  let { total = null }: { /** The estimate for the whole solve, in seconds. */ total?: number | null } = $props();

  /** Bar position on a log scale from the first measurement down to the target. */
  const share = $derived.by(() => {
    const p = solver.progress;
    if (!p || solver.startPct === null) return null;
    const from = Math.log(Math.max(solver.startPct, solver.settings.target_pct * 1.01));
    const to = Math.log(solver.settings.target_pct);
    const now = Math.log(Math.max(p.exploitability_pct, solver.settings.target_pct));
    return Math.min(1, Math.max(0, (from - now) / (from - to)));
  });
  const elapsed = $derived((solver.progress?.elapsed_ms ?? 0) / 1000);
</script>

<div class="solving">
  <div class="head">
    <b>Ringside löst den Spot …</b>
    <b class="num share">{share === null ? "startet" : `${Math.round(share * 100)} %`}</b>
  </div>
  <ProgressBar value={share} tone="good" size="md" label="Fortschritt des Solvers" />
  <p class="detail">
    <Term id="exploit">Genauigkeit</Term> bisher <b>{solver.progress ? precision(solver.progress.exploitability_pct) : "–"}</b>, Ziel
    <b>{precision(solver.settings.target_pct)}</b> · läuft seit <span class="num">{duration(elapsed)}</span>{#if total}, geschätzt
      <span class="num">{duration(total, { about: true })}</span> insgesamt{/if}
  </p>
</div>
<Button variant="quiet" onclick={() => api.solverCancel()}>Abbrechen</Button>

<style>
  .solving {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    min-width: 0;
  }
  .head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: var(--sp-3);
    font-size: var(--fs-base);
  }
  .head b {
    font-weight: var(--fw-semibold);
  }
  .share {
    font-size: var(--fs-lg);
  }
  .detail {
    margin: 0;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .detail b {
    color: var(--text);
    font-weight: var(--fw-medium);
  }
</style>
