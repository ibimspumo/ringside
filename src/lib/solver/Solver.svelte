<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "../api";
  import ClientLock from "../app/ClientLock.svelte";
  import { bytes, duration } from "./actions";
  import BoardPicker from "./BoardPicker.svelte";
  import { handoff } from "./handoff.svelte";
  import { defaultSizes } from "./presets";
  import RangeEditor from "./RangeEditor.svelte";
  import SolverResult from "./SolverResult.svelte";
  import { applyPrefill, listen, solver } from "./store.svelte";
  import type { SolverEstimate, StreetSizes } from "./types";

  onMount(listen);

  // A spot sent from the replayer replaces the current inputs.
  $effect(() => {
    const p = handoff.prefill;
    if (!p) return;
    applyPrefill(p);
    handoff.prefill = null;
  });

  const STREETS = ["Flop", "Turn", "River"];
  let board = $state(cardsOf(solver.spot.board));
  $effect(() => {
    board = cardsOf(solver.spot.board);
  });

  function cardsOf(text: string): string[] {
    return text.replace(/[^AKQJT2-9cdhs]/gi, "").match(/.{2}/g) ?? [];
  }

  function setBoard() {
    solver.spot.board = board.join("");
  }

  /** Comma-separated percentages as typed; kept as text so half-typed values are fine. */
  let sizeText = $state(toText(solver.spot.sizes));
  $effect(() => {
    sizeText = toText(solver.spot.sizes);
  });

  function toText(sizes: StreetSizes[]) {
    return sizes.map((s) => ({ bet: s.bet.join(", "), raise: s.raise.join(", ") }));
  }

  function parseSizes(text: string): number[] | null {
    const parts = text
      .split(/[,;\s]+/)
      .map((t) => t.replace("%", "").trim())
      .filter(Boolean);
    const nums = parts.map((t) => Number(t.replace(",", ".")));
    return nums.every((n) => Number.isFinite(n) && n > 0) ? nums : null;
  }

  function setSizes() {
    const next = sizeText.map((s, i) => ({
      bet: parseSizes(s.bet) ?? solver.spot.sizes[i].bet,
      raise: parseSizes(s.raise) ?? solver.spot.sizes[i].raise,
    }));
    solver.spot.sizes = next as typeof solver.spot.sizes;
  }

  const firstStreet = $derived(Math.max(0, Math.min(2, board.length - 3)));

  /** Edits after a solve keep the old result reachable until the next start. */
  function edited() {
    if (solver.status !== "running") solver.error = null;
  }

  let estimate = $state<SolverEstimate | null>(null);
  let estimateError = $state<string | null>(null);
  $effect(() => {
    const spot = $state.snapshot(solver.spot);
    const threads = solver.settings.threads;
    if (cardsOf(spot.board).length < 3) {
      estimate = null;
      estimateError = "Wähle mindestens drei Boardkarten.";
      return;
    }
    let live = true;
    const t = setTimeout(() => {
      api
        .solverEstimate(spot, threads)
        .then((e) => {
          if (!live) return;
          estimate = e;
          estimateError = null;
        })
        .catch((e) => {
          if (!live) return;
          estimate = null;
          estimateError = String(e);
        });
    }, 350);
    return () => {
      live = false;
      clearTimeout(t);
    };
  });

  async function start() {
    solver.error = null;
    solver.progress = null;
    solver.startPct = null;
    solver.summary = null;
    solver.status = "running";
    const spot = $state.snapshot(solver.spot);
    solver.solved = spot;
    solver.run += 1;
    try {
      await api.solverStart(spot, $state.snapshot(solver.settings), solver.run);
    } catch (e) {
      solver.status = "failed";
      solver.error = String(e);
    }
  }

  function cancel() {
    api.solverCancel();
  }

  function discard() {
    api.solverClear();
    solver.summary = null;
    solver.solved = null;
    solver.status = "idle";
    solver.view = "setup";
  }

  function resetSizes() {
    solver.spot.sizes = defaultSizes();
  }

  /** Bar position on a log scale from the first measurement down to the target. */
  const progressShare = $derived.by(() => {
    const p = solver.progress;
    if (!p || solver.startPct === null) return 0;
    const from = Math.log(Math.max(solver.startPct, solver.settings.target_pct * 1.01));
    const to = Math.log(solver.settings.target_pct);
    const now = Math.log(Math.max(p.exploitability_pct, solver.settings.target_pct));
    return Math.min(1, Math.max(0, (from - now) / (from - to)));
  });
  const running = $derived(solver.status === "running");
  const canStart = $derived(!!estimate?.fits && !running);
  const pctText = (x: number) => x.toLocaleString("de-DE", { maximumFractionDigits: x < 1 ? 2 : 1 });
</script>

<ClientLock feature="Der Solver">
  <div class="page solver">
    {#if solver.view === "result" && solver.summary}
      <SolverResult onEdit={() => (solver.view = "setup")} onNew={discard} />
    {:else}
      <div class="page-head">
        <div>
          <h1>Spot-Werkstatt</h1>
          <p>Heads-up ab Flop, Turn oder River – gelöst für die Ranges und Einsatzgrößen, die du hier festlegst.</p>
        </div>
        {#if solver.summary}
          <button class="btn" onclick={() => (solver.view = "result")}>Zur letzten Lösung</button>
        {/if}
      </div>
      {#if solver.source}
        <p class="source">Vorbelegt aus <b>{solver.source}</b>: Board, Pot und effektiver Stack stammen aus der Hand.</p>
      {/if}

      <div class="layout">
        <div class="col">
          <section class="panel box">
            <h3>Board</h3>
            <BoardPicker bind:board onEdit={() => (setBoard(), edited())} />
          </section>

          <section class="panel box">
            <h3>Pot und Stacks</h3>
            <div class="row2">
              <label>
                <span>Pot (BB)</span>
                <input class="field num" type="number" min="0.1" step="0.5" bind:value={solver.spot.pot} oninput={edited} />
              </label>
              <label>
                <span>Effektiver Stack (BB)</span>
                <input class="field num" type="number" min="0.1" step="1" bind:value={solver.spot.stack} oninput={edited} />
              </label>
            </div>
            <p class="faint small">
              SPR {(solver.spot.stack / Math.max(solver.spot.pot, 0.01)).toLocaleString("de-DE", { maximumFractionDigits: 1 })} · OOP handelt
              auf jeder Straße zuerst.
            </p>
          </section>

          <section class="panel box">
            <div class="h3row">
              <h3>Einsatzgrößen</h3>
              <button class="link" onclick={resetSizes}>Standard</button>
            </div>
            <table class="sizes">
              <thead><tr><th></th><th>Bet in % Pot</th><th>Raise in % Pot</th></tr></thead>
              <tbody>
                {#each STREETS as street, i (street)}
                  <tr class:off={i < firstStreet}>
                    <td>{street}</td>
                    <td>
                      <input
                        class="field num"
                        class:bad={parseSizes(sizeText[i].bet) === null && sizeText[i].bet.trim() !== ""}
                        bind:value={sizeText[i].bet}
                        onchange={() => (setSizes(), edited())}
                        disabled={i < firstStreet}
                        aria-label="Bet-Größen {street}"
                      />
                    </td>
                    <td>
                      <input
                        class="field num"
                        bind:value={sizeText[i].raise}
                        onchange={() => (setSizes(), edited())}
                        disabled={i < firstStreet}
                        aria-label="Raise-Größen {street}"
                      />
                    </td>
                  </tr>
                {/each}
              </tbody>
            </table>
            <label class="inline">
              <span>All-in anbieten bis</span>
              <input class="field num narrow" type="number" min="0" step="10" bind:value={solver.spot.allin_threshold} oninput={edited} />
              <span>% des Pots</span>
            </label>
            <p class="faint small">Mehrere Größen mit Komma trennen, z. B. „33, 75“. Raises: Prozent des Pots nach dem Call. Jede Größe mehr vervielfacht Speicher und Zeit.</p>
          </section>

          <section class="panel box">
            <h3>Genauigkeit</h3>
            <div class="row3">
              <label>
                <span>Ziel-Ausbeutbarkeit (% Pot)</span>
                <input class="field num" type="number" min="0.05" max="20" step="0.1" bind:value={solver.settings.target_pct} />
              </label>
              <label>
                <span>Threads</span>
                <input class="field num" type="number" min="1" max="256" step="1" bind:value={solver.settings.threads} />
              </label>
              <label>
                <span>Max. Iterationen</span>
                <input class="field num" type="number" min="10" step="100" bind:value={solver.settings.max_iterations} />
              </label>
            </div>
          </section>
        </div>

        <div class="ranges">
          <RangeEditor
            title="OOP"
            bind:position={solver.positions.oop}
            bind:value={solver.spot.oop_range}
            note={solver.notes.oop}
            onEdit={() => ((solver.notes.oop = ""), edited())}
          />
          <RangeEditor
            title="IP"
            bind:position={solver.positions.ip}
            bind:value={solver.spot.ip_range}
            note={solver.notes.ip}
            onEdit={() => ((solver.notes.ip = ""), edited())}
          />
        </div>
      </div>

      <section class="panel go" aria-live="polite">
        {#if running}
          <div class="progress">
            <div class="ptext">
              <span>Löse … Iteration <b class="num">{solver.progress?.iteration ?? 0}</b></span>
              <span
                >Ausbeutbarkeit <b class="num">{solver.progress ? `${pctText(solver.progress.exploitability_pct)} %` : "–"}</b> · Ziel
                <b class="num">{pctText(solver.settings.target_pct)} %</b></span
              >
              <span class="faint num">{duration((solver.progress?.elapsed_ms ?? 0) / 1000)}</span>
            </div>
            <div class="bar" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow={Math.round(progressShare * 100)}>
              <i style="width:{progressShare * 100}%"></i>
            </div>
          </div>
          <button class="btn danger" onclick={cancel}>Abbrechen</button>
        {:else}
          <div class="est">
            {#if estimate}
              <span>Speicher <b class="num" class:neg={!estimate.fits}>{bytes(estimate.memory_bytes)}</b>{estimate.compressed ? " (16-Bit-Speicher)" : ""}</span>
              <span>Zeit bis 0,5 % <b class="num">ca. {duration(estimate.seconds)}</b></span>
              <span class="dim num">{Math.round(estimate.combos[0])} gegen {Math.round(estimate.combos[1])} Kombos</span>
              {#if !estimate.fits}
                <span class="neg">Zu groß: erlaubt sind {bytes(estimate.memory_limit)}. Weniger Größen oder engere Ranges.</span>
              {/if}
            {:else if estimateError}
              <span class="neg">{estimateError}</span>
            {:else}
              <span class="faint">Schätze …</span>
            {/if}
            {#if solver.error}<span class="neg">{solver.error}</span>{/if}
            {#if solver.status === "cancelled" && !solver.error}<span class="dim">Abgebrochen.</span>{/if}
          </div>
          <button class="btn primary" disabled={!canStart} onclick={start}>Lösen</button>
        {/if}
      </section>
    {/if}
  </div>
</ClientLock>

<style>
  .solver {
    container-type: inline-size;
  }
  .source {
    margin: -8px 0 16px;
    color: var(--text-2);
    font-size: 13px;
  }
  .source b {
    color: var(--text);
    font-weight: 600;
  }
  .layout {
    display: grid;
    grid-template-columns: minmax(300px, 370px) minmax(0, 1fr);
    gap: 16px;
    align-items: start;
  }
  .col {
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-width: 0;
  }
  .ranges {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 12px;
    container-type: inline-size;
  }
  .box {
    padding: 14px;
  }
  h3 {
    margin: 0 0 10px;
    font-size: 13px;
    font-weight: 600;
  }
  .h3row {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
  }
  .link {
    border: 0;
    background: none;
    color: var(--text-2);
    font-size: 12px;
    cursor: pointer;
    text-decoration: underline;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 12px;
    color: var(--text-2);
    min-width: 0;
  }
  label .field {
    width: 100%;
  }
  .row2,
  .row3 {
    display: grid;
    gap: 10px;
  }
  .row2 {
    grid-template-columns: 1fr 1fr;
  }
  .row3 {
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }
  .inline {
    flex-direction: row;
    align-items: center;
    gap: 8px;
    margin-top: 10px;
  }
  .narrow {
    width: 70px !important;
  }
  .small {
    margin: 8px 0 0;
    font-size: 11.5px;
    line-height: 1.45;
  }
  .sizes {
    width: 100%;
    border-collapse: collapse;
    font-size: 12.5px;
  }
  .sizes th {
    color: var(--text-3);
    font-weight: 500;
    font-size: 11px;
    text-align: left;
    padding: 0 0 4px 6px;
  }
  .sizes td {
    padding: 2px 0 2px 6px;
  }
  .sizes td:first-child {
    padding-left: 0;
    color: var(--text-2);
    width: 46px;
  }
  .sizes .field {
    width: 100%;
    height: 28px;
  }
  .sizes .field.bad {
    border-color: var(--neg);
  }
  .sizes tr.off {
    opacity: 0.4;
  }
  .go {
    position: sticky;
    bottom: 12px;
    z-index: 3;
    display: flex;
    align-items: center;
    gap: 16px;
    margin-top: 16px;
    padding: 12px 16px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.35);
  }
  .est {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 18px;
    flex: 1;
    font-size: 13px;
    color: var(--text-2);
  }
  .est b {
    color: var(--text);
    font-weight: 500;
  }
  .est b.neg {
    color: var(--neg);
  }
  .go .btn.primary {
    min-width: 110px;
  }
  .progress {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }
  .ptext {
    display: flex;
    flex-wrap: wrap;
    gap: 2px 18px;
    font-size: 13px;
    color: var(--text-2);
  }
  .ptext b {
    color: var(--text);
    font-weight: 500;
  }
  .bar {
    height: 6px;
    border-radius: 3px;
    background: var(--surface-3);
    overflow: hidden;
  }
  .bar i {
    display: block;
    height: 100%;
    background: var(--pos);
    transition: width 0.25s;
  }
  @container (max-width: 1000px) {
    .ranges {
      grid-template-columns: minmax(0, 1fr);
    }
  }
  @container (max-width: 820px) {
    .layout {
      grid-template-columns: minmax(0, 1fr);
    }
    .ranges {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }
  @container (max-width: 640px) {
    .ranges {
      grid-template-columns: minmax(0, 1fr);
    }
    .row3 {
      grid-template-columns: 1fr 1fr;
    }
    .go {
      flex-wrap: wrap;
    }
  }
</style>
