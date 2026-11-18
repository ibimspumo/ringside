<script lang="ts">
  // Dev-only page (`?view=table-lab&n=9&hero=0&w=900`): the practice table with chips and an
  // action badge at every seat, so the end-to-end geometry check and screenshots see every
  // place filled. Not part of the built app.
  import SeatRing from "../learn/SeatRing.svelte";
  import Table from "../play/Table.svelte";
  import SolverTable from "../solver/SolverTable.svelte";
  import type { Choice, HandView, PlayEvent, Snapshot } from "../play/types";
  import { positionsFor } from "../solver/scenario";

  const q = new URLSearchParams(window.location.search);
  const n = Math.max(2, Math.min(9, Number(q.get("n") ?? 6)));
  const hero = Math.max(0, Math.min(n - 1, Number(q.get("hero") ?? 0)));
  const width = q.get("w");
  const showdown = q.get("end") === "1";
  const kind = q.get("kind") ?? "play";
  const order = positionsFor(n);
  const pair = { oop: order[n - 1], ip: order[n - 3] ?? order[0] };

  const KINDS: Choice["kind"][] = ["raise", "call", "all_in", "call", "raise", "call", "call", "raise", "call"];
  const AMOUNTS = [2.2, 2.2, 105.5, 24.5, 12.25, 12.25, 6.1, 18, 18];
  const bets = Array.from({ length: n }, (_, s) => AMOUNTS[s]);
  const snap = (k: number): Snapshot => ({
    street: "river",
    board: ["As", "Kd", "Th", "7c", "2s"],
    bets: bets.map((b, s) => (s <= k ? b : 0)),
    stacks: bets.map((b) => 120 - b),
    pot: 40 + bets.reduce((a, b) => a + b, 0),
    folded: new Array(n).fill(false),
    to_act: k + 1 < n ? k + 1 : null,
  });
  const events: PlayEvent[] = [
    { type: "board", street: "river", cards: ["2s"], snapshot: 0 },
    ...Array.from({ length: n }, (_, s): PlayEvent => ({ type: "action", street: "river", seat: s, choice: { kind: KINDS[s], to_bb: bets[s], add_bb: bets[s], pct_pot: null }, verdict: null, snapshot: s + 1 })),
  ];
  const view: HandView = {
    seed: 1,
    config: { mode: "full", players: n, ante: "big_blind", stack_bb: 120, mixed_stacks: false, hero_seat: hero },
    players: n,
    positions: positionsFor(n),
    stack_bb: 120,
    ante: new Array(n).fill(0),
    hero,
    hero_cards: ["Qs", "Qh"],
    board: ["As", "Kd", "Th", "7c", "2s"],
    events,
    snapshots: [snap(-1), ...Array.from({ length: n }, (_, s) => snap(s))],
    verdicts: [],
    decision: null,
    outcome: showdown
      ? { reason: "showdown", winners: [hero], net: new Array(n).fill(0), shown: Array.from({ length: n }, (_, s) => (s === hero ? null : ["9c", "9d"])), made: new Array(n).fill("Paar") }
      : null,
    preflop_source: "model",
    spot: null,
    waiting: false,
  };
</script>

<div class="lab" style:width={width ? `${width}px` : "100%"}>
  {#if kind === "solver"}
    <SolverTable
      players={n}
      positions={pair}
      anchor={order[hero]}
      board={["As", "Kd", "Th", "7c", "2s"]}
      pot={40}
      bets={[12.5, 37.5]}
      stacks={[80, 55]}
      acting="oop"
      views={{ oop: { cards: "hidden", tag: "Call" }, ip: { cards: "hidden", tag: "Raise" } }}
      street="River"
    />
  {:else if kind === "ring"}
    <SeatRing size={n} occupied={order.map((_, i) => i + 1)} button={n - 2} smallBlind={n - 1} bigBlind={n} />
  {:else}
    <Table {view} at={view.snapshots.length - 1} final={showdown} ms={0} />
  {/if}
</div>

<style>
  .lab {
    margin: 24px auto;
    padding: 0 16px;
  }
  :global(body) {
    background: var(--bg);
    color: var(--text);
  }
</style>
