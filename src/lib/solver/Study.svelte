<script lang="ts">
  import { untrack } from "svelte";
  import type { Anchor } from "../app/format";
  import { signed } from "../app/format";
  import PlayingCard from "../PlayingCard.svelte";
  import ActionBar from "./ActionBar.svelte";
  import { actionColors, bb, bytes, duration } from "./actions";
  import DeckPopover from "./DeckPopover.svelte";
  import { cap, GLOSSARY, handAdvice, headline, often, precision, verb, why } from "./explain";
  import { loadNode } from "./nodes";
  import { STREET_INDEX, STREET_NAME, actionText, isAggressive, matchStep, nearestSize, targetAmount, withSize, type Crumb, type SizeMatch } from "./play";
  import MiniCard from "../ui/MiniCard.svelte";
  import { handNumbers } from "./practice";
  import { classLabel } from "./range";
  import StrategyGrid, { type GridMode } from "./StrategyGrid.svelte";
  import { solveNotice, streetAt } from "./handoff.svelte";
  import MeChooser from "./MeChooser.svelte";
  import { meSide, solvedTable, solver, startSolve } from "./store.svelte";
  import type { SeatView } from "./SolverTable.svelte";
  import SolverTable from "./SolverTable.svelte";
  import Term from "./Term.svelte";
  import type { SolverHand, SolverNode, SolverPlayer, SolverStep } from "./types";
  import Button from "../ui/Button.svelte";

  let { onEdit, onNew, onPractice }: { onEdit: () => void; onNew: () => void; onPractice: () => void } = $props();

  const MAX_SIZES = 4;
  let line = $state<Crumb[]>([]);
  let node = $state<SolverNode | null>(null);
  let error = $state<string | null>(null);
  let loading = $state(false);
  let note = $state<string | null>(null);

  const table = $derived(solvedTable());
  const spot = $derived(solver.solved ?? solver.spot);
  const summary = $derived(solver.summary);
  const path = $derived(line.map((c) => c.step));
  const pos = (p: SolverPlayer) => table.positions[p];

  $effect(() => {
    const p = $state.snapshot(path);
    let live = true;
    loading = true;
    loadNode(p)
      .then((n) => {
        if (!live) return;
        node = n;
        error = null;
      })
      .catch((e) => live && (error = String(e)))
      .finally(() => live && (loading = false));
    return () => (live = false);
  });

  // A handed-over line can arrive after the page is up, once a saved solution has opened.
  $effect(() => {
    const r = solver.replay;
    if (!r || solver.status !== "solved") return;
    untrack(() => {
      solver.replay = null;
      replay(r);
    });
  });

  /**
   * Walks the old line in the new tree; actions map to the closest size of the same kind. An
   * exact `path` past what a saved solution holds solves the spot in full and walks on.
   */
  async function replay(r: NonNullable<typeof solver.replay>) {
    const out: Crumb[] = [];
    try {
      for (const step of r.path ?? []) {
        const n = await loadNode(out.map((x) => x.step));
        out.push("card" in step ? cardCrumb(n, step.card) : crumbFor(n, step.action));
      }
      for (const c of r.line) {
        const n = await loadNode(out.map((x) => x.step));
        const step = matchStep(n, c);
        if (!step) {
          note = `Ab „${c.label}“ gibt es die Linie im neuen Baum nicht.`;
          break;
        }
        out.push("action" in step ? crumbFor(n, step.action) : c);
      }
      if (r.then && out.length === r.line.length) {
        const n = await loadNode(out.map((x) => x.step));
        const m = nearestSize(n, r.then.amount);
        if (m) out.push(crumbFor(n, m.index));
      }
    } catch (e) {
      if (r.path && solver.partial) return deeper(r.path);
      note = String(e);
    }
    line = out;
  }

  /** Solves the saved spot again in full and walks back to `path` or where the user was. */
  async function deeper(path?: SolverStep[]) {
    const to = path ?? $state.snapshot(line).map((c) => c.step);
    const notice = await solveNotice(
      `Für den ${streetAt(to)} rechnet Ringside kurz nach`,
      "Gespeichert sind nur der Flop und die erste Entscheidung am Turn; danach geht es an derselben Stelle weiter.",
    );
    solver.replay = { line: [], then: null, path: to, notice };
    startSolve(true);
  }

  function cardCrumb(n: SolverNode, card: string): Crumb {
    return { step: { card }, player: null, label: card, street: n.board.length === 3 ? "turn" : "river" };
  }

  function crumbFor(n: SolverNode, k: number): Crumb {
    const a = n.actions[k];
    return { step: { action: k }, player: n.player, label: `${pos(n.player!)} ${actionText(a, n)}`, street: n.street, kind: a.kind, amount: a.amount };
  }

  const colors = $derived(node ? actionColors(node.actions) : []);
  const labels = $derived(node ? node.actions.map((a) => actionText(a, node!)) : []);
  const hero = $derived(node && solver.hero && solver.hero.player === node.player ? solver.hero.cards : null);

  function act(k: number) {
    if (!node || node.kind !== "action" || k >= node.actions.length) return;
    line = [...line, crumbFor(node, k)];
    sizeOpen = false;
    match = null;
    note = null;
  }

  function deal(card: string) {
    if (!node || node.kind !== "chance") return;
    line = [...line, cardCrumb(node, card)];
    picker = null;
  }

  function dealRandom() {
    if (!node || node.kind !== "chance") return;
    deal(node.cards[Math.floor(Math.random() * node.cards.length)]);
  }

  function back(to: number) {
    line = line.slice(0, to);
    match = null;
    note = null;
  }

  // Turn and river are picked at the table.
  let picker = $state<Anchor | null>(null);
  function openSlot(_: number, el: HTMLElement) {
    const r = el.getBoundingClientRect();
    picker = { left: r.left, top: r.top, right: r.right, bottom: r.bottom };
  }

  // Own sizes.
  let sizeOpen = $state(false);
  let sizeValue = $state<number | null>(null);
  let sizeUnit = $state<"bb" | "pct">("pct");
  let match = $state<(SizeMatch & { street: number }) | null>(null);
  let sizeError = $state<string | null>(null);
  const canBet = $derived(!!node && node.kind === "action" && node.actions.some((a) => isAggressive(a.kind)));

  function tryOwn(e: Event) {
    e.preventDefault();
    if (!node || sizeValue === null || !(sizeValue > 0)) return;
    const m = nearestSize(node, targetAmount(node, sizeValue, sizeUnit));
    sizeError = null;
    match = null;
    if (!m) return;
    if (m.tooBig) {
      sizeError = `Mehr als der Stack erlaubt; All-in sind ${bb(node.street_bets[side(node.player!)] + node.stacks[side(node.player!)], 2)} BB.`;
      return;
    }
    if (m.exact) act(m.index);
    else match = { ...m, street: STREET_INDEX[node.street] };
  }

  const side = (p: SolverPlayer) => (p === "oop" ? 0 : 1);

  function resolveWith() {
    if (!match) return;
    const base = $state.snapshot(solver.solved ?? solver.spot);
    solver.spot = withSize(base, match.street, match.kind, match.pct, MAX_SIZES);
    solver.positions = { ...table.positions };
    solver.labels = { ...table.labels };
    solver.replay = { line: $state.snapshot(line), then: { kind: match.kind, amount: match.wanted } };
    startSolve();
  }

  // Grid and hand detail.
  let mode = $state<GridMode>("strategy");
  let pinned = $state<number | null>(null);
  // Your exact cards, as the practice grades them; the grid cell mixes the whole class.
  const heroNumbers = $derived(hero && node ? handNumbers(node, hero) : null);
  const heroClass = $derived(heroNumbers?.hand?.class ?? null);
  const heroAdvice = $derived(heroNumbers?.hand && node ? handAdvice(node, heroNumbers.hand.strategy, [heroNumbers.hand]) : null);

  function key(e: KeyboardEvent) {
    const t = e.target as HTMLElement | null;
    if (t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.tagName === "SELECT")) return;
    if (e.ctrlKey || e.metaKey || e.altKey) return;
    if (/^[1-9]$/.test(e.key) && node?.kind === "action") act(Number(e.key) - 1);
    else if (e.key === "Backspace" && line.length) back(line.length - 1);
    else if ((e.key === "z" || e.key === "Z") && node?.kind === "chance") dealRandom();
    else if (e.key === "Escape" && pinned !== null) pinned = null;
    else return;
    e.preventDefault();
  }

  /** The last action of each player on the street on screen, as a tag under the seat. */
  const views = $derived.by(() => {
    const out: { oop: SeatView; ip: SeatView } = { oop: {}, ip: {} };
    const street = node?.street;
    for (const c of line) {
      if (c.player && c.street === street) out[c.player] = { tag: c.label.slice(pos(c.player).length + 1).replace(/ \(.*\)$/, ""), tone: c.kind === "all_in" ? "allin" : undefined };
    }
    if (solver.hero) out[solver.hero.player].cards = solver.hero.cards;
    return out;
  });

  const ended = $derived(node?.kind === "terminal");
  const folded = $derived(ended && line.at(-1)?.kind === "fold" ? line.at(-1)!.player : null);
  const chipsShown = $derived<[number, number]>(node && node.kind === "action" ? node.street_bets : [0, 0]);
  const players = $derived(node?.player ? node.players[side(node.player)] : null);
  const startBoard = $derived(spot.board.replace(/[^AKQJT2-9cdhs]/gi, "").match(/.{2}/g) ?? []);
  const pct = (x: number) => `${Math.round(x * 100)} %`;

  /** How your hand differs from the range it belongs to, in equity and in how often it bets. */
  function handVsRange(n: SolverNode, h: SolverHand): string {
    const aggressive = (k: number) => isAggressive(n.actions[k].kind);
    const mine = h.strategy.reduce((s, f, k) => s + (aggressive(k) ? f : 0), 0);
    const all = n.actions.reduce((s, a) => s + (isAggressive(a.kind) ? a.frequency : 0), 0);
    const eq = n.players[side(n.player!)].equity;
    const strength =
      h.equity > eq + 0.08 ? "stärker als der Schnitt" : h.equity < eq - 0.08 ? "schwächer als der Schnitt" : "etwa so stark wie der Schnitt";
    const play =
      mine > all + 0.1
        ? `Mit ihr setzt die Lösung deutlich öfter (${pct(mine)} statt ${pct(all)}).`
        : mine < all - 0.1
          ? `Mit ihr setzt die Lösung seltener (${pct(mine)} statt ${pct(all)}).`
          : "Gespielt wird sie etwa wie der Rest der Range.";
    return `Deine Hand ist ${strength} deiner Range (${pct(h.equity)} gegen ${pct(eq)} Equity). ${play}`;
  }

  /** Every action the solution takes with your hand, most often first. */
  function handLine(n: SolverNode, strategy: number[]): string {
    const parts = strategy
      .map((f, k) => ({ f, k }))
      .filter((x) => x.f >= 0.03)
      .sort((x, y) => y.f - x.f)
      .slice(0, 3)
      .map((x) => `${often(x.f)} ${verb(n.actions[x.k], n)}`);
    return parts.length > 1 ? `${parts.slice(0, -1).join(", ")} und ${parts.at(-1)}` : (parts[0] ?? "");
  }
</script>

<svelte:window onkeydown={key} />

<div class="study">
  <section class="head">
    <div class="title">
      <b>{table.positions.oop} gegen {table.positions.ip}</b>
      <span class="dim num">Pot {bb(spot.pot, 2)} BB · effektiv {bb(spot.stack, 2)} BB</span>
    </div>
    {#if summary}
      <div class="stats">
        <span>
          <Term term="exploit">Lösung</Term>
          <b class:warn={!summary.converged}>{precision(summary.exploitability_pct)}</b>
          <span class="num faint">({summary.exploitability_pct.toLocaleString("de-DE", { maximumFractionDigits: 2 })} % des Pots)</span>
        </span>
        {#if summary.threads > 0}<span>Zeit <b class="num">{duration(summary.elapsed_ms / 1000)}</b></span>{/if}
        <span>Speicher <b class="num">{bytes(summary.memory_bytes)}{summary.compressed ? " (16 Bit)" : ""}</b></span>
      </div>
    {/if}
    <div class="buttons">
      <button class="btn" onclick={onPractice}>Üben</button>
      <button class="btn" onclick={onEdit}>Spot bearbeiten</button>
      <button class="btn ghost" onclick={onNew}>Verwerfen</button>
    </div>
  </section>

  {#if (solver.notes.oop || solver.notes.ip) && solver.source}
    <p class="notice">Die Ranges sind Annahmen aus Position und Preflop-Aktion, nicht aus der Hand gelesen.</p>
  {/if}
  {#if summary && !summary.converged}
    <p class="notice">Das Genauigkeitsziel wurde nicht erreicht; die Strategie ist noch ungenau.</p>
  {/if}

  <nav class="line" aria-label="Spielverlauf">
    <button class:on={line.length === 0} onclick={() => back(0)}>
      {["Flop", "Turn", "River"][Math.max(0, startBoard.length - 3)]}
      {#each startBoard as c (c)}<PlayingCard card={c} size={13} />{/each}
    </button>
    {#each line as c, i (i)}
      <span class="sep" aria-hidden="true">›</span>
      <button class:on={i === line.length - 1} class:card={"card" in c.step} onclick={() => back(i + 1)}>
        {#if "card" in c.step}
          {STREET_NAME[c.street]} <PlayingCard card={c.step.card} size={13} />
        {:else}
          {c.label}
        {/if}
      </button>
    {/each}
    {#if line.length}<span class="hint faint">⌫ zurück</span>{/if}
  </nav>

  {#if error}
    <p class="err">
      {error}
      {#if solver.partial && line.length}
        <button class="btn sm deeper" onclick={() => deeper()}>Jetzt tiefer rechnen</button>
      {/if}
    </p>
  {/if}
  {#if solver.reused && !error}
    <p class="notice quiet">
      Aus „Meine Lösungen“ geöffnet, ohne neu zu rechnen.{solver.partial
        ? " Gespeichert sind die erste Straße und der Einstieg in die nächste; tiefer rechnet Ringside bei Bedarf neu."
        : ""}
    </p>
  {/if}
  {#if note}<p class="notice">{note}</p>{/if}

  {#if node}
    <section class="answer" aria-live="polite">
      {#if node.kind === "action" && node.player && hero && heroNumbers?.hand && heroAdvice}
        {@const h = heroNumbers.hand}
        <span class="who"><b>Du</b> bist am Zug · {pos(node.player)}</span>
        <div class="mineanswer">
          <span class="cards" aria-label="Deine Hand">{#each hero as c (c)}<PlayingCard card={c} size={34} />{/each}</span>
          <div>
            <p class="say">Mit deiner Hand hier {handLine(node, h.strategy)}.</p>
            {#if heroAdvice.close}<p class="close">{heroAdvice.close}</p>{/if}
          </div>
        </div>
        <p class="mixcap"><b>Deine Hand</b> · so oft spielt die Lösung jede Aktion mit genau diesen zwei Karten</p>
        <div class="handmix" role="img" aria-label="So spielt die Lösung deine Hand">
          {#each h.strategy as f, k (k)}
            {#if f >= 0.005}
              <i style="flex-grow:{f}; --c:{colors[k]}" title="{labels[k]}: in {pct(f)} der Fälle">
                <span>{cap(verb(node.actions[k], node))}</span><b class="num">{pct(f)}</b>
              </i>
            {/if}
          {/each}
        </div>
        <div class="whyrange">
          <p><b>Warum deine Hand so?</b> {handVsRange(node, h)}</p>
          <p><b>Ganze Range:</b> {cap(headline(node).replace(/^Hier /, ""))} {why(node)}</p>
        </div>
      {:else if node.kind === "action" && node.player}
        <span class="who">
          <b>{pos(node.player)}</b> ist am Zug ·
          {#if node.player === "oop"}<Term term="oop">OOP</Term>{:else}<Term term="ip">IP</Term>{/if}
        </span>
        {#if hero && heroNumbers && !heroNumbers.hand}
          <p class="say">Deine Hand <span class="inline">{#each hero as c (c)}<MiniCard card={c} />{/each}</span> kommt hier nicht vor.</p>
          <p class="because">
            {pos(node.player)} spielt diese Hand bis hierher laut Range nie, darum gibt es für sie keine Strategie. So spielt die Range: {headline(node).replace(/^Hier /, "")}
          </p>
        {:else}
          <p class="say">{headline(node)}</p>
          <p class="because">{why(node)}</p>
        {/if}
      {:else if node.kind === "chance"}
        <p class="say">Jetzt kommt der {node.board.length === 3 ? "Turn" : "River"}.</p>
        <p class="because">Wähle die Karte am Tisch oder lass sie zufällig kommen – danach siehst du, wie sich die Strategie ändert.</p>
      {:else}
        <p class="say">Die Hand ist vorbei.</p>
        <p class="because">Geh im Verlauf oben zurück, um eine andere Linie auszuprobieren.</p>
      {/if}
    </section>

    {#if node.kind === "action" && node.player}
      <section class="acts" class:busy={loading}>
        <div class="actor">
          <span><b>Weiterspielen · ganze Range</b> – Klick auf eine Aktion. Zahl und Balken im Knopf zeigen, wie oft die ganze Range sie wählt.</span>
          <span class="faint">Tasten 1–{Math.min(9, node.actions.length)}</span>
        </div>
        <ActionBar {node} {colors} onAct={act} />
        {#if canBet}
          <div class="own">
            {#if !sizeOpen}
              <button class="link" onclick={() => (sizeOpen = true)}>Eigene Größe eingeben …</button>
            {:else}
              <form class="ownform" onsubmit={tryOwn}>
                <label for="own-size">Eigene Größe</label>
                <input id="own-size" class="field num" type="number" min="0.01" step="any" bind:value={sizeValue} />
                <div class="seg" role="group" aria-label="Einheit">
                  <button type="button" class:on={sizeUnit === "pct"} onclick={() => (sizeUnit = "pct")}>% Pot</button>
                  <button type="button" class:on={sizeUnit === "bb"} onclick={() => (sizeUnit = "bb")}>BB</button>
                </div>
                <button class="btn" type="submit">Spielen</button>
                <button class="link" type="button" onclick={() => ((sizeOpen = false), (match = null), (sizeError = null))}>abbrechen</button>
              </form>
              {#if sizeUnit === "pct" && node.street_bets.some((b) => b > 0)}
                <p class="faint small">Bei einem Raise: Prozent des Pots nach dem Call.</p>
              {/if}
            {/if}
            {#if sizeError}<p class="neg small">{sizeError}</p>{/if}
            {#if match}
              {@const near = node.actions[match.index]}
              <div class="mapped" role="status">
                <p>
                  <b>{bb(match.wanted, 2)} BB</b> ({Math.round(match.pct)} %) ist nicht im gelösten Baum. Der Solver nimmt die nächstliegende Größe:
                  <b>{actionText(near, node)}</b>, {bb(Math.abs(match.off), 2)} BB {match.off > 0 ? "mehr" : "weniger"}.
                </p>
                <div class="mapbtns">
                  <button class="btn" onclick={() => act(match!.index)}>Mit {actionText(near, node).replace(/ \(.*\)$/, "")} weiterspielen</button>
                  <button class="btn primary" onclick={resolveWith}>Mit {Math.round(match.pct)} % neu lösen</button>
                </div>
                <p class="faint small">
                  Neu lösen nimmt {Math.round(match.pct)} % zu den {match.kind === "bet" ? "Bet" : "Raise"}-Größen am {["Flop", "Turn", "River"][match.street]}
                  hinzu{spot.sizes[match.street][match.kind].length >= MAX_SIZES ? " und ersetzt die nächstliegende" : ""} und spielt die Linie danach wieder bis hierher.
                </p>
              </div>
            {/if}
          </div>
        {/if}
      </section>
    {/if}

    <div class="body" class:busy={loading}>
      <div class="left">
        <SolverTable
          players={table.players}
          positions={table.positions}
          anchor={table.positions[meSide()]}
          board={node.board}
          nextSlot={node.kind === "chance" ? node.board.length : -1}
          onSlot={node.kind === "chance" ? openSlot : undefined}
          pot={node.pot}
          bets={chipsShown}
          stacks={node.stacks}
          acting={node.player}
          {views}
          street={STREET_NAME[node.street]}
        />
        <MeChooser positions={table.positions} />

        {#if node.kind === "chance"}
          <div class="chance">
            <span><b>{node.board.length === 3 ? "Turn" : "River"}</b>: Karte am Tisch wählen</span>
            <Button onclick={dealRandom} kbd="Z">Zufällige Karte</Button>
          </div>
        {:else if node.kind === "terminal"}
          <div class="chance">
            <span>
              {#if folded}
                <b>{pos(folded)}</b> foldet – {pos(folded === "oop" ? "ip" : "oop")} gewinnt {bb(node.pot, 2)} BB.
              {:else}
                Showdown um {bb(node.pot, 2)} BB.
              {/if}
            </span>
            <Button onclick={() => back(line.length - 1)} kbd="⌫">Einen Schritt zurück</Button>
          </div>
        {/if}
      </div>

      <div class="right panel">
        {#if node.kind === "action" && node.player}
          <header class="rhead">
            <div>
              <b>{pos(node.player)}</b>
              {#if players}
                <span class="dim num"
                  >{bb(players.combos, 1)} Kombos · <Term term="equity">Equity</Term> {pct(players.equity)} · <Term term="ev">EV</Term>
                  {signed(players.ev, 2)} BB</span
                >
              {/if}
            </div>
            <div class="seg" role="group" aria-label="Ansicht">
              {#each [["strategy", "Strategie", "Was jede Hand hier spielt; Farbe je Aktion"], ["range", "Range", GLOSSARY.range], ["ev", "EV", GLOSSARY.ev], ["equity", "Equity", GLOSSARY.equity]] as [m, l, hint] (m)}
                <button class:on={mode === m} title={hint} onclick={() => (mode = m as GridMode)}>{l}</button>
              {/each}
            </div>
          </header>
          <div class="grid">
            <StrategyGrid
              {node}
              {labels}
              {colors}
              {mode}
              {hero}
              selected={pinned ?? -1}
              onSelect={(i) => (pinned = pinned === i ? null : i)}
              onClose={() => (pinned = null)}
            />
            <div class="legend">
              {#if mode === "strategy"}
                {#each node.actions as _, k (k)}<span><i style="background:{colors[k]}"></i>{labels[k]}</span>{/each}
              {:else if mode === "range"}
                <span class="faint">Füllhöhe: Anteil der Kombos, die diesen Punkt erreichen.</span>
              {:else if mode === "ev"}
                <span class="faint">EV in BB: grün höher, rot unter null. Zahl in der Zelle.</span>
              {:else}
                <span class="faint">Equity gegen die Range des Gegners an diesem Punkt, in %.</span>
              {/if}
            </div>
            {#if heroClass !== null && heroAdvice}
              <p class="mine">
                Deine Hand <span class="inline">{#each hero ?? [] as c (c)}<MiniCard card={c} />{/each}</span> ist im Raster umrandet.
                <span class="faint">Die Zelle {classLabel(heroClass)} mischt alle Farbvarianten dieser Hand.</span>
                <button class="more" onclick={() => (pinned = heroClass)}>Details</button>
              </p>
            {:else}
              <p class="mine faint">Fahre über eine Hand für ihre Werte, Klick hält sie fest.</p>
            {/if}
          </div>
        {:else}
          <div class="idle">
            <p class="dim">
              {node.kind === "chance" ? "Nach der nächsten Karte zeigt das Raster wieder die Strategie des Spielers am Zug." : "Die Hand ist zu Ende."}
            </p>
            <table class="sum">
              <thead><tr><th></th><th>Kombos</th><th>Equity</th><th>EV</th></tr></thead>
              <tbody>
                {#each ["oop", "ip"] as const as p, i (p)}
                  <tr>
                    <td>{pos(p)}</td>
                    <td class="num">{bb(node.players[i].combos, 1)}</td>
                    <td class="num">{pct(node.players[i].equity)}</td>
                    <td class="num">{signed(node.players[i].ev, 2)}</td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        {/if}
      </div>
    </div>
  {:else if !error}
    <p class="faint">Lädt …</p>
  {/if}
</div>

{#if picker && node?.kind === "chance"}
  <DeckPopover
    anchor={picker}
    title={node.board.length === 3 ? "Turn wählen" : "River wählen"}
    available={node.cards}
    onPick={deal}
    onRandom={dealRandom}
    onClose={() => (picker = null)}
  />
{/if}

<style>
  .study {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 22px;
  }
  .title {
    display: flex;
    flex-direction: column;
  }
  .title b {
    font-family: var(--font-num);
    font-size: 18px;
    font-weight: 700;
  }
  .title span {
    font-size: 12.5px;
  }
  .stats {
    display: flex;
    flex-wrap: wrap;
    gap: 2px 14px;
    font-size: 12.5px;
    color: var(--text-2);
  }
  .stats b {
    color: var(--text);
    font-weight: 500;
  }
  .stats b.warn {
    color: var(--amber);
  }
  .buttons {
    display: flex;
    gap: 6px;
    margin-left: auto;
  }
  .notice {
    margin: 0;
    padding: 7px 12px;
    border-left: 3px solid var(--amber);
    background: color-mix(in srgb, var(--amber) 7%, transparent);
    color: var(--text-2);
    font-size: 12.5px;
  }
  .answer {
    padding: 12px 16px;
    border-radius: var(--r);
    background: var(--surface);
    border: 1px solid var(--line);
    border-left: 3px solid var(--text);
  }
  .answer .who {
    font-size: 12px;
    color: var(--text-2);
  }
  .answer .who b {
    color: var(--text);
  }
  .say {
    margin: 2px 0 0;
    font-size: 18px;
    font-weight: 600;
    line-height: 1.3;
  }
  .mineanswer {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    margin-top: var(--sp-1);
  }
  .mineanswer .cards {
    display: inline-flex;
    gap: 3px;
  }
  .mineanswer .say {
    margin: 0;
  }
  .close {
    margin: 2px 0 0;
    font-size: var(--fs-md);
    color: var(--text);
  }
  .handmix {
    display: flex;
    gap: 2px;
    height: 28px;
    margin-top: var(--sp-1);
    border-radius: var(--r-xs);
    overflow: hidden;
  }
  .handmix i {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-2);
    min-width: 0;
    padding: 0 var(--sp-2);
    background: color-mix(in srgb, var(--c) 24%, var(--surface-2));
    box-shadow: inset 0 -3px 0 var(--c);
    color: var(--text);
    font-size: var(--fs-xs);
    font-style: normal;
    font-weight: var(--fw-semibold);
    white-space: nowrap;
    overflow: hidden;
  }
  .handmix span {
    overflow: hidden;
    text-overflow: ellipsis;
    font-weight: var(--fw-medium);
  }
  .handmix b {
    font-weight: var(--fw-bold);
  }
  .mixcap {
    margin: var(--sp-3) 0 0;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .mixcap b {
    color: var(--text);
    font-weight: var(--fw-semibold);
  }
  .whyrange {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
    gap: var(--sp-2) var(--sp-6);
    margin: var(--sp-3) 0 0;
    padding-top: var(--sp-3);
    border-top: 1px solid var(--line);
    font-size: var(--fs-md);
    line-height: 1.5;
    color: var(--text-2);
  }
  .whyrange p {
    margin: 0;
    max-width: 72ch;
  }
  .whyrange b {
    color: var(--text);
    font-weight: var(--fw-semibold);
  }
  .inline {
    display: inline-flex;
    gap: 2px;
  }
  .actor b {
    color: var(--text);
    font-weight: var(--fw-semibold);
  }

  .because {
    margin: 4px 0 0;
    font-size: 13px;
    color: var(--text-2);
    line-height: 1.45;
  }
  .line {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px;
    font-size: 12.5px;
  }
  .line button {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    height: 26px;
    padding: 0 8px;
    border: 1px solid var(--line);
    border-radius: var(--r-sm);
    background: var(--surface);
    color: var(--text-2);
    cursor: pointer;
    white-space: nowrap;
  }
  .line button:hover {
    color: var(--text);
  }
  .line button.on {
    color: var(--text);
    border-color: var(--line-2);
    background: var(--surface-3);
  }
  .line button.card {
    background: var(--surface-2);
  }
  .sep {
    color: var(--text-3);
  }
  .hint {
    margin-left: 6px;
    font-size: 11.5px;
  }
  .body {
    display: grid;
    grid-template-columns: minmax(0, 1.15fr) minmax(340px, 1fr);
    grid-template-areas: "left right";
    gap: 12px 16px;
    align-items: start;
  }
  .body.busy {
    opacity: 0.75;
  }
  .left {
    /* The actions under the table stay in view in short windows. */
    --stage-vh: 40vh;
    grid-area: left;
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
  }
  .acts {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }
  .acts.busy {
    opacity: 0.75;
  }
  .actor {
    display: flex;
    justify-content: space-between;
    gap: 10px;
    font-size: 12.5px;
    color: var(--text-2);
  }
  .own {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 12.5px;
  }
  .link {
    align-self: flex-start;
    padding: 0;
    border: 0;
    background: none;
    color: var(--text-2);
    font-size: 12.5px;
    text-decoration: underline;
    cursor: pointer;
  }
  .ownform {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
  .ownform label {
    color: var(--text-2);
  }
  .ownform .field {
    width: 86px;
  }
  .ownform .link {
    align-self: center;
  }
  .mapped {
    padding: 10px 12px;
    border-radius: var(--r-sm);
    border: 1px solid color-mix(in srgb, var(--amber) 45%, var(--line-2));
    background: color-mix(in srgb, var(--amber) 8%, var(--surface));
  }
  .mapped p {
    margin: 0;
    line-height: 1.45;
  }
  .mapbtns {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin: 8px 0 6px;
  }
  .small {
    margin: 0;
    font-size: 11.5px;
  }
  .chance {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 10px 12px;
    border-radius: var(--r-sm);
    background: var(--surface);
    border: 1px solid var(--line);
    font-size: 13px;
  }
  .right {
    grid-area: right;
    padding: 12px 14px;
    min-width: 0;
    container-type: inline-size;
  }
  .rhead {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 6px 10px;
    margin-bottom: 10px;
  }
  .rhead b {
    font-family: var(--font-num);
    font-size: 15px;
    margin-right: 6px;
  }
  .rhead span {
    font-size: 12px;
  }
  .legend {
    display: flex;
    flex-wrap: wrap;
    gap: 2px 12px;
    margin-top: 6px;
    font-size: 11.5px;
    color: var(--text-2);
  }
  .legend i {
    display: inline-block;
    width: 8px;
    height: 8px;
    margin-right: 5px;
    border-radius: 2px;
  }
  .mine {
    margin: 8px 0 0;
    padding-top: 8px;
    border-top: 1px solid var(--line);
    font-size: 12.5px;
    line-height: 1.45;
  }
  .more {
    padding: 0;
    border: 0;
    background: none;
    color: var(--text-2);
    font: inherit;
    text-decoration: underline;
    text-underline-offset: 2px;
    cursor: pointer;
  }
  .more:hover {
    color: var(--text);
  }
  .idle p {
    margin: 0 0 10px;
    font-size: 13px;
  }
  .sum {
    width: 100%;
    border-collapse: collapse;
    font-size: 12.5px;
  }
  .sum th {
    font-size: 11px;
    font-weight: 500;
    color: var(--text-3);
    text-align: right;
  }
  .sum td + td {
    text-align: right;
  }
  .err {
    color: var(--neg);
    margin: 0;
  }
  .notice.quiet {
    border-left-color: var(--text-3);
    background: var(--surface);
  }
  .deeper {
    height: 26px;
    margin-left: 8px;
    padding: 0 10px;
    color: var(--text);
    font-size: 12.5px;
  }
  @container solverpage (max-width: 900px) {
    .body {
      grid-template-columns: minmax(0, 1fr);
      grid-template-areas:
        "left"
        "right";
    }
    .buttons {
      margin-left: 0;
    }
  }
</style>
