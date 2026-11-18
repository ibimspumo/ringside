<script lang="ts">
  import { onMount } from "svelte";
  import { gpuApi, gpuErrorText, gpuPreview, onGpuProgress, type GpuBenchmark, type GpuRun, type GpuSpotResult } from "./api";
  import Button from "../ui/Button.svelte";
  import CardText from "../ui/CardText.svelte";
  import EmptyState from "../ui/EmptyState.svelte";
  import ProgressBar from "../ui/ProgressBar.svelte";
  import SettingRow from "../ui/SettingRow.svelte";
  import Tag from "../ui/Tag.svelte";

  /** `locked`: PokerStars runs, so the test may not start. */
  let { locked = false }: { locked?: boolean } = $props();

  const preview = gpuPreview();
  let running = $state(preview?.running ?? false);
  let line = $state(preview?.line ?? "");
  let result = $state<GpuBenchmark | null>(preview?.result ?? null);
  let error = $state<string | null>(preview?.error ?? null);
  let copied = $state(false);
  let details = $state(false);

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
  const time = (r: GpuRun) => (r.ms < 1000 ? `${Math.round(r.ms)} ms` : `${de(r.ms / 1000, 2)} s`);
  /** How many times faster the GPU reaches the target than the CPU: the times shown side by side. */
  const factor = (s: GpuSpotResult) => s.speedup ?? (s.gpu ? s.cpu.ms / s.gpu.ms : null);

  /** The verdict comes from the largest spot the GPU solved: small spots say little about speed. */
  const verdict = $derived.by(() => {
    if (!result?.gpu) return null;
    const solved = result.spots.filter((s) => s.gpu && factor(s) != null);
    if (!solved.length) return { same: false, text: "Die GPU konnte keinen Spot lösen. Der Solver rechnet wie bisher auf der CPU." };
    const big = solved.reduce((a, b) => (b.nodes > a.nodes ? b : a));
    const f = factor(big)!;
    const same = result.spots.every((s) => s.matches);
    const speed =
      f >= 1.05
        ? `Beim ${big.name}-Spot ist deine GPU ${de(f, 1)}× so schnell am Ziel wie die CPU`
        : f > 0.95
          ? `Beim ${big.name}-Spot sind GPU und CPU etwa gleich schnell`
          : `Beim ${big.name}-Spot ist deine GPU langsamer als die CPU (${de(f, 1)}×)`;
    return {
      same,
      text: `Du musst nichts tun – Ringside rechnet weiter auf dem Prozessor. ${speed}.${same ? "" : " Achtung: Die Lösungen weichen voneinander ab."}`,
    };
  });
</script>

<SettingRow
  title="GPU-Test"
  sub="Misst, ob der Solver auf deiner Grafikkarte schneller rechnen würde. Nur ein Vergleich: Gerechnet wird weiter auf dem Prozessor."
  {locked}
>
  {#snippet more()}
    <p>
      Ein River- und ein Turn-Spot werden auf allen CPU-Kernen und auf der GPU bis zur selben Genauigkeit gelöst. Das dauert
      bis zu ein paar Minuten, der PC ist währenddessen ausgelastet. Die Rechnung auf der Grafikkarte ist noch ein Prototyp;
      mit „Ergebnis kopieren“ kannst du deine Messung dem Projekt schicken.
    </p>
  {/snippet}
  {#snippet control()}
    {#if result}<Button size="sm" variant="quiet" onclick={copy}>{copied ? "Kopiert" : "Ergebnis kopieren"}</Button>{/if}
    <Button size="sm" onclick={start} loading={running} disabled={running || locked}>
      {running ? "Läuft …" : result || error ? "Erneut testen" : "Test starten"}
    </Button>
  {/snippet}

  {#if running || error || result}
    <div class="out">
      {#if running}
        <div class="run">
          <ProgressBar value={null} label="GPU-Test läuft" />
          <p class="soft">{line || "Startet …"} Der PC ist gerade ausgelastet.</p>
        </div>
      {:else if error}
        <EmptyState inline tone="error" title="Der Test ist nicht durchgelaufen" text={error} />
      {:else if result && !result.gpu}
        <p class="verdict">Keine GPU gefunden. Du musst nichts tun – der Solver rechnet wie bisher auf der CPU.</p>
      {:else if result?.gpu}
        {#if verdict}
          <p class="verdict" class:bad={!verdict.same}>{verdict.text}</p>
          {#if verdict.same}<span class="same"><Tag><span class="ok-dot"></span>GPU und CPU kommen praktisch auf dieselbe Lösung</Tag></span>{/if}
        {/if}
        <div class="facts">
        <p class="soft">
          {result.gpu.name} · {result.gpu.backend}{result.gpu.driver ? ` · Treiber ${result.gpu.driver}` : ""} · CPU mit {result.cpu_threads}
          Threads · Ziel {de(result.target_pct, 1)} % des Pots
        </p>
        <Button size="sm" variant="quiet" aria-expanded={details} onclick={() => (details = !details)}>
          {details ? "Messwerte ausblenden" : "Messwerte zeigen"}
        </Button>
        </div>
        {#if details}
        <table>
          <thead>
            <tr>
              <th>Spot</th>
              <th class="r">CPU</th>
              <th class="r">GPU</th>
              <th class="r" title="Zeit der CPU geteilt durch die Zeit der GPU">GPU-Tempo</th>
              <th class="r" title="Unterschied im Erwartungswert beider Lösungen, in Prozent des Pots">Abweichung</th>
              <th class="r">Lösung</th>
            </tr>
          </thead>
          <tbody>
            {#each result.spots as spot (spot.name)}
              <tr>
                <td>
                  {spot.name} <CardText text={spot.board.match(/../g)?.join(" ") ?? spot.board} />
                  <div class="soft small num">{spot.hands[0]}/{spot.hands[1]} Hände · {spot.nodes.toLocaleString("de-DE")} Knoten</div>
                </td>
                <td class="r num">{time(spot.cpu)}<div class="soft small">{spot.cpu.iterations} Durchläufe</div></td>
                {#if spot.gpu}
                  <td class="r num">{time(spot.gpu)}<div class="soft small">{spot.gpu.iterations} Durchläufe</div></td>
                  {@const f = factor(spot) ?? 0}
                  <td class="r num strong">{de(f, 1)}×<div class="soft small">{f >= 1.05 ? "GPU schneller" : f > 0.95 ? "etwa gleich" : "GPU langsamer"}</div></td>
                  <td class="r num">{de(Math.abs(spot.value_diff_pct ?? 0), 3)} %</td>
                  <td class="r" class:good={spot.matches} class:bad={!spot.matches}>{spot.matches ? "praktisch gleich" : "abweichend"}</td>
                {:else}
                  <td class="bad" colspan="4">{spot.gpu_error}</td>
                {/if}
              </tr>
            {/each}
          </tbody>
        </table>
        <p class="soft small note">
          Zeiten bis zum Ziel. GPU-Tempo: Zeit der CPU geteilt durch die Zeit der GPU – über 1 ist die GPU schneller. Beide
          rechnen denselben Algorithmus, durch Rundung brauchen sie aber nicht immer gleich viele Durchläufe. Abweichung:
          Unterschied im Erwartungswert beider Lösungen, in Prozent des Pots.
        </p>
        {/if}
      {/if}
    </div>
  {/if}
</SettingRow>

<style>
  .out {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }
  p {
    margin: 0;
    line-height: 1.5;
  }
  .run {
    display: grid;
    gap: var(--sp-2);
    max-width: 68ch;
  }
  .verdict {
    max-width: 76ch;
    color: var(--text);
    font-size: var(--fs-md);
  }
  .same {
    display: flex;
  }
  .ok-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--pos);
  }
  .verdict.bad,
  .bad {
    color: var(--danger-text);
  }
  .soft {
    color: var(--text-2);
    font-size: var(--fs-md);
  }
  .small {
    font-size: var(--fs-md);
  }
  .note {
    max-width: 76ch;
  }
  .facts > :global(.ui-btn.quiet:last-child) {
    margin-right: -10px;
  }
  .facts {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-2) var(--sp-4);
  }
  .strong {
    color: var(--text);
    font-weight: var(--fw-semibold);
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--fs-md);
  }
  th {
    font-weight: var(--fw-medium);
    font-size: var(--fs-md);
    color: var(--text-2);
    text-align: left;
    padding: 0 var(--sp-3) var(--sp-2) 0;
  }
  td {
    padding: var(--sp-2) var(--sp-3) var(--sp-2) 0;
    border-top: 1px solid var(--line);
    vertical-align: top;
  }
  th:last-child,
  td:last-child {
    padding-right: 0;
  }
  .r {
    padding-left: var(--sp-6);
    text-align: right;
  }
  .good {
    color: var(--pos);
  }
</style>
