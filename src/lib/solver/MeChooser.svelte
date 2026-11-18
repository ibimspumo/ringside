<script lang="ts">
  import SegmentedControl from "../ui/SegmentedControl.svelte";
  import { meSide, solver } from "./store.svelte";
  import type { SolverPlayer } from "./types";

  /** "Ich bin: BTN (IP) / BB (OOP)": which player you are, drawn at the bottom of the table. */
  let { positions }: { positions: { oop: string; ip: string } } = $props();

  const options = $derived([
    { value: "oop" as SolverPlayer, label: `${positions.oop} (OOP)` },
    { value: "ip" as SolverPlayer, label: `${positions.ip} (IP)` },
  ]);
</script>

<div class="me">
  {#if solver.hero}
    <span class="faint">Du bist <b>{positions[solver.hero.player]}</b> – aus deiner Hand.</span>
  {:else}
    <span class="faint">Ich bin</span>
    <SegmentedControl size="sm" label="Welcher Spieler du bist" {options} value={meSide()} onchange={(v: SolverPlayer) => (solver.me = v)} />
  {/if}
</div>

<style>
  .me {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-size: var(--fs-sm);
  }
  .me b {
    color: var(--text);
  }
</style>
