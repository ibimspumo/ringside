import { invoke } from "@tauri-apps/api/core";
import { inTauri, on } from "../api";

export interface GpuInfo {
  name: string;
  backend: string;
  driver: string;
  kind: string;
}

export interface GpuRun {
  iterations: number;
  ms: number;
  iterations_per_second: number;
  exploitability_pct: number;
  reached: boolean;
}

export interface GpuSpotResult {
  name: string;
  board: string;
  hands: [number, number];
  nodes: number;
  gpu_mb: number;
  cpu: GpuRun;
  gpu: GpuRun | null;
  gpu_error: string | null;
  speedup: number | null;
  speedup_per_iteration: number | null;
  value_diff_pct: number | null;
  strategy_diff: number | null;
  strategy_diff_max: number | null;
  matches: boolean;
}

export interface GpuBenchmark {
  gpu: GpuInfo | null;
  cpu_threads: number;
  target_pct: number;
  spots: GpuSpotResult[];
  /** Plain-text summary for the clipboard. */
  text: string;
}

const run = (iterations: number, ms: number, pct: number): GpuRun => ({
  iterations,
  ms,
  iterations_per_second: (iterations / ms) * 1000,
  exploitability_pct: pct,
  reached: true,
});

/** Browser demo: a result shaped like a real run, so the page can be designed without a GPU. */
const demoResult: GpuBenchmark = {
  gpu: { name: "Demo-GPU", backend: "Metal", driver: "", kind: "IntegratedGpu" },
  cpu_threads: 18,
  target_pct: 0.3,
  spots: [
    {
      name: "River",
      board: "QsJh7d2c5h",
      hands: [534, 607],
      nodes: 57,
      gpu_mb: 1,
      cpu: run(80, 14, 0.257),
      gpu: run(90, 20, 0.291),
      gpu_error: null,
      speedup: 0.7,
      speedup_per_iteration: 0.78,
      value_diff_pct: 0.005,
      strategy_diff: 0.029,
      strategy_diff_max: 0.66,
      matches: true,
    },
    {
      name: "Turn",
      board: "QsJh7d2c",
      hands: [548, 624],
      nodes: 16137,
      gpu_mb: 148,
      cpu: run(210, 652, 0.286),
      gpu: run(140, 378, 0.287),
      gpu_error: null,
      speedup: 1.72,
      speedup_per_iteration: 1.15,
      value_diff_pct: 0.016,
      strategy_diff: 0.054,
      strategy_diff_max: 1,
      matches: true,
    },
  ],
  text: "Ringside GPU-Test (Demo)\n",
};

let demoListeners: ((line: string) => void)[] = [];

async function demoBenchmark(): Promise<GpuBenchmark> {
  for (const line of ["Suche GPU …", "River: CPU rechnet …", "River: GPU rechnet …", "Turn: CPU rechnet …", "Turn: GPU rechnet …"]) {
    demoListeners.forEach((f) => f(line));
    await new Promise((r) => setTimeout(r, 400));
  }
  return structuredClone(demoResult);
}

export const gpuApi = {
  benchmark: (): Promise<GpuBenchmark> => (inTauri ? invoke<GpuBenchmark>("gpu_benchmark") : demoBenchmark()),
};

export async function onGpuProgress(handler: (line: string) => void): Promise<() => void> {
  if (inTauri) return on<string>("gpu-benchmark-progress", handler);
  demoListeners.push(handler);
  return () => (demoListeners = demoListeners.filter((f) => f !== handler));
}

export function gpuErrorText(e: unknown): string {
  const text = String(e);
  return text === "gesperrt" ? "Gesperrt, solange PokerStars läuft." : text;
}
