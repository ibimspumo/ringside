import { demoShow } from "../solver/demo";
import type { LibraryMode, LibraryOverview, LibraryPower, LibraryScenario, OpenedSpot, Shared, SpotSource } from "./types";

// Browser preview only: a few scenarios of the real catalog with made-up progress.
const FLOPS = `AsKhTd AsKh5d AsQhTd AsQh5d AsJh9d AsJh4d AsTh7d AsTh2d As9h4d As8h5d As7h5d As6h4d As5h2d KsQhTd KsQh5d
KsJh9d KsJh4d KsTh7d KsTh2d Ks9h4d Ks8h5d Ks7h3d Ks6h2d QsJhTd QsJh5d QsTh8d Qs9h7d Qs9h3d Qs8h4d Qs7h2d Qs6h3d JsTh9d
JsTh4d Js9h6d Js8h5d Js7h2d Ts9h8d Ts9h4d Ts8h3d Ts7h2d 9s8h6d 9s7h3d 9s5h2d 8s7h5d 8s6h2d 7s6h4d 7s4h2d 6s5h3d 5s4h2d
AsKsQh AsKs7h AsQs9h AsJs3h AsTs5h As9s2h As7s6h As5s4h AsKhQh AsKh8h AsQhJh AsJh6h AsTh2h As8h7h As6h3h KsQsTh
KsJs5h KsTs8h Ks9s3h Ks7s6h KsQhJh KsJh7h KsTh3h Ks8h4h QsJs9h QsTs4h Qs8s7h QsJhTh Qs9h6h Qs7h2h JsTs8h Js9s3h Js7s5h
JsTh7h Js8h4h Ts9s7h Ts8s2h Ts9h5h Ts6h4h 9s8s6h 9s7h5h 8s7s4h 8s6h3h 7s6s5h 7s5h2h 6s5s3h 5s4h3h AsKsTs AsJs4s
KsQs6s QsTs8s Js9s5s Ts8s3s 8s6s4s 7s5s2s AsAhKd AsAh7d AsAh2s KsKhAd KsKh9s KsKh4d QsQhJd QsQh5s JsJhTd JsJh3s TsTh8d
9s9hAd 8s8hKs 7s7hQd 6s6hTs 5s5h9d 4s4hJs 3s3h7d 2s2hAs AsAhAd`.split(/\s+/);

const SCENARIOS: Omit<LibraryScenario, "status" | "shared">[] = [
  { id: "srp-btn-bb-40", pot_type: "srp", oop: "BB", ip: "BTN", stack_bb: 40, pot: 5.9, stack: 36.8 },
  { id: "srp-co-bb-40", pot_type: "srp", oop: "BB", ip: "CO", stack_bb: 40, pot: 5.9, stack: 36.8 },
  { id: "srp-utg-bb-40", pot_type: "srp", oop: "BB", ip: "UTG", stack_bb: 40, pot: 5.9, stack: 36.8 },
  { id: "srp-sb-bb-40", pot_type: "srp", oop: "SB", ip: "BB", stack_bb: 40, pot: 7, stack: 36 },
  { id: "3bp-btn-bb-40", pot_type: "three_bet", oop: "BB", ip: "BTN", stack_bb: 40, pot: 18, stack: 30.75 },
  { id: "3bp-btn-sb-40", pot_type: "three_bet", oop: "SB", ip: "BTN", stack_bb: 40, pot: 18.5, stack: 31.75 },
  { id: "3bp-co-btn-40", pot_type: "three_bet", oop: "CO", ip: "BTN", stack_bb: 40, pot: 14.6, stack: 33.95 },
  { id: "srp-btn-bb-20", pot_type: "srp", oop: "BB", ip: "BTN", stack_bb: 20, pot: 5.5, stack: 17 },
  { id: "srp-co-bb-20", pot_type: "srp", oop: "BB", ip: "CO", stack_bb: 20, pot: 5.5, stack: 17 },
  { id: "srp-utg-bb-20", pot_type: "srp", oop: "BB", ip: "UTG", stack_bb: 20, pot: 5.5, stack: 17 },
  { id: "srp-sb-bb-20", pot_type: "srp", oop: "SB", ip: "BB", stack_bb: 20, pot: 6, stack: 16.5 },
  { id: "srp-btn-bb-75", pot_type: "srp", oop: "BB", ip: "BTN", stack_bb: 75, pot: 6.5, stack: 71.5 },
  { id: "3bp-btn-bb-75", pot_type: "three_bet", oop: "BB", ip: "BTN", stack_bb: 75, pot: 21.5, stack: 64 },
];

function statuses(i: number): (SpotSource | null)[] {
  return FLOPS.map((_, f) => {
    if (i === 0) return f < 104 ? "cloud" : f < 112 ? "local" : null;
    if (i === 1) return f < 23 ? "local" : null;
    if (i === 4) return f % 3 === 0 ? "cloud" : null;
    return null;
  });
}

const NOW = Math.floor(Date.now() / 1000);
const MIXED: (Shared | null)[] = [
  { state: "verified", at: NOW - 86_400 * 3 },
  { state: "verified", at: NOW - 86_400 * 2 },
  { state: "submitted", at: NOW - 1_800 },
  { state: "submitted", at: NOW - 600 },
  { state: "rejected", at: NOW - 86_400, note: "exploitability 1.84% of the pot is above the 1% limit" },
  { state: "failed", at: NOW - 300, note: "network error: timed out" },
  null,
  { state: "verified", at: NOW - 86_400 * 4 },
];

function shared(i: number, status: (SpotSource | null)[]): (Shared | null)[] {
  return status.map((st, f) => {
    if (st !== "local") return null;
    if (i === 0) return MIXED[(f - 104) % MIXED.length];
    return f < 18 ? { state: "verified", at: NOW - 86_400 * 5 } : { state: "submitted", at: NOW - 3_600 };
  });
}

let mode: LibraryMode = "idle";
let power: LibraryPower = "normal";
let running = false;
let halted = false;

export function overview(): LibraryOverview {
  return {
    catalog: 1,
    target_pct: 0.5,
    flops: FLOPS,
    flop_weights: FLOPS.map(() => 22100 / FLOPS.length),
    scenarios: SCENARIOS.map((s, i) => {
      const status = statuses(i);
      return { ...s, status, shared: shared(i, status) };
    }),
    disk_bytes: 18_400_000,
    mode,
    power,
    cores: 16,
    share: true,
    worker: {
      state: running ? "solving" : mode === "off" ? "off" : "waiting",
      reason: running ? null : halted ? "halted" : mode === "off" ? "off" : "in_use",
      manual: running,
      scenario: running ? "srp-co-bb-40" : null,
      flop: running ? FLOPS[23] : null,
      iteration: running ? 60 : 0,
      exploitability_pct: running ? 0.81 : 0,
      progress: running ? 0.63 : null,
      elapsed_ms: 9_400,
      threads: { gentle: 4, normal: 8, full: 15 }[power],
      solved: 23,
      fetched: 104,
      shared: 21,
      last_error: null,
    },
  };
}

export function setMode(m: LibraryMode) {
  mode = m;
  halted = false;
}

export function setPower(p: LibraryPower) {
  power = p;
}

export function run(on: boolean) {
  running = on;
  halted = !on;
}

export function open(scenario: string, flop: string): OpenedSpot {
  const s = SCENARIOS.find((x) => x.id === scenario) ?? SCENARIOS[0];
  const size = (bet: number[], raise: number[]) => ({ bet, raise });
  const opened: OpenedSpot = {
    spot: {
      oop_range: "22-99,A2s-AJs,K2s-KJs,Q2s-QJs,J4s+,T6s+,96s+,85s+,74s+,63s+,53s+,43s,A2o-AJo,K8o-KJo,Q9o+,J9o+,T8o+,98o,87o",
      ip_range: "22+,A2s+,K2s+,Q4s+,J6s+,T6s+,96s+,85s+,75s+,64s+,54s,A2o+,K8o+,Q9o+,J9o+,T8o+,98o",
      board: flop,
      pot: s.pot,
      stack: s.stack,
      sizes: [size([33], [60]), size([75], []), size([75], [])],
      allin_threshold: 150,
    },
    summary: {
      iterations: 80,
      exploitability: 0.024,
      exploitability_pct: 0.41,
      converged: true,
      elapsed_ms: 0,
      memory_bytes: 104_000,
      compressed: false,
      threads: 0,
    },
    oop: s.oop,
    ip: s.ip,
    oop_note: "Call aus BB gegen das Open",
    ip_note: "Open-Raise aus BTN auf 2,2 BB",
    source: "cloud",
  };
  demoShow(opened.spot);
  return opened;
}
