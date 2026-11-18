<script lang="ts">
  import { api } from "../api";
  import Button from "../ui/Button.svelte";
  import ProgressBar from "../ui/ProgressBar.svelte";
  import { duration } from "./actions";
  import { solver } from "./store.svelte";
  import Term from "./Term.svelte";

  /** A running solve: how far it is, how long it has taken, and a way to stop it. */

  /** Bar position on a log scale from the first measurement down to the target. */
  const share = $derived.by(() => {
    const p = solver.progress;
    if (!p || solver.startPct === null) return null;
    const from = Math.log(Math.max(solver.startPct, solver.settings.target_pct * 1.01));
    const to = Math.log(solver.settings.target_pct);
    const now = Math.log(Math.max(p.exploitability_pct, solver.settings.target_pct));
    return Math.min(1, Math.max(0, (from - now) / (from - to)));
  });
  const pctText = (x: number) => x.toLocaleString("de-DE", { maximumFractionDigits: x < 1 ? 2 : 1 });
</script>

<div class="solving">
  <div class="text">
    <b>Ringside löst den Spot …</b>
    <span class="num">{share === null ? "startet" : `${Math.round(share * 100)} %`}</span>
    <span>
      <Term term="exploit">Abstand zum Ziel</Term>
      <b class="num">{solver.progress ? `${pctText(solver.progress.exploitability_pct)} %` : "–"}</b> · Ziel
      <b class="num">{pctText(solver.settings.target_pct)} %</b>
    </span>
    <span class="faint num">{duration((solver.progress?.elapsed_ms ?? 0) / 1000)}</span>
  </div>
  <ProgressBar value={share} tone="good" size="md" label="Fortschritt des Solvers" />
</div>
<Button variant="danger" onclick={() => api.solverCancel()}>Abbrechen</Button>

<style>
  .solving {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    min-width: 0;
  }
  .text {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 2px var(--sp-4);
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .text b {
    color: var(--text);
    font-weight: var(--fw-medium);
  }
  .text > b:first-child {
    font-weight: var(--fw-semibold);
  }
</style>
