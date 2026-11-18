<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "../api";
  import type { Anchor } from "../app/format";
  import { bb, bytes, duration, solveSeconds } from "./actions";
  import DeckPopover from "./DeckPopover.svelte";
  import { DECK, dealFrom } from "./practice";
  import Button from "../ui/Button.svelte";
  import MiniCard from "../ui/MiniCard.svelte";
  import RangeEditor from "./RangeEditor.svelte";
  import { comboClass, parseRange, rangeCombos } from "./range";
  import { ANTES, POT_TYPES, derive, oopIp, originText, potTypeProblem, template, type PotType, type Scenario } from "./scenario";
  import SizeChooser from "./SizeChooser.svelte";
  import SolveProgress from "./SolveProgress.svelte";
  import SolverTable from "./SolverTable.svelte";
  import MeChooser from "./MeChooser.svelte";
  import { meSide, solver, startSolve } from "./store.svelte";
  import Term from "./Term.svelte";
  import type { SolverEstimate } from "./types";

  /** Most sizes per street the solver takes, `ringside_solver::MAX_SIZES`. */
  const MAX_SIZES = 4;
  const TABLES = [6, 8, 9];
  /** Target exploitability in % of the pot. */
  const PRECISION = [
    { pct: 2, label: "Schnell", hint: "Für einen ersten Blick: Die großen Linien stimmen, Feinheiten nicht." },
    { pct: 0.5, label: "Normal", hint: "Zum Lernen genau genug: Ein perfekter Gegner gewönne höchstens 0,5 % des Pots mehr." },
    { pct: 0.2, label: "Genau", hint: "Für knappe Entscheidungen; dauert etwa doppelt so lange." },
  ];

  const board = $derived(cardsOf(solver.spot.board));
  function cardsOf(text: string): string[] {
    return text.replace(/[^AKQJT2-9cdhs]/gi, "").match(/.{2}/g) ?? [];
  }
  const firstStreet = $derived(Math.max(0, Math.min(2, board.length - 3)));

  /** Edits after a solve keep the old result reachable until the next start. */
  function edited() {
    if (solver.status !== "running") solver.error = null;
  }

  // Scenario: seats, pot type, depth → ranges, pot and stack from the preflop model.
  let deriving = $state(false);
  let deriveNote = $state<string | null>(null);
  let deriveDetail = $state("");
  let token = 0;

  async function apply(next: Scenario) {
    solver.scenario = next;
    const problem = potTypeProblem(next);
    solver.positions = oopIp(next.players, next.seats);
    if (problem) {
      deriveNote = problem;
      return;
    }
    const mine = ++token;
    deriving = true;
    let d;
    let origin: string;
    try {
      d = await derive(next);
      origin = originText(next, d.modelStack);
      deriveNote = null;
    } catch (e) {
      d = template(next);
      origin = "Vorlage";
      deriveNote = "Preflop-Modell nicht verfügbar – die Ranges sind einfache Vorlagen.";
      deriveDetail = e instanceof Error ? e.message : String(e);
    }
    if (mine !== token) return;
    deriving = false;
    solver.positions = { oop: d.oop, ip: d.ip };
    solver.spot = { ...solver.spot, oop_range: d.oopRange, ip_range: d.ipRange, pot: d.pot, stack: d.stack };
    solver.notes = { oop: d.oopNote, ip: d.ipNote };
    solver.labels = d.labels;
    solver.origin = origin;
    solver.custom = { oop: false, ip: false };
    // A hand handed over belongs to that hand's spot; your own pick stays with you.
    if (solver.source) solver.hero = null;
    solver.source = "";
    edited();
  }

  const change = (patch: Partial<Scenario>) => apply({ ...$state.snapshot(solver.scenario), ...patch } as Scenario);

  function pickSeat(pos: string) {
    const [a, b] = solver.scenario.seats;
    if (pos === a || pos === b) return;
    // The seat picked longest ago gives way, so two clicks always set both players.
    change({ seats: [b, pos] });
    // The first seat you pick is you, until you choose otherwise.
    if (solver.me === null) solver.me = solver.positions.oop === pos ? "oop" : "ip";
  }

  function setTable(players: number) {
    const known = (p: string) => (players === 6 ? !["UTG", "UTG+1", "UTG+2"].includes(p) : players === 8 ? p !== "UTG+2" : true);
    const seats = solver.scenario.seats.every(known) ? solver.scenario.seats : (["BTN", "BB"] as [string, string]);
    change({ players, seats });
  }

  let stackText = $state(solver.scenario.stack);
  $effect(() => {
    stackText = solver.scenario.stack;
  });
  function commitStack(v: number) {
    if (!Number.isFinite(v)) return;
    change({ stack: Math.min(200, Math.max(5, Math.round(v))) });
  }

  onMount(() => {
    // A fresh page starts from the model; a spot handed over or edited keeps its ranges.
    if (!solver.origin && !solver.source && !solver.summary && !solver.custom.oop && !solver.custom.ip) change({});
  });

  // Board at the table.
  let picker = $state<{ slot: number; anchor: Anchor } | null>(null);

  function openSlot(i: number, el: HTMLElement) {
    handPicker = null;
    const r = el.getBoundingClientRect();
    picker = { slot: Math.min(i, board.length), anchor: { left: r.left, top: r.top, right: r.right, bottom: r.bottom } };
  }

  function setBoard(cards: string[]) {
    solver.spot.board = cards.join("");
    edited();
  }

  function pickCard(card: string) {
    if (!picker) return;
    const next = [...board];
    next[picker.slot] = card;
    setBoard(next.filter(Boolean));
    // Filling the flop moves on to the next slot, so three clicks deal it.
    picker = picker.slot < 2 && board.length < 3 ? { ...picker, slot: picker.slot + 1 } : null;
  }

  function randomCard(except: string[]): string {
    const left = DECK.filter((c) => !except.includes(c));
    return left[Math.floor(Math.random() * left.length)];
  }

  function randomFlop() {
    const out: string[] = [];
    const dead = mine();
    while (out.length < 3) out.push(randomCard([...out, ...dead]));
    setBoard(out);
    picker = null;
  }

  // Your hand, picked at your seat like the board; optional.
  let draft = $state<string[]>([]);
  let handPicker = $state<{ slot: number; anchor: Anchor } | null>(null);
  const mine = () => solver.hero?.cards ?? draft;
  const shownHand = $derived(solver.hero?.cards ?? draft);

  function openHand(slot: number, el: HTMLElement) {
    picker = null;
    const r = el.getBoundingClientRect();
    handPicker = { slot: Math.min(slot, shownHand.length), anchor: { left: r.left, top: r.top, right: r.right, bottom: r.bottom } };
  }

  function setHand(cards: string[]) {
    if (cards.length === 2) {
      solver.hero = { player: meSide(), cards };
      draft = [];
    } else {
      solver.hero = null;
      draft = cards;
    }
    edited();
  }

  function pickHandCard(card: string) {
    if (!handPicker) return;
    const next = [...shownHand];
    next[handPicker.slot] = card;
    setHand(next.filter(Boolean));
    handPicker = handPicker.slot === 0 && next.filter(Boolean).length < 2 ? { ...handPicker, slot: 1 } : null;
  }

  function randomHand() {
    const side = meSide();
    const cards = dealFrom(side === "oop" ? solver.spot.oop_range : solver.spot.ip_range, board);
    if (cards) setHand(cards);
    handPicker = null;
  }

  /** Whether your hand is in your range at all; otherwise the solution has nothing for it. */
  const handInRange = $derived.by(() => {
    const cards = solver.hero?.cards;
    if (!cards) return true;
    const w = parseRange(meSide() === "oop" ? solver.spot.oop_range : solver.spot.ip_range);
    return !w || w[comboClass(cards[0], cards[1])] > 0;
  });

  // Ranges.
  let editing = $state(false);
  const share = (text: string) => {
    const w = parseRange(text);
    return w ? rangeCombos(w) : 0;
  };
  const originOf = () => (solver.source ? `aus ${solver.source}` : solver.origin || "eigene Eingabe");

  // Estimate.
  let estimate = $state<SolverEstimate | null>(null);
  let estimateError = $state<string | null>(null);
  $effect(() => {
    const spot = $state.snapshot(solver.spot);
    const threads = solver.settings.threads;
    if (cardsOf(spot.board).length < 3) {
      estimate = null;
      estimateError = "Wähle mindestens drei Boardkarten – Klick auf die Plätze in der Tischmitte.";
      return;
    }
    let live = true;
    const t = setTimeout(() => {
      api
        .solverEstimate(spot, threads)
        .then((e) => {
          if (!live) return;
          estimate = e;
          estimateError = null;
        })
        .catch((e) => {
          if (!live) return;
          estimate = null;
          estimateError = String(e);
        });
    }, 350);
    return () => {
      live = false;
      clearTimeout(t);
    };
  });

  const running = $derived(solver.status === "running");
  const canStart = $derived(!!estimate?.fits && !running && !deriving && board.length >= 3);
  const spr = $derived(solver.spot.stack / Math.max(solver.spot.pot, 0.01));
  const seatViews = $derived.by(() => {
    const me = meSide();
    const view = (p: "oop" | "ip") => ({ cards: p === me && shownHand.length ? shownHand : ("hidden" as const), tag: solver.labels[solver.positions[p]] });
    return { oop: view("oop"), ip: view("ip") };
  });
  const seconds = $derived(estimate ? solveSeconds(estimate.seconds, solver.settings.target_pct) : 0);
</script>

<div class="setup">
  <div class="stagecol">
    <SolverTable
      players={solver.scenario.players}
      positions={solver.positions}
      anchor={solver.positions[meSide()]}
      {board}
      slotsClickable
      nextSlot={board.length < 3 ? board.length : -1}
      onSlot={openSlot}
      pickCards={solver.source && solver.hero ? null : meSide()}
      onCards={openHand}
      pot={solver.spot.pot}
      stacks={[solver.spot.stack, solver.spot.stack]}
      views={seatViews}
      seatsClickable
      onSeat={pickSeat}
      street={board.length >= 3 ? ["Flop", "Turn", "River"][board.length - 3] : "Board wählen"}
    />
    <div class="under">
      <MeChooser positions={solver.positions} />
      <span class="handnote">
        {#if solver.hero}
          Deine Hand <span class="cards">{#each solver.hero.cards as c (c)}<MiniCard card={c} />{/each}</span>
          {#if !solver.source}<Button variant="quiet" size="sm" onclick={() => setHand([])}>Entfernen</Button>{/if}
        {:else}
          <span>Deine Hand (optional): Klick auf die Karten an deinem Platz.</span>
        {/if}
      </span>
      <span class="spacer"></span>
      {#if board.length}<Button variant="quiet" size="sm" onclick={() => setBoard([])}>Board leeren</Button>{/if}
      <Button size="sm" onclick={randomFlop}>Zufälliger Flop</Button>
    </div>
    {#if !handInRange && solver.hero}
      <p class="warn small">
        {solver.positions[meSide()]} spielt <span class="cards">{#each solver.hero.cards as c (c)}<MiniCard card={c} />{/each}</span> laut Range hier nie – die Lösung hat dafür keine Strategie. Wähl eine andere Hand oder pass die Range an.
      </p>
    {/if}
    <section class="panel go" aria-live="polite">
      {#if running}
        <SolveProgress total={seconds || null} />
      {:else}
        <div class="est">
          <span class="spot num"
            ><b>{solver.positions.oop}</b> gegen <b>{solver.positions.ip}</b>
            {#if board.length}<span class="cards">{#each board as c (c)}<MiniCard card={c} />{/each}</span>{:else}· kein Board{/if}
            {#if solver.hero}· Du <span class="cards">{#each solver.hero.cards as c (c)}<MiniCard card={c} />{/each}</span>{/if}</span
          >
          {#if estimate}
            <span>Rechnet <b class="num">ca. {duration(seconds)}</b></span>
            <span title={estimate.compressed ? "Passt nur mit platzsparendem 16-Bit-Speicher" : undefined}
              >braucht <b class="num" class:neg={!estimate.fits}>{bytes(estimate.memory_bytes)}</b> Arbeitsspeicher</span
            >
            {#if !estimate.fits}
              <span class="neg">Zu groß für diesen Rechner (erlaubt sind {bytes(estimate.memory_limit)}). Weniger Einsatzgrößen oder ein späteres Board helfen.</span>
            {/if}
          {:else if estimateError}
            <span class="neg">{estimateError}</span>
          {:else}
            <span class="faint">Schätze …</span>
          {/if}
          {#if solver.error}<span class="neg">{solver.error}</span>{/if}
          {#if solver.status === "cancelled" && !solver.error}<span class="dim">Abgebrochen.</span>{/if}
        </div>
        <button class="btn primary" disabled={!canStart} onclick={() => startSolve()}>Lösen</button>
      {/if}
    </section>
  </div>

  <aside class="panel scen">
    <section>
      <h3>An welchem Tisch?</h3>
      <div class="pair">
        <div class="seg" role="group" aria-label="Spieler am Tisch">
          {#each TABLES as n (n)}
            <button class:on={solver.scenario.players === n} onclick={() => setTable(n)}>{n}-max</button>
          {/each}
        </div>
        <div class="seg" role="group" aria-label="Ante">
          {#each ANTES as a (a.id)}
            <button class:on={solver.scenario.ante === a.id} onclick={() => change({ ante: a.id })}>{a.label}</button>
          {/each}
        </div>
      </div>
    </section>

    <section>
      <h3>Wie kam der Pot zustande?</h3>
      <div class="seg wide" role="group" aria-label="Pot-Typ">
        {#each POT_TYPES as t (t.id)}
          <button class:on={solver.scenario.pot === t.id} title={t.hint} onclick={() => change({ pot: t.id as PotType })}>{t.label}</button>
        {/each}
      </div>
      <p class="faint small">{POT_TYPES.find((t) => t.id === solver.scenario.pot)?.hint}</p>
    </section>

    <section>
      <h3>Wie tief sind die Stacks?</h3>
      <div class="depth">
        <input type="range" min="10" max="100" step="1" bind:value={stackText} onchange={() => commitStack(stackText)} aria-label="Stacktiefe in BB" />
        <label class="unitfield">
          <input class="field num" type="number" min="5" max="200" bind:value={stackText} onchange={() => commitStack(stackText)} aria-label="Stacktiefe" />
          <span>BB</span>
        </label>
      </div>
      <div class="facts num">
        <span>Pot <b>{bb(solver.spot.pot, 2)}</b></span>
        <span><Term term="effective">effektiv</Term> <b>{bb(solver.spot.stack, 2)}</b></span>
        <span><Term term="spr">SPR</Term> <b>{bb(spr, 1)}</b></span>
      </div>
    </section>

    <section>
      <div class="h3row">
        <h3>Welche Hände haben beide?</h3>
        {#if deriving}<span class="faint small">lädt …</span>{/if}
      </div>
      {#each ["ip", "oop"] as const as p (p)}
        {@const text = p === "oop" ? solver.spot.oop_range : solver.spot.ip_range}
        <div class="who">
          <b class="num">{solver.positions[p]}</b>
          <span class="dim">{solver.notes[p] || "Range"}</span>
          <span class="num faint" title="Anteil aller Starthände">{Math.round((share(text) / 1326) * 1000) / 10} %</span>
        </div>
      {/each}
      <p class="origin" class:custom={solver.custom.oop || solver.custom.ip}>
        {#if solver.custom.oop || solver.custom.ip}
          {solver.custom.oop && solver.custom.ip ? "Beide Ranges angepasst" : `${solver.positions[solver.custom.oop ? "oop" : "ip"]} angepasst`}
        {:else}
          <span class="faint">Quelle:</span> {originOf()}
        {/if}
      </p>
      {#if deriveNote}<p class="warn small" title={deriveDetail || undefined}>{deriveNote}</p>{/if}
      <button class="btn ghost full" aria-expanded={editing} onclick={() => (editing = !editing)}>
        {editing ? "Ranges schließen" : "Ranges anpassen"}
      </button>
    </section>
  </aside>
</div>

{#if editing}
  <div class="ranges">
    <RangeEditor
      title="OOP"
      position={solver.positions.oop}
      lockPosition
      bind:value={solver.spot.oop_range}
      note={solver.source ? solver.notes.oop : ""}
      onEdit={() => ((solver.custom.oop = true), edited())}
    />
    <RangeEditor
      title="IP"
      position={solver.positions.ip}
      lockPosition
      bind:value={solver.spot.ip_range}
      note={solver.source ? solver.notes.ip : ""}
      onEdit={() => ((solver.custom.ip = true), edited())}
    />
  </div>
{/if}

<details class="panel expert">
  <summary>
    <b>Für Fortgeschrittene: Einsatzgrößen und Genauigkeit</b>
    <span class="faint">Die Voreinstellung passt für die meisten Spots.</span>
  </summary>
  <div class="lower">
    <section class="box">
      <h3>Einsatzgrößen</h3>
      <SizeChooser bind:sizes={solver.spot.sizes} bind:allin={solver.spot.allin_threshold} {firstStreet} max={MAX_SIZES} onEdit={edited} />
    </section>
    <section class="box">
      <h3>Genauigkeit</h3>
      <div class="acc">
        <div class="seg" role="group" aria-label="Genauigkeit">
          {#each PRECISION as p (p.pct)}
            <button class:on={solver.settings.target_pct === p.pct} title={p.hint} onclick={() => (solver.settings.target_pct = p.pct)}>{p.label}</button>
          {/each}
        </div>
        <p class="faint small">{PRECISION.find((p) => p.pct === solver.settings.target_pct)?.hint ?? "Eigener Wert."}</p>
        <details>
          <summary>Rechner und eigener Wert</summary>
          <div class="acc2">
            <label>
              <span>Ziel in % des Pots</span>
              <input class="field num" type="number" min="0.05" max="20" step="0.1" bind:value={solver.settings.target_pct} />
            </label>
            <label>
              <span>Threads</span>
              <input class="field num" type="number" min="1" max="256" step="1" bind:value={solver.settings.threads} />
            </label>
            <label>
              <span>Max. Iterationen</span>
              <input class="field num" type="number" min="10" step="100" bind:value={solver.settings.max_iterations} />
            </label>
          </div>
        </details>
      </div>
    </section>
  </div>
</details>


{#if picker}
  <DeckPopover
    anchor={picker.anchor}
    title={picker.slot < 3 ? `Flop, Karte ${picker.slot + 1}` : picker.slot === 3 ? "Turn" : "River"}
    available={DECK.filter((c) => (!board.includes(c) || c === board[picker!.slot]) && !shownHand.includes(c))}
    current={board[picker.slot] ?? null}
    onPick={pickCard}
    onRandom={picker.slot < 3 && board.length < 3 ? randomFlop : () => pickCard(randomCard([...board, ...shownHand]))}
    onClear={() => {
      setBoard(board.slice(0, picker!.slot));
      picker = null;
    }}
    onClose={() => (picker = null)}
  />
{/if}

{#if handPicker}
  <DeckPopover
    anchor={handPicker.anchor}
    side="top"
    title={`Deine Hand, Karte ${handPicker.slot + 1}`}
    available={DECK.filter((c) => !board.includes(c) && (!shownHand.includes(c) || c === shownHand[handPicker!.slot]))}
    current={shownHand[handPicker.slot] ?? null}
    onPick={pickHandCard}
    onRandom={randomHand}
    onClear={() => {
      setHand([]);
      handPicker = null;
    }}
    onClose={() => (handPicker = null)}
  />
{/if}

<style>
  .setup {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 300px;
    gap: 16px;
    align-items: start;
  }
  .stagecol {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
  }
  .under {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px;
    font-size: 12px;
  }
  .spacer {
    flex: 1;
  }
  .handnote {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: var(--text-2);
  }
  .cards {
    display: inline-flex;
    gap: 2px;
  }
  .scen {
    display: flex;
    flex-direction: column;
    padding: 4px 14px;
  }
  .scen section {
    padding: 12px 0;
  }
  .scen section + section {
    border-top: 1px solid var(--line);
  }
  h3 {
    margin: 0 0 8px;
    font-size: 12.5px;
    font-weight: 600;
    color: var(--text);
  }
  .h3row {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
  }
  .pair {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .seg.wide {
    display: flex;
  }
  .seg.wide button {
    flex: 1;
    padding: 0 6px;
    white-space: nowrap;
  }
  .small {
    margin: 6px 0 0;
    font-size: 11.5px;
    line-height: 1.4;
  }
  .depth {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .depth input[type="range"] {
    flex: 1;
    min-width: 0;
  }
  .unitfield {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: 12px;
    color: var(--text-2);
  }
  .unitfield .field {
    width: 62px;
  }
  .facts {
    display: flex;
    gap: 14px;
    margin-top: 8px;
    font-size: 12px;
    color: var(--text-2);
  }
  .facts b {
    color: var(--text);
    font-weight: 600;
  }
  .who {
    display: grid;
    grid-template-columns: 46px minmax(0, 1fr) auto;
    gap: 8px;
    align-items: baseline;
    padding: 3px 0;
    font-size: 12.5px;
  }
  .who .dim {
    line-height: 1.35;
  }
  .origin {
    margin: 6px 0 10px;
    color: var(--text-2);
    font-size: 11.5px;
    line-height: 1.4;
  }
  .origin.custom {
    color: var(--amber);
  }
  .warn {
    color: var(--amber);
    margin: 0 0 10px;
  }
  .full {
    width: 100%;
  }
  .ranges {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 12px;
    margin-top: 16px;
    container-type: inline-size;
  }
  .expert {
    margin-top: 16px;
    padding: 0;
  }
  .expert > summary {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 4px 12px;
    padding: 12px 16px;
    cursor: pointer;
    font-size: 13px;
  }
  .expert > summary b {
    font-weight: 600;
  }
  .lower {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 300px;
    gap: 16px;
    padding: 0 16px 14px;
    align-items: start;
  }
  .box {
    container-type: inline-size;
  }
  .acc {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .acc label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 12px;
    color: var(--text-2);
  }
  .acc details summary {
    cursor: pointer;
    font-size: 12.5px;
    color: var(--text-2);
  }
  .acc2 {
    display: grid;
    grid-template-columns: 1fr 1fr 1fr;
    gap: 10px;
    margin-top: 8px;
  }
  .acc .field {
    width: 100%;
  }
  .go {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-3) var(--sp-4);
    margin-top: var(--sp-1);
    padding: var(--sp-3) var(--sp-4);
  }
  .est {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 18px;
    flex: 1;
    font-size: 13px;
    color: var(--text-2);
  }
  .est b {
    color: var(--text);
    font-weight: 500;
  }
  .est .spot b {
    font-weight: 700;
  }
  .est b.neg {
    color: var(--neg);
  }
  .go .btn.primary {
    min-width: 110px;
  }





  @container solverpage (max-width: 900px) {
    .setup {
      grid-template-columns: minmax(0, 1fr) 256px;
    }
    .lower {
      grid-template-columns: minmax(0, 1fr);
    }
  }
  @container solverpage (max-width: 680px) {
    .setup,
    .ranges {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
