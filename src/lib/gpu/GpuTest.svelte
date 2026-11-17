<script lang="ts">
  import { onMount } from "svelte";
  import { gpuApi, gpuErrorText, onGpuProgress, type GpuBenchmark, type GpuRun } from "./api";

  let running = $state(false);
  let line = $state("");
  let result = $state<GpuBenchmark | null>(null);
  let error = $state<string | null>(null);
  let copied = $state(false);

  onMount(() => {
    const sub = onGpuProgress((l) => (line = l));
    return () => {
      sub.then((f) => f());
    };
  });

  async function start() {
    running = true;
    error = null;
    result = null;
    copied = false;
    try {
      result = await gpuApi.benchmark();
    } catch (e) {
      error = gpuErrorText(e);
    } finally {
      running = false;
      line = "";
    }
  }

  async function copy() {
    if (!result) return;
    await navigator.clipboard.writeText(result.text);
    copied = true;
  }

  const de = (x: number, digits: number) =>
    x.toLocaleString("de-DE", { minimumFractionDigits: digits, maximumFractionDigits: digits });
  const time = (r: GpuRun) => `${de(r.ms / 1000, 2)} s`;
</script>

<section class="panel gpu">
  <div class="panel-head"><h2 class="panel-title">GPU-Test</h2></div>
  <div class="body">
    <p class="hint">
      Prüft, ob der Solver auf deiner Grafikkarte schneller rechnet: Ein River- und ein Turn-Spot werden auf allen
      CPU-Kernen und auf der GPU bis zur selben Genauigkeit gelöst. Das dauert bis zu ein paar Minuten, der PC ist
      währenddessen ausgelastet.
    </p>
    <div class="row">
      <button class="btn" onclick={start} disabled={running}>{running ? "Läuft …" : "GPU-Test starten"}</button>
      {#if running && line}<span class="dim">{line}</span>{/if}
      {#if result}
        <button class="btn" onclick={copy}>Ergebnis kopieren</button>
        {#if copied}<span class="dim">Kopiert.</span>{/if}
      {/if}
    </div>
    {#if error}<p class="bad">{error}</p>{/if}

    {#if result}
      {#if !result.gpu}
        <p>Keine GPU gefunden. Der Solver rechnet wie bisher auf der CPU.</p>
      {:else}
        <p class="dim">
          {result.gpu.name} · {result.gpu.backend}{result.gpu.driver ? ` · Treiber ${result.gpu.driver}` : ""} · CPU mit {result.cpu_threads}
          Threads · Ziel {de(result.target_pct, 1)} % des Pots
        </p>
        <table>
          <thead>
            <tr>
              <th>Spot</th>
              <th class="r">CPU</th>
              <th class="r">GPU</th>
              <th class="r">Faktor</th>
              <th class="r">Abweichung</th>
              <th></th>
            </tr>
          </thead>
          <tbody>
            {#each result.spots as spot (spot.name)}
              <tr>
                <td>
                  {spot.name} <span class="dim num">{spot.board}</span>
                  <div class="dim small num">{spot.hands[0]}/{spot.hands[1]} Hände · {spot.nodes.toLocaleString("de-DE")} Knoten</div>
                </td>
                <td class="r num">{time(spot.cpu)}<div class="dim small">{spot.cpu.iterations} It.</div></td>
                {#if spot.gpu}
                  <td class="r num">{time(spot.gpu)}<div class="dim small">{spot.gpu.iterations} It.</div></td>
                  <td class="r num">{de(spot.speedup_per_iteration ?? 0, 1)}×</td>
                  <td class="r num">{de(Math.abs(spot.value_diff_pct ?? 0), 3)} %</td>
                  <td class:good={spot.matches} class:bad={!spot.matches}>{spot.matches ? "gleich" : "abweichend"}</td>
                {:else}
                  <td class="bad" colspan="4">{spot.gpu_error}</td>
                {/if}
              </tr>
            {/each}
          </tbody>
        </table>
        <p class="dim small">
          Zeiten bis zum Ziel. Faktor: Iterationen pro Sekunde der GPU geteilt durch die der CPU; beide rechnen denselben
          Algorithmus, durch Rundung brauchen sie aber nicht immer gleich viele Iterationen. Abweichung: Unterschied im
          Erwartungswert beider Lösungen, in Prozent des Pots.
        </p>
      {/if}
    {/if}
  </div>
</section>

<style>
  .body {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 12px 18px 18px;
  }
  p {
    margin: 0;
    line-height: 1.5;
  }
  .hint,
  .dim {
    color: var(--text-2);
  }
  .small {
    font-size: 12px;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
  table {
    width: 100%;
    border-collapse: collapse;
  }
  th {
    font-weight: 500;
    font-size: 12px;
    color: var(--text-3);
    text-align: left;
    padding: 0 8px 6px 0;
  }
  td {
    padding: 8px 8px 8px 0;
    border-top: 1px solid var(--line);
    vertical-align: top;
  }
  .r {
    text-align: right;
  }
  .good {
    color: var(--pos);
  }
  .bad {
    color: var(--neg);
  }
</style>
