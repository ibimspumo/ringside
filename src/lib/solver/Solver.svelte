<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "../api";
  import ClientLock from "../app/ClientLock.svelte";
  import { handoff } from "./handoff.svelte";
  import Practice from "./Practice.svelte";
  import Saved from "./Saved.svelte";
  import Setup from "./Setup.svelte";
  import { applyPrefill, listen, solver } from "./store.svelte";
  import Study from "./Study.svelte";

  onMount(listen);

  // A spot sent from the replayer replaces the current inputs.
  $effect(() => {
    const p = handoff.prefill;
    if (!p) return;
    applyPrefill(p);
    handoff.prefill = null;
  });

  function discard() {
    api.solverClear();
    solver.summary = null;
    solver.solved = null;
    solver.status = "idle";
    solver.view = "setup";
  }

  const solved = $derived(!!solver.summary && solver.status !== "running");
  const view = $derived(solver.view === "saved" ? "saved" : solved ? solver.view : "setup");
  const TABS = [
    { id: "setup", label: "Aufbau" },
    { id: "result", label: "Durchspielen" },
    { id: "practice", label: "Üben" },
    { id: "saved", label: "Meine Lösungen" },
  ] as const;
</script>

<ClientLock feature="Der Solver">
  <div class="page solver">
    <div class="page-head">
      <div>
        <h1>Spot-Werkstatt</h1>
        <p>Heads-up ab Flop, Turn oder River: Spot am Tisch aufbauen, lösen, Zug um Zug durchspielen und üben.</p>
      </div>
      <div class="seg tabs" role="tablist" aria-label="Ansicht">
        {#each TABS as t (t.id)}
          <button
            role="tab"
            aria-selected={view === t.id}
            class:on={view === t.id}
            disabled={t.id !== "setup" && t.id !== "saved" && !solved}
            title={t.id !== "setup" && t.id !== "saved" && !solved ? "Erst lösen" : undefined}
            onclick={() => (solver.view = t.id)}>{t.label}</button
          >
        {/each}
      </div>
    </div>
    {#if solver.source && view === "setup"}
      <p class="source">Vorbelegt aus <b>{solver.source}</b>: Board, Pot und effektiver Stack stammen aus der Hand.</p>
    {/if}

    {#if view === "result"}
      <Study onEdit={() => (solver.view = "setup")} onNew={discard} onPractice={() => (solver.view = "practice")} />
    {:else if view === "saved"}
      <Saved />
    {:else if view === "practice"}
      <Practice onStudy={() => (solver.view = "result")} />
    {:else}
      <Setup />
    {/if}
  </div>
</ClientLock>

<style>
  .solver {
    container: solverpage / inline-size;
  }
  .tabs {
    align-self: center;
  }
  .tabs button {
    white-space: nowrap;
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
</style>
