<script lang="ts">
  import { api } from "../api";
  import { libraryApi } from "../library/api";
  import { labelOf } from "../library/labels";
  import { openInSolver } from "../library/open";
  import type { LibraryScenario } from "../library/types";
  import Button from "../ui/Button.svelte";
  import Card from "../ui/Card.svelte";
  import MiniCard from "../ui/MiniCard.svelte";
  import { solveSeconds } from "./actions";
  import { ago, duration } from "../ui/format";
  import { RANKS } from "./range";
  import { openSaved, solutionsApi, type SavedSolution } from "./solutions";
  import SolveProgress from "./SolveProgress.svelte";
  import { meSide, solver, startSolve } from "./store.svelte";
  import type { SolverEstimate } from "./types";

  /**
   * "Durchspielen" and "Üben" before anything is solved: what waits there, one way to get
   * there (solve the spot), and solutions that open at once without solving.
   */
  let { what, onSetup }: { what: "result" | "practice"; onSetup: () => void } = $props();

  const cardsOf = (b: string) => b.replace(/[^AKQJT2-9cdhs]/gi, "").match(/.{2}/g) ?? [];
  const board = $derived(cardsOf(solver.spot.board));
  const hand = $derived(solver.hero?.cards ?? null);
  const running = $derived(solver.status === "running");
  const { oop, ip } = $derived(solver.positions);

  let estimate = $state<SolverEstimate | null>(null);
  $effect(() => {
    const spot = $state.snapshot(solver.spot);
    if (cardsOf(spot.board).length < 3) return void (estimate = null);
    let live = true;
    api
      .solverEstimate(spot, solver.settings.threads)
      .then((e) => live && (estimate = e))
      .catch(() => live && (estimate = null));
    return () => (live = false);
  });
  const seconds = $derived(estimate ? solveSeconds(estimate.seconds, solver.settings.target_pct) : 0);

  // Solutions of the same two positions that open without solving: saved ones (the same board
  // first), then the library flop nearest to this board.
  let saved = $state<SavedSolution[]>([]);
  let library = $state<{ scenario: LibraryScenario; flop: string } | null>(null);
  $effect(() => {
    const key = `${oop}-${ip}`;
    const want = board.join("");
    let live = true;
    solutionsApi
      .list()
      .then((v) => {
        if (!live) return;
        const same = v.items.filter((s) => `${s.oop}-${s.ip}` === key);
        saved = same.sort((a, b) => Number(b.board === want) - Number(a.board === want) || b.created_at - a.created_at).slice(0, 2);
      })
      .catch(() => (saved = []));
    libraryApi
      .overview()
      .then((o) => {
        if (!live) return;
        const pot = { srp: "srp", "3bet": "three_bet" }[solver.scenario.pot as string];
        const fits = o.scenarios.filter((s) => s.oop === oop && s.ip === ip && s.pot_type === pot);
        const scenario = fits.sort((a, b) => Math.abs(a.stack_bb - solver.scenario.stack) - Math.abs(b.stack_bb - solver.scenario.stack))[0];
        if (!scenario) return void (library = null);
        const ready = o.flops.map((f, i) => ({ f, ok: scenario.status[i] !== null })).filter((x) => x.ok);
        const best = ready.sort((a, b) => flopDistance(a.f, board) - flopDistance(b.f, board))[0];
        library = best ? { scenario, flop: best.f } : null;
      })
      .catch(() => (library = null));
    return () => (live = false);
  });

  /** How far two flops are apart by their ranks, high card first; any flop when none is set. */
  function flopDistance(flop: string, cards: string[]): number {
    if (cards.length < 3) return 0;
    const ranks = (cs: string[]) => cs.map((c) => RANKS.indexOf(c[0])).sort((a, b) => a - b);
    const a = ranks(cardsOf(flop));
    const b = ranks(cards.slice(0, 3));
    return a.reduce((s, r, i) => s + Math.abs(r - b[i]) * (3 - i), 0);
  }

  let opening = $state<string | null>(null);
  let problem = $state<string | null>(null);
  async function open(id: string, go: () => Promise<void>) {
    opening = id;
    problem = null;
    try {
      await go();
      solver.view = what;
    } catch (e) {
      problem = String(e);
    } finally {
      opening = null;
    }
  }


  const title = $derived(
    running ? "Der Spot wird gerade gelöst" : what === "result" ? "Durchspielen startet nach dem Lösen" : "Üben startet nach dem Lösen",
  );
</script>

<div class="wait">
  <Card pad="lg">
    <div class="main">
      <p class="spot num">
        <b>{oop} gegen {ip}</b>
        {#if board.length}<span class="cards">{#each board as c (c)}<MiniCard card={c} />{/each}</span>{:else}<span class="faint">noch kein Board</span>{/if}
        {#if hand}<span class="mine">Du ({solver.positions[meSide()]}) <span class="cards">{#each hand as c (c)}<MiniCard card={c} />{/each}</span></span>{/if}
      </p>
      <h2>{title}</h2>
      <p class="say">
        {#if running}
          Sobald die Lösung steht, öffnet sich hier {what === "result"
            ? hand
              ? "die Antwort für deine Hand – und der Spot zum Durchspielen"
              : "der Spot zum Durchspielen"
            : hand
              ? "das Üben mit deiner Hand"
              : "das Üben gegen die Lösung"}. Du kannst die Seite solange verlassen.
        {:else if what === "result"}
          Hier gehst du den Spot Zug für Zug durch. Bei jeder Entscheidung steht oben, was die Lösung
          {#if hand}mit deiner Hand macht{:else}macht{/if} – darunter, wie sie jede andere Hand spielt und warum.
        {:else}
          Hier spielst du {hand ? "deine Hand" : "Hände aus deiner Range"} gegen die Lösung. Nach jeder Entscheidung bekommst du eine Note und siehst, was
          die beste Wahl gewesen wäre.
        {/if}
      </p>

      {#if running}
        <div class="go"><SolveProgress total={seconds || null} /></div>
      {:else if board.length < 3}
        <div class="go">
          <Button variant="primary" onclick={onSetup}>Zum Aufbau</Button>
          <span class="faint">Wähle dort zuerst mindestens drei Boardkarten.</span>
        </div>
      {:else if estimate && !estimate.fits}
        <div class="go">
          <Button variant="primary" onclick={onSetup}>Zum Aufbau</Button>
          <span class="neg">Der Spot ist zu groß für diesen Rechner. Weniger Einsatzgrößen oder ein späteres Board helfen.</span>
        </div>
      {:else}
        <div class="go">
          <Button variant="primary" size="lg" disabled={!estimate} onclick={() => startSolve()}>
            Spot lösen{estimate ? ` · ${duration(seconds, { about: true })}` : ""}
          </Button>
          <Button variant="quiet" onclick={onSetup}>Spot erst ändern</Button>
        </div>
        {#if solver.error}<p class="neg small">{solver.error}</p>{/if}
      {/if}
    </div>

    {#if saved.length || library}
      <div class="ready">
        <h3>Sofort öffnen, ohne zu rechnen</h3>
        <ul>
          {#each saved as s (s.id)}
            {@const exact = s.board === board.join("")}
            <li>
              <span class="cards">{#each cardsOf(s.board) as c (c)}<MiniCard card={c} />{/each}</span>
              <span class="what">
                <b>{exact ? "Genau dieser Spot" : s.title}</b>
                <span class="faint">Meine Lösungen · {ago(Date.now() / 1000 - s.created_at)}</span>
              </span>
              <Button size="sm" disabled={opening !== null} onclick={() => open(`s${s.id}`, () => openSaved(s.id))}>
                {opening === `s${s.id}` ? "Öffnet …" : "Öffnen"}
              </Button>
            </li>
          {/each}
          {#if library}
            {@const l = library}
            <li>
              <span class="cards">{#each cardsOf(l.flop) as c (c)}<MiniCard card={c} />{/each}</span>
              <span class="what">
                <b>{l.flop === board.join("") ? "Genau dieser Flop" : "Ähnlicher Flop"}</b>
                <span class="faint">Bibliothek · {labelOf(l.scenario)}</span>
              </span>
              <Button size="sm" disabled={opening !== null} onclick={() => open("lib", () => openInSolver(l.scenario, l.flop))}>
                {opening === "lib" ? "Öffnet …" : "Öffnen"}
              </Button>
            </li>
          {/if}
        </ul>
        {#if problem}<p class="neg small">{problem}</p>{/if}
      </div>
    {/if}
  </Card>
</div>

<style>
  .wait {
    max-width: none;
    margin-top: var(--sp-2);
  }
  .main {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }
  .spot {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2) var(--sp-3);
    margin: 0;
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .spot b {
    color: var(--text);
    font-weight: var(--fw-semibold);
  }
  .cards {
    display: inline-flex;
    gap: 2px;
  }
  .mine {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-1);
    padding-left: var(--sp-3);
    border-left: 1px solid var(--line-2);
  }
  h2 {
    margin: var(--sp-2) 0 0;
    font-size: var(--fs-xl);
    font-weight: var(--fw-semibold);
    color: var(--text);
  }
  .say {
    max-width: 62ch;
    margin: 0;
    font-size: var(--fs-base);
    line-height: 1.5;
    color: var(--text-2);
  }
  .go {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-3);
    margin-top: var(--sp-3);
    font-size: var(--fs-md);
  }
  .small {
    margin: 0;
    font-size: var(--fs-sm);
  }
  .ready {
    margin-top: var(--sp-6);
    padding-top: var(--sp-4);
    border-top: 1px solid var(--line);
  }
  h3 {
    margin: 0 0 var(--sp-2);
    font-size: var(--fs-sm);
    font-weight: var(--fw-semibold);
    color: var(--text-2);
  }
  ul {
    display: flex;
    flex-direction: column;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  li {
    display: grid;
    grid-template-columns: 188px minmax(0, 1fr) auto;
    align-items: center;
    gap: var(--sp-3);
    padding: var(--sp-2) 0;
  }
  li {
    border-top: 1px solid var(--line);
  }
  .what {
    display: flex;
    flex-direction: column;
    min-width: 0;
    font-size: var(--fs-md);
  }
  .what b {
    font-weight: var(--fw-medium);
    color: var(--text);
  }
  .what span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--fs-sm);
  }
</style>
