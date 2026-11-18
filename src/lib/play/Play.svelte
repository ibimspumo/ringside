<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { solver } from "../solver/store.svelte";
  import Term from "../solver/Term.svelte";
  import { Playback, SPEEDS, nudgeSpeed, setSpeed, speed, stepMs, type Speed } from "../table/playback.svelte";
  import BestHand from "../ui/BestHand.svelte";
  import Button from "../ui/Button.svelte";
  import CardText from "../ui/CardText.svelte";
  import GradeBadge from "../ui/GradeBadge.svelte";
  import Kbd from "../ui/Kbd.svelte";
  import SegmentedControl from "../ui/SegmentedControl.svelte";
  import { playApi } from "./api";
  import InfoPanel from "./InfoPanel.svelte";
  import { GRADE, GRADES, choiceColors, choiceLabel, configLine, etaText, num, resultLine, signedBB } from "./labels";
  import { openSolverTab } from "./nav.svelte";
  import PoolChip from "./PoolChip.svelte";
  import { act, askHint, listen, nextHand, prepare, repeatHand, resume, saveConfig, session, stats, type SessionHand } from "./session.svelte";
  import SessionStats from "./SessionStats.svelte";
  import Start from "./Start.svelte";
  import { situation, streetHead } from "./story";
  import Table from "./Table.svelte";
  import Timeline from "./Timeline.svelte";
  import type { HandView, PlayConfig, Street, Verdict } from "./types";
  import VerdictCard from "./VerdictCard.svelte";

  const wide = (px: number) => typeof window !== "undefined" && window.innerWidth >= px;
  let leftOpen = $state(wide(1500));
  let rightOpen = $state(wide(1220));

  onMount(() => {
    listen();
    resume();
    prepare($state.snapshot(session.config));
    return () => {
      pb.dispose();
      playApi.away();
    };
  });

  /** The hand on the table: the live one, or one from the session list under review. */
  const view = $derived(session.review?.view ?? session.hand);
  const live = $derived(!session.review);
  const last = $derived(view ? view.snapshots.length - 1 : 0);

  // Step by step: the table follows a cursor over the snapshots; a new street pauses longer.
  const pb = new Playback((step) => (view?.events.some((e) => e.type === "board" && e.snapshot === step) ? 1.8 : 1));
  let shownView: HandView | null = null;
  $effect(() => {
    const v = view;
    if (!v) return;
    const end = v.snapshots.length - 1;
    untrack(() => {
      const prev = shownView;
      shownView = v;
      if (session.review || !prev) {
        pb.jump(end);
        return;
      }
      // The same hand after a move plays on from where the table is; a new one starts at the deal.
      const goesOn = prev.seed === v.seed && v.verdicts.length > prev.verdicts.length && v.snapshots.length > prev.snapshots.length;
      if (goesOn) pb.jump(Math.min(pb.cursor, prev.snapshots.length - 1));
      else pb.jump(0);
      pb.play(end);
    });
  });

  const at = $derived(Math.min(session.look ?? pb.cursor, last));
  const upto = $derived(session.look === null ? at : last);
  const over = $derived(!!view?.outcome);
  const settled = $derived(!pb.running && session.look === null);
  const finalShown = $derived(over && at === last);
  const decision = $derived(live && settled ? (view?.decision ?? null) : null);
  const s = $derived(
    stats(session.hand && !session.hand.outcome ? [...session.hands, { n: 0, view: session.hand, repeat: false }] : session.hands),
  );
  const ms = $derived(stepMs());

  /** Your moves up to the step shown, with the event that made each. */
  const moves = $derived.by(() => {
    if (!view) return [];
    const out: { v: Verdict; n: number; snapshot: number; street: Street }[] = [];
    for (const e of view.events) if (e.type === "action" && e.verdict !== null && e.snapshot <= upto) out.push({ v: view.verdicts[e.verdict], n: out.length + 1, snapshot: e.snapshot, street: e.street });
    return out;
  });
  /**
   * The card on top belongs to the move just made, while the others act. Once you are to
   * act again, or the hand is over, it moves into the list of your moves.
   */
  const current = $derived.by(() => {
    if (!view || decision || !moves.length) return null;
    if (session.look !== null) return moves.findLast((m) => m.snapshot <= session.look!) ?? null;
    if (over && !pb.running) return null;
    return moves[moves.length - 1];
  });
  let open = $state<number | null>(null);
  $effect(() => {
    void view?.seed;
    open = null;
  });

  const sit = $derived(decision && view ? situation(view, at) : null);
  const head = $derived(view ? streetHead(view, at) : null);
  const infoLesson = $derived(decision && session.hint ? session.hint : (current?.v ?? moves[moves.length - 1]?.v ?? null));
  const colors = $derived(decision ? choiceColors(decision.choices) : []);
  const canSolver = $derived(over && live && view?.spot?.source === "solver" && view.verdicts.some((v) => v.path));

  const summary = $derived.by(() => {
    if (!view) return "";
    const graded = view.verdicts.filter((v) => v.judgement);
    if (!view.verdicts.length) return "";
    const parts = GRADES.map((g) => ({ g, k: graded.filter((v) => v.judgement!.grade === g).length })).filter((x) => x.k);
    const loss = graded.reduce((a, v) => a + (v.judgement!.loss_bb ?? 0), 0);
    const word = view.verdicts.length === 1 ? "Zug" : "Züge";
    const list = parts.map((x) => `${x.k}× ${GRADE[x.g].label.toLowerCase().replace(" (mix)", "")}`).join(", ");
    return `${view.verdicts.length} ${word}: ${list || "nicht bewertet"}${loss > 0.005 ? ` (−${num(loss, 2)} BB)` : ""}`;
  });
  const net = $derived(view?.outcome ? view.outcome.net[view.hero] : 0);
  /** Mean points of the hand's graded moves, 0–100. */
  const score = $derived.by(() => {
    const p = view?.verdicts.map((v) => v.points).filter((x): x is number => x !== null) ?? [];
    return p.length ? p.reduce((a, b) => a + b, 0) / p.length : null;
  });
  const STREETS: Street[] = ["preflop", "flop", "turn", "river"];
  const STREET_NAME: Record<Street, string> = { preflop: "Preflop", flop: "Flop", turn: "Turn", river: "River" };
  /** Per street the weakest of your graded moves there. */
  const streetGrades = $derived(
    STREETS.map((st) => {
      const gs = moves.filter((m) => m.street === st && m.v.judgement).map((m) => GRADES.indexOf(m.v.judgement!.grade));
      return { st, grade: gs.length ? GRADES[Math.max(...gs)] : null, played: moves.some((m) => m.street === st) || (view?.events.some((e) => e.street === st) ?? false) };
    }),
  );

  // Your buttons and the end of the hand come into view when they appear.
  let dock = $state<HTMLElement>();
  $effect(() => {
    if (!(decision || (over && settled)) || !dock) return;
    dock.scrollIntoView({ block: "nearest", behavior: ms ? "smooth" : "auto" });
  });

  function start(c: PlayConfig) {
    saveConfig(c);
    session.hands = [];
    nextHand();
  }

  function look(snapshot: number | null) {
    pb.skip();
    session.look = snapshot === null || snapshot >= last ? null : snapshot;
  }

  function stepBy(d: number) {
    look(Math.max(0, Math.min(last, at + d)));
  }

  function review(h: SessionHand) {
    session.review = session.hand && h.view === session.hand ? null : h;
    session.look = null;
  }

  async function openInSolver() {
    try {
      const o = await playApi.openSolver();
      solver.run += 1;
      solver.spot = o.spot;
      solver.solved = o.spot;
      solver.summary = o.summary;
      solver.status = "solved";
      solver.error = null;
      solver.progress = null;
      solver.positions = { oop: o.oop, ip: o.ip };
      solver.notes = { oop: "", ip: "" };
      solver.custom = { oop: false, ip: false };
      solver.origin = `aus dem Preflop-Modell, ${session.hand?.players ?? 6} Spieler, ${num(session.hand?.stack_bb ?? 40, 0)} BB`;
      solver.source = "Hände spielen: deine teuerste Entscheidung";
      solver.hero = o.hero ? { player: o.hero[0], cards: o.hero[1] } : null;
      solver.labels = {};
      solver.solvedTable = { players: session.hand?.players ?? 6, positions: { oop: o.oop, ip: o.ip }, labels: {}, run: solver.run };
      solver.replay = {
        line: o.line.map((c) => ({ step: c.step, player: null, label: c.kind ?? "Karte", street: "flop", kind: c.kind ?? undefined, amount: c.amount })),
        then: null,
      };
      solver.view = "result";
      session.hand = null;
      session.playing = false;
      openSolverTab();
    } catch (e) {
      session.error = String(e);
    }
  }

  function onKey(e: KeyboardEvent) {
    if (!session.playing || e.metaKey || e.ctrlKey || e.altKey) return;
    const t = e.target as HTMLElement | null;
    if (t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.isContentEditable)) return;
    const onButton = t?.tagName === "BUTTON" || t?.getAttribute("role") === "radio";
    if (e.key === "+" || e.key === "-") {
      e.preventDefault();
      nudgeSpeed(e.key === "+" ? 1 : -1);
    } else if (e.key === " " && pb.running) {
      e.preventDefault();
      pb.skip();
    } else if (/^[1-9]$/.test(e.key) && decision) {
      const k = Number(e.key) - 1;
      if (k < decision.choices.length) {
        e.preventDefault();
        act(k);
      }
    } else if ((e.key === " " || e.key === "Enter") && !onButton && (over || session.review)) {
      e.preventDefault();
      nextHand();
    } else if ((e.key === "ArrowLeft" || e.key === "ArrowRight") && !onButton) {
      e.preventDefault();
      stepBy(e.key === "ArrowLeft" ? -1 : 1);
    } else if ((e.key === "r" || e.key === "R") && over && live) {
      e.preventDefault();
      repeatHand();
    } else if ((e.key === "h" || e.key === "H") && decision && !decision.hinted) {
      e.preventDefault();
      askHint();
    } else if (e.key === "Escape" && (session.look !== null || session.review)) {
      session.look = null;
      session.review = null;
    }
  }

  const resumePool = () => prepare($state.snapshot(session.config));
  const speedOptions = SPEEDS.map((x) => ({ value: x.id, label: x.label, hint: x.id === "instant" ? "Nur die Endstände, ohne Animation" : undefined }));
</script>

<svelte:window onkeydown={onKey} />

<div class="play">
  {#if !session.playing || !view}
    <Start config={session.config} onChange={(c) => (saveConfig(c), prepare(c))} onStart={start} />
    <div class="foot"><PoolChip pool={session.pool} onCancel={playApi.poolCancel} onResume={resumePool} /></div>
    {#if session.error}<p class="error" role="alert">{session.error}</p>{/if}
  {:else}
    <div class="top">
      <button class="btn ghost back" onclick={() => (session.playing = false)} title="Zurück zur Auswahl">
        <svg viewBox="0 0 24 24" width="15" height="15" aria-hidden="true"><path d="M19 12H5M11 6l-6 6 6 6" /></svg>
        <span>{configLine(session.config)}</span>
      </button>
      <PoolChip pool={session.pool} onCancel={playApi.poolCancel} onResume={resumePool} />
    </div>

    <div class="body" class:left={leftOpen} class:right={rightOpen}>
      <aside class="side l" aria-label="Session">
        {#if leftOpen}
          <div class="side-head"><b>Session</b><button class="fold" onclick={() => (leftOpen = false)} aria-label="Session-Statistik einklappen">‹</button></div>
          <SessionStats {s} />
        {:else}
          <button class="rail" onclick={() => (leftOpen = true)} aria-label="Session-Statistik aufklappen">
            <span class="num">{s.score === null ? "–" : Math.round(s.score)}</span><span class="vt">Session</span>
          </button>
        {/if}
      </aside>

      <main class="center">
        <Timeline {view} {at} {upto} onLook={look} />

        {#if session.review}
          <div class="banner">
            <span>Rückblick auf Hand #{session.review.n}</span>
            <button class="link" onclick={() => (session.review = null)}>Zur aktuellen Hand (Esc)</button>
          </div>
        {:else if view.waiting && live}
          <div class="banner">
            <span>Spot wird gerechnet{session.pool?.next_secs ? `, ${etaText(session.pool.next_secs)}` : ""} – solange spielst du eine Preflop-Hand.</span>
          </div>
        {/if}

        <div class="tablebar">
          <p class="head" aria-live="polite">
            {#if head}
              <b>{head.street}</b> · {head.players}: Du ({view.positions[view.hero]},
              <Term term={head.inPosition ? "ip" : "oop"}>{head.inPosition ? "mit Position" : "ohne Position"}</Term>) gegen {head.others}
              · Pot <span class="num">{num(head.pot)} BB</span> · <Term term="effective">effektiv</Term>
              <span class="num">{num(head.effective, 0)} BB</span>
            {:else}
              <b>Preflop</b> · {view.players} Spieler · <span class="num">{num(view.stack_bb, 0)} BB</span> Stacks
            {/if}
          </p>
          <div class="speed">
            <span class="faint">Tempo</span>
            <SegmentedControl size="sm" label="Tempo der Animation" options={speedOptions} value={speed.id} onchange={(v: Speed) => setSpeed(v)} />
          </div>
        </div>

        <!-- A click on the table shows the end of the running animation. -->
        <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
        <div class="tablebox" onclick={() => pb.running && pb.skip()}>
          <Table {view} {at} final={finalShown} {ms} />
          {#if pb.running}<span class="skip faint">Überspringen <Kbd keys="Leertaste" /></span>{/if}
        </div>

        <div class="dock" bind:this={dock}>
          {#if decision && sit}
            <p class="situation" aria-live="polite">
              <span class="lead">
                {sit.lead}{#if sit.raise}&nbsp;(<Term term="threebet">{sit.raise}</Term>){/if}{sit.lead && sit.folds ? ", " : ""}{sit.folds}.
              </span>
              <span><CardText text={sit.you} /></span>
              {#if sit.need !== null}
                <span class="odds"><Term term="potodds">Pot Odds</Term>: Ein Call braucht rund <b class="num">{Math.round(sit.need * 100)} %</b> Equity.</span>
              {/if}
            </p>
            <div class="actions" role="group" aria-label="Deine Entscheidung">
              {#each decision.choices as c, k (k)}
                {@const l = choiceLabel(c)}
                <button class="act" style:--c={colors[k]} onclick={() => act(k)} disabled={session.busy}>
                  <Kbd keys={String(k + 1)} />
                  <b>{l.label}</b>
                  {#if l.size}<small class="num">{l.size}</small>{/if}
                </button>
              {/each}
              <Button variant="quiet" onclick={askHint} disabled={decision.hinted || session.busy} title="Zeigt die Strategie für deine Hand. Der Zug zählt dann nur halb." kbd="H">
                {decision.hinted ? "Tipp angezeigt" : "Tipp anzeigen"}
              </Button>
            </div>
          {:else if session.look !== null}
            <div class="banner">
              <span>Du siehst Schritt {at + 1} von {last + 1}. Mit <Kbd keys="←" /> <Kbd keys="→" /> gehst du weiter.</span>
              <button class="link" onclick={() => look(null)}>Zur aktuellen Stelle (Esc)</button>
            </div>
          {/if}

          {#if over && settled}
            <section class="end" class:review={!live} aria-label="Ergebnis der Hand">
              <div class="endrow">
                {#if score !== null}
                  <div class="ring" style:--p={score} style:--c={score >= 85 ? "var(--grade-best)" : score >= 50 ? "var(--grade-inaccurate)" : "var(--grade-blunder)"} role="img" aria-label="Punkte dieser Hand: {Math.round(score)} von 100">
                    <b class="num">{Math.round(score)}</b><small>Punkte</small>
                  </div>
                {/if}
                <div class="endtext">
                  <p class="result" class:win={net > 0.04} class:loss={net < -0.04}>
                    <span class="amount num">{signedBB(net)}</span>
                    <span class="text">{resultLine(view)}</span>
                  </p>
                  {#if summary}<p class="summary">{summary}</p>{/if}
              {#if view.outcome?.reason === "showdown" && view.board.length === 5}
                {@const o = view.outcome}
                <div class="showdown" aria-label="Showdown">
                  <div class="who">
                    <span class="faint">Du</span>
                    <BestHand hole={view.hero_cards} board={view.board} winner={o.winners.includes(view.hero)} size={30} />
                  </div>
                  {#each o.shown as c, s (s)}
                    {#if c}
                      <div class="who">
                        <span class="faint">{view.positions[s]}</span>
                        <BestHand hole={c} board={view.board} winner={o.winners.includes(s)} size={30} />
                      </div>
                    {/if}
                  {/each}
                </div>
              {/if}
                  <ol class="streets" aria-label="Deine Züge je Straße">
                    {#each streetGrades as g (g.st)}
                      <li class:off={!g.played} style:--g={g.grade ? GRADE[g.grade].color : "var(--line-2)"} title={g.grade ? `${STREET_NAME[g.st]}: ${GRADE[g.grade].label}` : undefined}>
                        {#if g.grade}<i aria-hidden="true">{GRADE[g.grade].mark}</i>{/if}{STREET_NAME[g.st]}
                      </li>
                    {/each}
                  </ol>
                </div>
              </div>
              <div class="buttons">
                <Button variant="primary" size="lg" onclick={nextHand} disabled={session.busy} kbd="Leertaste">Nächste Hand</Button>
                {#if live}<Button variant="quiet" onclick={repeatHand} disabled={session.busy} kbd="R">Hand wiederholen</Button>{/if}
                {#if canSolver}<Button variant="quiet" onclick={openInSolver}>Im Solver öffnen</Button>{/if}
              </div>
            </section>
          {/if}

          {#if decision && session.hint}
            <VerdictCard {view} lesson={session.hint} title="Tipp für deine Hand" />
          {/if}
          {#if current}
            {#key `${view.seed}:${current.n}`}
              <VerdictCard {view} lesson={current.v} n={current.n} />
            {/key}
          {/if}

          {#if moves.length && (decision || (over && settled) || moves.length > 1)}
            {@const listed = moves.filter((m) => m !== current)}
            {#if listed.length}
              <section class="moves" aria-label="Deine Züge in dieser Hand">
                <h3>Deine Züge in dieser Hand</h3>
                <ol>
                  {#each listed as m (m.n)}
                    {@const c = m.v.choices[m.v.chosen]}
                    {@const l = choiceLabel(c)}
                    <li>
                      <button class="move" class:on={open === m.n} aria-expanded={open === m.n} onclick={() => (open = open === m.n ? null : m.n)}>
                        <span class="n num">{m.n}.</span>
                        <span class="st">{m.street === "preflop" ? "Preflop" : m.street === "flop" ? "Flop" : m.street === "turn" ? "Turn" : "River"}</span>
                        <b>{l.label}{l.size ? ` ${l.size}` : ""}</b>
                        {#if m.v.judgement}
                          <GradeBadge grade={m.v.judgement.grade} cost={m.v.judgement.loss_bb ?? undefined} size="sm" />
                        {:else}
                          <span class="faint">nicht bewertet</span>
                        {/if}
                        <span class="chev" aria-hidden="true">{open === m.n ? "▴" : "▾"}</span>
                      </button>
                      {#if open === m.n}
                        <div class="detail">
                          <VerdictCard {view} lesson={m.v} n={m.n} />
                          <button class="link" onclick={() => look(m.snapshot)}>Am Tisch ansehen</button>
                        </div>
                      {/if}
                    </li>
                  {/each}
                </ol>
              </section>
            {/if}
          {/if}
          {#if session.error}<p class="error" role="alert">{session.error}</p>{/if}
        </div>
      </main>

      <aside class="side r" aria-label="Info">
        {#if rightOpen}
          <div class="side-head"><b>Info</b><button class="fold" onclick={() => (rightOpen = false)} aria-label="Info einklappen">›</button></div>
          <InfoPanel {view} lesson={infoLesson} hands={session.hands} reviewing={session.review?.n ?? null} onOpen={review} />
        {:else}
          <button class="rail" onclick={() => (rightOpen = true)} aria-label="Info aufklappen"><span class="vt">Strategie · Hände</span></button>
        {/if}
      </aside>
    </div>
  {/if}
</div>

<style>
  .play {
    min-width: 0;
    padding: 18px 24px 40px;
  }
  .foot {
    max-width: 1280px;
    margin: 0 auto;
  }
  .top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 10px;
    flex-wrap: wrap;
  }
  .back {
    display: inline-flex;
    gap: 8px;
    color: var(--text-2);
  }
  .back svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
  }
  .body {
    display: grid;
    grid-template-columns: 40px minmax(0, 1fr) 40px;
    gap: 14px;
    align-items: start;
  }
  .body.left {
    grid-template-columns: 236px minmax(0, 1fr) 40px;
  }
  .body.right {
    grid-template-columns: 40px minmax(0, 1fr) clamp(280px, 22vw, 380px);
  }
  .body.left.right {
    grid-template-columns: 236px minmax(0, 1fr) clamp(280px, 22vw, 380px);
  }
  .side {
    position: sticky;
    top: 0;
    padding: 14px;
    border-radius: var(--r);
    background: var(--surface);
    border: 1px solid var(--line);
    min-width: 0;
  }
  .side:has(.rail) {
    padding: 0;
    background: none;
    border: 0;
  }
  .side-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 12px;
    font-size: 13px;
  }
  .fold {
    width: 26px;
    height: 26px;
    border-radius: 6px;
    border: 1px solid var(--line);
    background: none;
    color: var(--text-2);
    cursor: pointer;
    font-size: 15px;
    line-height: 1;
  }
  .rail {
    width: 40px;
    min-height: 180px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    padding: 12px 0;
    border-radius: var(--r);
    border: 1px solid var(--line);
    background: var(--surface);
    color: var(--text-2);
    cursor: pointer;
    font-size: 12px;
  }
  .rail .num {
    font-size: 15px;
    color: var(--text);
  }
  .vt {
    writing-mode: vertical-rl;
    letter-spacing: 0.04em;
  }
  .center {
    display: grid;
    gap: 10px;
    min-width: 0;
    max-width: 1180px;
    width: 100%;
    margin: 0 auto;
  }
  .banner {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
    padding: 8px 12px;
    border-radius: var(--r-sm);
    background: color-mix(in srgb, var(--sd) 12%, var(--surface));
    border: 1px solid color-mix(in srgb, var(--sd) 30%, transparent);
    font-size: 13px;
  }
  .link {
    border: 0;
    background: none;
    color: var(--text);
    text-decoration: underline;
    text-underline-offset: 3px;
    cursor: pointer;
    font-size: 13px;
    padding: 0;
  }
  .tablebar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 8px 16px;
  }
  .head {
    margin: 0;
    font-size: 14px;
    color: var(--text-2);
  }
  .head b {
    color: var(--text);
  }
  .speed {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
  }
  .tablebox {
    position: relative;
  }
  .skip {
    position: absolute;
    right: 8px;
    bottom: 4px;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
  }
  .dock {
    display: grid;
    gap: 12px;
  }
  .situation {
    margin: 0;
    display: grid;
    gap: 2px;
    justify-items: center;
    text-align: center;
    font-size: 14px;
    color: var(--text-2);
  }
  .situation .lead {
    font-size: 15px;
    color: var(--text);
    font-weight: 600;
  }
  .odds {
    font-size: 12.5px;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    align-items: stretch;
    justify-content: center;
  }
  .act {
    position: relative;
    flex: 1 1 0;
    max-width: 210px;
    min-width: 128px;
    display: grid;
    grid-template-columns: auto 1fr;
    grid-template-rows: auto auto;
    column-gap: 10px;
    align-items: center;
    min-height: 58px;
    padding: 8px 14px 8px 12px;
    border-radius: var(--r);
    border: 1px solid color-mix(in srgb, var(--c) 65%, transparent);
    background: color-mix(in srgb, var(--c) 20%, var(--surface));
    color: var(--text);
    text-align: left;
    cursor: pointer;
    transition:
      background var(--dur-fast) var(--ease-standard),
      transform var(--dur-fast) var(--ease-standard);
  }
  .act:hover:not(:disabled) {
    background: color-mix(in srgb, var(--c) 34%, var(--surface));
  }
  .act:active:not(:disabled) {
    transform: translateY(1px);
  }
  .act:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: 2px;
  }
  .act :global(kbd) {
    grid-row: span 2;
  }
  .act b {
    font-size: 16px;
  }
  .act small {
    color: var(--text-2);
    font-size: 12.5px;
    white-space: nowrap;
  }
  .end {
    display: grid;
    gap: 8px;
    padding: 16px 18px;
    border-radius: var(--r);
    background: var(--surface);
    border: 1px solid var(--line-2);
  }
  .endrow {
    display: flex;
    align-items: center;
    gap: 18px;
  }
  .endtext {
    display: grid;
    gap: 6px;
    min-width: 0;
  }
  .ring {
    flex: none;
    display: grid;
    place-items: center;
    align-content: center;
    width: 84px;
    height: 84px;
    border-radius: 50%;
    background:
      radial-gradient(closest-side, var(--surface) 80%, transparent 81%),
      conic-gradient(var(--c) calc(var(--p) * 1%), var(--surface-3) 0);
  }
  .ring b {
    font-size: 24px;
    line-height: 1;
    color: var(--c);
  }
  .ring small {
    font-size: 10.5px;
    color: var(--text-3);
  }
  .streets {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .streets li {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 24px;
    padding: 0 9px;
    border-radius: var(--r-sm);
    background: var(--surface-2);
    box-shadow: inset 0 -2px 0 var(--g);
    font-size: 11.5px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }
  .streets li.off {
    color: var(--text-3);
    box-shadow: none;
  }
  .streets i {
    font-style: normal;
    color: var(--g);
  }
  .result {
    margin: 0;
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 6px 14px;
  }
  .amount {
    font-size: 30px;
    font-weight: 700;
    letter-spacing: -0.01em;
  }
  .result.win .amount {
    color: var(--pos);
  }
  .result.loss .amount {
    color: var(--neg);
  }
  .result .text {
    font-size: 16px;
    font-weight: 600;
  }
  .showdown {
    display: flex;
    flex-wrap: wrap;
    gap: 12px 28px;
  }
  .showdown .who {
    display: grid;
    gap: 4px;
  }
  .summary {
    margin: 0;
    color: var(--text-2);
    font-size: 13.5px;
  }
  .buttons {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
    margin-top: 4px;
  }
  .moves h3 {
    margin: 0 0 6px;
    font-size: 12px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-3);
  }
  .moves ol {
    display: grid;
    gap: 6px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .move {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    min-height: 38px;
    padding: 4px 12px;
    border-radius: var(--r-sm);
    border: 1px solid var(--line);
    background: var(--surface);
    color: var(--text);
    font-size: 13px;
    text-align: left;
    cursor: pointer;
  }
  .move:hover,
  .move.on {
    border-color: var(--line-2);
    background: var(--surface-2);
  }
  .move .n {
    color: var(--text-3);
  }
  .move .st {
    color: var(--text-3);
    font-size: 12px;
    min-width: 48px;
  }
  .move b {
    font-weight: 600;
    margin-right: auto;
  }
  .chev {
    color: var(--text-3);
  }
  .detail {
    display: grid;
    gap: 6px;
    justify-items: start;
    padding: 8px 0 4px;
  }
  .error {
    color: var(--neg);
    margin: 0;
  }
  @media (prefers-reduced-motion: reduce) {
    .act {
      transition: none;
    }
  }
</style>
