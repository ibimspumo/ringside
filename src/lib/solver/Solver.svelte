<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "../api";
  import ClientLock from "../app/ClientLock.svelte";
  import { handoff, takePrefill } from "./handoff.svelte";
  import Practice from "./Practice.svelte";
  import Saved from "./Saved.svelte";
  import Setup from "./Setup.svelte";
  import { listen, solver } from "./store.svelte";
  import Study from "./Study.svelte";
  import { usePageHead } from "../ui/page.svelte";

  onMount(() => {
    listen();
    api.solverPage(true);
    return () => api.solverPage(false);
  });

  // A spot sent from the replayer or the hand analysis replaces the current inputs.
  $effect(() => {
    const p = handoff.prefill;
    if (!p) return;
    handoff.prefill = null;
    takePrefill(p);
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

  usePageHead(() => ({
    sub: "Heads-up ab Flop, Turn oder River: Spot aufbauen, lösen, durchspielen und üben. Gerechnet wird nur auf diesem Rechner.",
    label: "Ansicht",
    value: view,
    tabs: TABS.map((t) => {
      const locked = t.id !== "setup" && t.id !== "saved" && !solved;
      return { id: t.id, label: t.label, disabled: locked, hint: locked ? "Erst lösen" : undefined };
    }),
    onchange: (id) => (solver.view = id as typeof solver.view),
  }));
</script>

<ClientLock feature="Der Solver">
  <div class="page solver">
    {#if solver.status === "running" && solver.replay?.notice}
      <p class="source" role="status">{solver.replay.notice}</p>
    {:else if solver.source && view === "setup"}
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
