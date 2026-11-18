<script lang="ts">
  import { onMount } from "svelte";
  import { api, inTauri } from "../api";
  import ClientLock from "../app/ClientLock.svelte";
  import { handoff, takePrefill } from "./handoff.svelte";
  import NotSolved from "./NotSolved.svelte";
  import Practice from "./Practice.svelte";
  import Saved from "./Saved.svelte";
  import Setup from "./Setup.svelte";
  import { listen, solver, startSolve } from "./store.svelte";
  import Study from "./Study.svelte";
  import { usePageHead } from "../ui/page.svelte";

  onMount(() => {
    listen();
    api.solverPage(true);
    if (!inTauri) preview();
    return () => api.solverPage(false);
  });

  /**
   * Browser preview only, for pictures of every state: `hand=AsQd` picks your hand, `solve=1`
   * solves at once (`hold` stops half-way), `sview=result|practice|saved` opens a tab.
   */
  function preview() {
    const q = new URLSearchParams(window.location.search);
    const hand = q.get("hand")?.match(/[AKQJT2-9][cdhs]/g);
    if (hand?.length === 2) solver.hero = { player: solver.me ?? "oop", cards: hand };
    const view = q.get("sview");
    if (view === "result" || view === "practice" || view === "saved") solver.view = view;
    if (q.get("solve") && solver.status !== "running" && !solver.summary) startSolve();
  }

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
  const running = $derived(solver.status === "running");
  const view = $derived(solver.view);
  const TABS = [
    { id: "setup", label: "Aufbau" },
    { id: "result", label: "Durchspielen" },
    { id: "practice", label: "Üben" },
    { id: "saved", label: "Meine Lösungen" },
  ] as const;

  usePageHead(() => ({
    sub: "Zeigt für jede Hand den besten Zug – auch für deine. Wie gut sie gespielt ist, hängt davon ab, was du mit den anderen tust.",
    label: "Ansicht",
    value: view,
    tabs: TABS.map((t) => {
      const waits = (t.id === "result" || t.id === "practice") && !solved;
      return { id: t.id, label: t.label, note: waits ? (running ? "rechnet …" : "nach dem Lösen") : undefined };
    }),
    onchange: (id) => (solver.view = id as typeof solver.view),
  }));
</script>

<div class="page solver">
  <ClientLock feature="Der Solver">
    {#if solver.status === "running" && solver.replay?.notice}
      <p class="source" role="status">{solver.replay.notice}</p>
    {:else if solver.source && view === "setup"}
      <p class="source">Vorbelegt aus <b>{solver.source}</b>: Board, Pot und effektiver Stack stammen aus der Hand.</p>
    {/if}

    {#if (view === "result" || view === "practice") && !solved}
      <NotSolved what={view} onSetup={() => (solver.view = "setup")} />
    {:else if view === "result"}
      <Study onEdit={() => (solver.view = "setup")} onNew={discard} onPractice={() => (solver.view = "practice")} />
    {:else if view === "saved"}
      <Saved />
    {:else if view === "practice"}
      <Practice onStudy={() => (solver.view = "result")} />
    {:else}
      <Setup />
    {/if}
  </ClientLock>
</div>

<style>
  .solver {
    /* Table, answer and the 13×13 grid side by side use the room of wide screens. */
    max-width: 1680px;
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
