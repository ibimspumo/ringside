<script lang="ts">
  import { onDestroy } from "svelte";
  import { signed } from "../app/format";
  import PlayingCard from "../PlayingCard.svelte";
  import Button from "../ui/Button.svelte";
  import ActionBar from "./ActionBar.svelte";
  import { actionColors, bb } from "./actions";
  import { cap, lesson, verb } from "./explain";
  import { loadNode } from "./nodes";
  import { STREET_NAME, actionText, type Crumb } from "./play";
  import { cardText } from "../play/labels";
  import {
    BEST_MIN_FREQUENCY,
    GRADES,
    THRESHOLDS,
    categoryName,
    compare,
    dealFrom,
    handOf,
    judge,
    pick,
    rank7,
    type Grade,
    type Verdict,
  } from "./practice";
  import type { SeatView } from "./SolverTable.svelte";
  import SolverTable from "./SolverTable.svelte";
  import { meSide, solvedTable, solver } from "./store.svelte";
  import { comboClass, parseRange } from "./range";
  import SegmentedControl from "../ui/SegmentedControl.svelte";
  import Term from "./Term.svelte";
  import type { SolverNode, SolverPlayer } from "./types";

  let { onStudy }: { onStudy: () => void } = $props();

  /**
   * You play one seat of the solved spot with a hand dealt from its range; the other seat
   * holds a hand from its range too and plays the solution's frequencies for that hand.
   */
  const table = $derived(solvedTable());
  const spot = $derived(solver.solved ?? solver.spot);
  const board = $derived<string[]>(spot.board.replace(/[^AKQJT2-9cdhs]/gi, "").match(/.{2}/g) ?? []);

  let me = $state<SolverPlayer>(meSide());
  // Your own hand from the setup, if it can be dealt here; otherwise hands from your range.
  const ownHand = $derived.by(() => {
    const h = solver.hero;
    if (!h || h.player !== me || h.cards.some((c) => board.includes(c))) return null;
    const w = parseRange(me === "oop" ? spot.oop_range : spot.ip_range);
    return w && w[comboClass(h.cards[0], h.cards[1])] > 0 ? h.cards : null;
  });
  let useOwn = $state(true);
  const other = $derived<SolverPlayer>(me === "oop" ? "ip" : "oop");
  const pos = (p: SolverPlayer) => table.positions[p];

  interface Decision {
    street: SolverNode["street"];
    label: string;
    verdict: Verdict;
    pot: number;
    /** The hand's own strategy and labels at that point. */
    strategy: number[];
    labels: string[];
    colors: string[];
    /** What the choice means, in plain words: a first line and why. */
    intro: string;
    lesson: string;
  }

  let cards = $state<{ me: string[]; other: string[] } | null>(null);
  let line = $state<Crumb[]>([]);
  let node = $state<SolverNode | null>(null);
  let phase = $state<"idle" | "me" | "wait" | "over">("idle");
  let decisions = $state<Decision[]>([]);
  let problem = $state<string | null>(null);
  let series = $state({ hands: 0, points: 0, decisions: 0, loss: 0, grades: { best: 0, good: 0, inaccurate: 0, mistake: 0 } as Record<Grade, number> });
  let hand = 0;

  const reduced = typeof matchMedia !== "undefined" && matchMedia("(prefers-reduced-motion: reduce)").matches;
  const pause = (ms: number) => new Promise((r) => setTimeout(r, reduced ? Math.min(ms, 150) : ms));
  let alive = true;
  onDestroy(() => (alive = false));

  function deal() {
    problem = null;
    const mine = ownHand && useOwn ? [...ownHand] : dealFrom(me === "oop" ? spot.oop_range : spot.ip_range, board);
    const theirs = mine && dealFrom(other === "oop" ? spot.oop_range : spot.ip_range, [...board, ...mine]);
    if (!mine || !theirs) {
      problem = "Aus diesen Ranges lässt sich keine Hand austeilen.";
      return;
    }
    cards = { me: mine, other: theirs };
    line = [];
    decisions = [];
    node = null;
    hand += 1;
    run(hand);
  }

  /** Plays the opponent and the cards until it is your turn or the hand is over. */
  async function run(id: number) {
    phase = "wait";
    for (;;) {
      let n: SolverNode;
      try {
        n = await loadNode($state.snapshot(line).map((c) => c.step));
      } catch (e) {
        problem = String(e);
        phase = "over";
        return;
      }
      if (!alive || id !== hand) return;
      node = n;
      if (n.kind === "terminal") return finish();
      const dead = [...(cards?.me ?? []), ...(cards?.other ?? [])];
      if (n.kind === "chance") {
        await pause(450);
        const left = n.cards.filter((c) => !dead.includes(c));
        const card = left[Math.floor(Math.random() * left.length)];
        line = [...line, { step: { card }, player: null, label: card, street: n.board.length === 3 ? "turn" : "river" }];
        continue;
      }
      if (n.player === me) {
        if (!handOf(n, cards!.me)) {
          problem = "Für diese Stelle hat die Lösung keine Daten je Hand – hier endet die Übung.";
          return finish();
        }
        phase = "me";
        return;
      }
      const theirs = handOf(n, cards!.other);
      const k = theirs ? pick(theirs.strategy) : pick(n.actions.map((a) => a.frequency));
      await pause(650);
      if (!alive || id !== hand) return;
      line = [...line, crumb(n, k)];
    }
  }

  function crumb(n: SolverNode, k: number): Crumb {
    const a = n.actions[k];
    return { step: { action: k }, player: n.player, label: actionText(a, n), street: n.street, kind: a.kind, amount: a.amount };
  }

  function choose(k: number) {
    if (phase !== "me" || !node || !cards) return;
    const h = handOf(node, cards.me);
    if (!h) return;
    const verdict = judge(h, k, node.pot);
    const labels = node.actions.map((a) => actionText(a, node!));
    const best = cap(verb(node.actions[verdict.best], node));
    const intro = {
      best: "Stark – so spielt es auch die Lösung.",
      good: `Gut gespielt – fast so viel wert wie die beste Wahl (${best}).`,
      inaccurate: `Knapp daneben: ${best} wäre etwas besser gewesen.`,
      mistake: `Hier lohnt sich ${best} deutlich mehr.`,
    }[verdict.grade];
    const why = lesson(node, h, verdict.grade === "best" ? k : verdict.best);
    decisions = [
      ...decisions,
      { street: node.street, label: labels[k], verdict, pot: node.pot, strategy: h.strategy, labels, colors: actionColors(node.actions), intro, lesson: why },
    ];
    series.decisions += 1;
    series.points += GRADES[verdict.grade].points;
    series.loss += verdict.loss ?? 0;
    series.grades[verdict.grade] += 1;
    line = [...line, crumb(node, k)];
    run(hand);
  }

  let result = $state<string>("");
  function finish() {
    phase = "over";
    series.hands += 1;
    const last = line.at(-1);
    if (!node || !cards) return;
    if (last?.kind === "fold") {
      const winner = last.player === me ? other : me;
      result = `${pos(last.player!)} foldet. ${winner === me ? "Du gewinnst" : `${pos(winner)} gewinnt`} den Pot von ${bb(node.pot, 2)} BB.`;
    } else if (node.board.length === 5) {
      const a = rank7([...cards.me, ...node.board]);
      const b = rank7([...cards.other, ...node.board]);
      const c = compare(a, b);
      result = `Showdown: du ${categoryName(a)}, ${pos(other)} ${categoryName(b)}. ${c > 0 ? `Du gewinnst ${bb(node.pot, 2)} BB.` : c < 0 ? `${pos(other)} gewinnt ${bb(node.pot, 2)} BB.` : "Split Pot."}`;
    } else {
      result = "Die Hand endet hier.";
    }
  }

  function key(e: KeyboardEvent) {
    const t = e.target as HTMLElement | null;
    if (t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.tagName === "SELECT")) return;
    if (e.ctrlKey || e.metaKey || e.altKey) return;
    if (/^[1-9]$/.test(e.key) && phase === "me") choose(Number(e.key) - 1);
    else if ((e.key === "n" || e.key === "N" || e.key === "Enter") && (phase === "over" || phase === "idle")) deal();
    else return;
    e.preventDefault();
  }

  function seat(pos: string) {
    if (phase === "me" || phase === "wait") return;
    me = pos === table.positions.oop ? "oop" : "ip";
  }

  const views = $derived.by(() => {
    const out: { oop: SeatView; ip: SeatView } = { oop: {}, ip: {} };
    const street = node?.street;
    for (const c of line) if (c.player && c.street === street) out[c.player] = { tag: c.label.replace(/ \(.*\)$/, ""), tone: c.kind === "all_in" ? "allin" : undefined };
    out[me].cards = cards?.me ?? (ownHand && useOwn ? ownHand : "hidden");
    out[other].cards = phase === "over" && cards ? cards.other : "hidden";
    if (!cards) out[me].tag = "Du";
    return out;
  });

  const last = $derived(decisions.at(-1) ?? null);
  const handPoints = $derived(decisions.length ? decisions.reduce((s, d) => s + GRADES[d.verdict.grade].points, 0) / decisions.length : null);
  const handLoss = $derived(decisions.reduce((s, d) => s + (d.verdict.loss ?? 0), 0));
  const colors = $derived(node && node.kind === "action" ? actionColors(node.actions) : []);
  const pct = (x: number) => `${Math.round(x * 100)} %`;
  const fmt = (x: number) => x.toLocaleString("de-DE", { maximumFractionDigits: 1 });
</script>

<svelte:window onkeydown={key} />

<div class="practice">
  <section class="head">
    <div class="title">
      <b>Üben: {pos(me)} gegen {pos(other)}</b>
      <span class="dim">
        {ownHand && useOwn ? "Du spielst deine Hand aus dem Aufbau" : "Du bekommst eine Hand aus deiner Range"}; der Gegner spielt die Lösung.
      </span>
    </div>
    <div class="seg" role="group" aria-label="Dein Platz">
      {#each ["oop", "ip"] as const as p (p)}
        <button class:on={me === p} disabled={phase === "me" || phase === "wait"} onclick={() => (me = p)}>{pos(p)} ({p === "oop" ? "OOP" : "IP"})</button>
      {/each}
    </div>
    {#if ownHand}
      <SegmentedControl
        size="sm"
        label="Welche Hand du spielst"
        options={[
          { value: "own", label: `Deine Hand ${ownHand.map(cardText).join("")}` },
          { value: "range", label: "Zufällig aus der Range" },
        ]}
        value={useOwn ? "own" : "range"}
        onchange={(v: string) => (useOwn = v === "own")}
      />
    {/if}
    <div class="buttons">
      <Button onclick={onStudy}>Zur Lösung</Button>
    </div>
  </section>

  <div class="body">
    <div class="left">
      <SolverTable
        players={table.players}
        positions={table.positions}
        anchor={pos(me)}
        board={node?.board ?? board}
        pot={node?.pot ?? spot.pot}
        bets={node && node.kind === "action" ? node.street_bets : [0, 0]}
        stacks={node?.stacks ?? [spot.stack, spot.stack]}
        acting={phase === "me" || phase === "wait" ? (node?.player ?? null) : null}
        {views}
        seatsClickable={phase === "idle" || phase === "over"}
        onSeat={seat}
        street={node ? STREET_NAME[node.street] : ""}
      />
      {#if phase === "me" && node}
        <div class="actor"><span><b>Du bist dran</b> mit {cards?.me.join(" ")}</span><span class="faint">Ziffern 1–{Math.min(9, node.actions.length)}</span></div>
        <ActionBar {node} {colors} onAct={choose} blind />
      {:else if phase === "wait"}
        <div class="actor"><span class="dim">{node?.kind === "chance" ? "Die nächste Karte kommt …" : `${pos(other)} überlegt …`}</span></div>
      {:else}
        <div class="start">
          {#if phase === "over"}<p>{result}</p>{/if}
          {#if problem}<p class="neg">{problem}</p>{/if}
          <Button variant="primary" kbd="N" onclick={deal}>
            {ownHand && useOwn ? (phase === "idle" ? "Deine Hand spielen" : "Noch einmal spielen") : phase === "idle" ? "Hand austeilen" : "Nächste Hand"}
          </Button>
          {#if phase === "idle"}<span class="faint small">Klick auf einen der beiden Plätze wählt deine Seite.</span>{/if}
        </div>
      {/if}
    </div>

    <aside class="right">
      <section class="panel box" aria-live="polite">
        <h3>Rückmeldung</h3>
        {#if last}
          <div class="verdict {last.verdict.grade}">
            <span class="badge">{GRADES[last.verdict.grade].label}</span>
            <b>{last.label}</b>
          </div>
          <p class="intro">{last.intro}</p>
          <p class="lesson">{last.lesson}</p>
          <p class="line">
            Die Lösung spielt mit deiner Hand
            {#each last.strategy as s, k (k)}{#if s >= 0.005}<span class="pill"><i style="background:{last.colors[k]}"></i>{last.labels[k].replace(/ \(.*\)$/, "")} {pct(s)}</span>{/if}{/each}
          </p>
          <div class="mix" aria-hidden="true">
            {#each last.strategy as s, k (k)}<i style="width:{s * 100}%; background:{last.colors[k]}"></i>{/each}
          </div>
          {#if last.verdict.loss !== null}
            <p class="loss num">
              {#if last.verdict.loss < 0.005}
                Kein EV-Verlust.
              {:else}
                <Term term="evloss">EV-Verlust</Term> <b>{bb(last.verdict.loss, 2)} BB</b> ({fmt(last.verdict.lossPct ?? 0)} % des Pots) gegenüber {last.labels[last.verdict.best].replace(/ \(.*\)$/, "")}.
              {/if}
            </p>
          {:else}
            <p class="faint small">Ohne EV je Aktion bewertet nach Häufigkeit.</p>
          {/if}
        {:else}
          <p class="faint small">Nach jeder Entscheidung steht hier, was die Lösung mit deiner Hand macht, warum – und was deine Wahl gekostet hätte.</p>
        {/if}
      </section>

      {#if decisions.length}
        <section class="panel box">
          <h3>Diese Hand</h3>
          <ol class="list">
            {#each decisions as d, i (i)}
              <li>
                <span class="faint">{STREET_NAME[d.street]}</span>
                <span>{d.label.replace(/ \(.*\)$/, "")}</span>
                <span class="tag {d.verdict.grade}">{GRADES[d.verdict.grade].label}</span>
                <span class="num dim">{d.verdict.loss !== null && d.verdict.loss >= 0.005 ? `−${bb(d.verdict.loss, 2)}` : "–"}</span>
              </li>
            {/each}
          </ol>
          {#if phase === "over" && handPoints !== null}
            {@const fine = decisions.filter((d) => d.verdict.grade === "best" || d.verdict.grade === "good").length}
            <p class="sumline">
              <b>{fine} von {decisions.length}</b> Entscheidungen wie die Lösung.
              {fine === decisions.length
                ? "Sauber gespielt!"
                : fine * 2 >= decisions.length
                  ? "Gut dabei – lies dir die Begründung bei der markierten Stelle noch einmal durch."
                  : "Kein Problem, genau dafür ist das Training da: Die Begründungen oben zeigen, worauf es ankommt."}
            </p>
            <p class="sumline num">Punkte <b>{Math.round(handPoints)}</b> von 100 · EV-Verlust <b>{bb(handLoss, 2)} BB</b></p>
          {/if}
        </section>
      {/if}

      <section class="panel box">
        <h3>Serie</h3>
        <div class="series num">
          <span>Hände <b>{series.hands}</b></span>
          <span>Punkte <b>{series.decisions ? Math.round(series.points / series.decisions) : "–"}</b></span>
          <span>EV-Verlust <b>{signed(-series.loss, 2)} BB</b></span>
        </div>
        <div class="grades">
          {#each Object.entries(GRADES) as [g, info] (g)}
            <span class="tag {g}">{info.label} {series.grades[g as Grade]}</span>
          {/each}
        </div>
        <details>
          <summary>So wird bewertet</summary>
          <p class="small">
            Maßstab ist der EV-Verlust deiner Wahl gegenüber der besten Aktion mit deiner Hand, in Prozent des Pots an dieser Stelle:
            <b>Beste Wahl</b> bis {fmt(THRESHOLDS.best)} % (so genau rechnet die Lösung) und die Lösung spielt die Aktion in mindestens
            {Math.round(BEST_MIN_FREQUENCY * 100)} % der Fälle, <b>Gut</b> bis {fmt(THRESHOLDS.good)} %, <b>Ungenau</b> bis {fmt(THRESHOLDS.inaccurate)} %,
            darüber <b>Fehler</b>. Punkte: 100 / 70 / 30 / 0.
          </p>
        </details>
      </section>
    </aside>
  </div>
</div>

<style>
  .practice {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 18px;
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
  .buttons {
    margin-left: auto;
  }
  .body {
    display: grid;
    grid-template-columns: minmax(0, 1.4fr) minmax(280px, 1fr);
    gap: 16px;
    align-items: start;
  }
  .left {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
  }
  .actor {
    display: flex;
    justify-content: space-between;
    gap: 10px;
    min-height: 20px;
    font-size: 12.5px;
    color: var(--text-2);
  }
  .actor b {
    color: var(--text);
  }
  .start {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 14px;
    padding: 10px 12px;
    border-radius: var(--r-sm);
    background: var(--surface);
    border: 1px solid var(--line);
  }
  .start p {
    flex-basis: 100%;
    margin: 0;
    font-size: 13px;
  }
  .right {
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-width: 0;
  }
  .box {
    padding: 12px 14px;
  }
  h3 {
    margin: 0 0 8px;
    font-size: 12px;
    font-weight: 600;
    color: var(--text-2);
  }
  .verdict {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 8px;
  }
  .verdict b {
    font-size: 15px;
  }
  .badge,
  .tag {
    display: inline-block;
    padding: 0 8px;
    border-radius: 99px;
    font-size: 11.5px;
    font-weight: 700;
    line-height: 20px;
    white-space: nowrap;
    border: 1px solid transparent;
  }
  .badge {
    font-size: 13px;
    line-height: 24px;
    padding: 0 10px;
  }
  .best {
    --g: var(--pos);
  }
  .good {
    --g: color-mix(in srgb, var(--pos) 55%, var(--text-2));
  }
  .inaccurate {
    --g: var(--amber);
  }
  .mistake {
    --g: var(--neg);
  }
  .verdict .badge,
  .tag {
    color: var(--g);
    background: color-mix(in srgb, var(--g) 14%, transparent);
    border-color: color-mix(in srgb, var(--g) 40%, transparent);
  }
  .intro {
    margin: 0 0 4px;
    font-size: 14px;
    font-weight: 600;
  }
  .lesson {
    margin: 0 0 10px;
    font-size: 13px;
    line-height: 1.5;
    color: var(--text-2);
  }
  .line {
    margin: 0 0 6px;
    font-size: 12.5px;
    line-height: 1.8;
    color: var(--text-2);
  }
  .pill {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    margin-left: 6px;
    color: var(--text);
    white-space: nowrap;
  }
  .pill i {
    width: 8px;
    height: 8px;
    border-radius: 2px;
  }
  .mix {
    display: flex;
    height: 8px;
    border-radius: 3px;
    overflow: hidden;
    background: var(--surface-2);
  }
  .loss {
    margin: 10px 0 0;
    font-size: 13px;
  }
  .small {
    margin: 0;
    font-size: 12px;
    line-height: 1.5;
  }
  .list {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 12.5px;
  }
  .list li {
    display: grid;
    grid-template-columns: 42px minmax(0, 1fr) auto 52px;
    align-items: center;
    gap: 8px;
  }
  .list li .num {
    text-align: right;
  }
  .sumline {
    margin: 10px 0 0;
    font-size: 12.5px;
    color: var(--text-2);
  }
  .sumline b,
  .series b {
    color: var(--text);
  }
  .series {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 16px;
    font-size: 13px;
    color: var(--text-2);
  }
  .grades {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin: 8px 0;
  }
  details summary {
    cursor: pointer;
    font-size: 12px;
    color: var(--text-2);
  }
  details p {
    margin-top: 6px;
    color: var(--text-2);
  }
  @container solverpage (max-width: 900px) {
    .body {
      grid-template-columns: minmax(0, 1fr);
    }
    .right {
      display: grid;
      grid-template-columns: 1fr 1fr;
    }
    .right > :first-child {
      grid-column: 1 / -1;
    }
  }
</style>
