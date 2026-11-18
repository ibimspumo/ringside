<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { preview } from "../learn/preview";
  import { solver } from "../solver/store.svelte";
  import Term from "../solver/Term.svelte";
  import { Playback, SPEEDS, nudgeSpeed, setSpeed, speed, stepMs, type Speed } from "../table/playback.svelte";
  import BestHand from "../ui/BestHand.svelte";
  import Button from "../ui/Button.svelte";
  import CardText from "../ui/CardText.svelte";
  import GradeBadge from "../ui/GradeBadge.svelte";
  import IconButton from "../ui/IconButton.svelte";
  import Kbd from "../ui/Kbd.svelte";
  import MiniCard from "../ui/MiniCard.svelte";
  import { usePageHead } from "../ui/page.svelte";
  import ProgressRing from "../ui/ProgressRing.svelte";
  import SegmentedControl from "../ui/SegmentedControl.svelte";
  import { playApi } from "./api";
  import InfoPanel from "./InfoPanel.svelte";
  import { GRADE, GRADES, UNRATED_SHORT, choiceColors, choiceLabel, configLine, etaText, freeLine, madeOf, num, resultLine, signedBB, solvingLine } from "./labels";
  import { openSolverTab } from "./nav.svelte";
  import PoolChip from "./PoolChip.svelte";
  import { act, askHint, cancelLive, listen, nextHand, prepare, repeatHand, resume, saveConfig, session, stats, type SessionHand } from "./session.svelte";
  import SessionStats from "./SessionStats.svelte";
  import Start from "./Start.svelte";
  import { situation, streetHead } from "./story";
  import Table from "./Table.svelte";
  import Timeline from "./Timeline.svelte";
  import type { HandView, PlayConfig, Street, Verdict } from "./types";
  import VerdictCard from "./VerdictCard.svelte";

  usePageHead(() => ({
    sub:
      session.playing && session.hand
        ? "Du entscheidest, Ringside bewertet jeden Zug sofort."
        : "Zufällige Hände, endlos. Nach jedem Zug siehst du, was die Lösung tun würde.",
  }));

  const wide = (px: number) => typeof window !== "undefined" && window.innerWidth >= px;
  let leftOpen = $state(wide(1500));
  let rightOpen = $state(wide(1220));

  onMount(() => {
    listen();
    resume();
    prepare($state.snapshot(session.config));
    // Browser demo, for screenshots: `players=<n>` deals a hand at that table, `act=2,0` plays your moves.
    const players = Number(preview("players"));
    if (players) {
      saveConfig({ ...session.config, mode: "full", players, stack_bb: 40, mixed_stacks: false, hero_seat: null });
      session.hands = [];
      void (async () => {
        await nextHand();
        for (const k of (preview("act") ?? "").split(",").filter(Boolean)) await act(Number(k));
      })();
    }
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
      // The same hand after a move or a live solve plays on from where the table is; a new one
      // starts at the deal.
      const goesOn = prev.seed === v.seed && (v.verdicts.length > prev.verdicts.length || !!prev.solving) && v.snapshots.length >= prev.snapshots.length;
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
  const canSolver = $derived(over && live && (view?.spot?.source === "solver" || view?.spot?.source === "live") && view.verdicts.some((v) => v.path));
  /** The hand waits for a live solve, and the table has caught up with it. */
  const solving = $derived(live && settled && view?.solving ? view.solving : null);
  const liveRun = $derived(session.live?.status === "running" ? session.live : null);
  const solvingDetail = $derived.by(() => {
    if (!solving) return "";
    const state = session.live?.status === "paused_client" ? " · pausiert, solange PokerStars läuft" : liveRun?.secs_left ? ` · ${etaText(liveRun.secs_left)}` : "";
    return `Schnell gerechnet, Genauigkeit ${num(solving.target_pct)} % des Pots${state}. Danach geht die Hand weiter.`;
  });
  const STREET_ORDER: Street[] = ["preflop", "flop", "turn", "river"];
  /** Once the table shows the street where the hand left the solutions. */
  const freeShown = $derived(!!view?.free && !!head && STREET_ORDER.indexOf(view.snapshots[at].street) >= STREET_ORDER.indexOf(view.free.street));

  const summary = $derived.by(() => {
    if (!view) return "";
    const graded = view.verdicts.filter((v) => v.judgement);
    if (!view.verdicts.length) return "";
    const parts = GRADES.map((g) => ({ g, k: graded.filter((v) => v.judgement!.grade === g).length })).filter((x) => x.k);
    const loss = graded.reduce((a, v) => a + (v.judgement!.loss_bb ?? 0), 0);
    const word = view.verdicts.length === 1 ? "Zug" : "Züge";
    const list = parts.map((x) => `${x.k}× ${GRADE[x.g].label.replace(" (mix)", "")}`).join(", ");
    return `${view.verdicts.length} ${word}: ${list || "nicht bewertet"}${loss > 0.005 ? ` · auf lange Sicht verschenkt: ${num(loss, 2)} BB` : ""}`;
  });
  const net = $derived(view?.outcome ? view.outcome.net[view.hero] : 0);

  /** Your move in words with what it cost against the better one, e.g. "Check am Flop: Ungenau – Bet wäre 0,25 BB besser". */
  function verdictWords(v: Verdict, street: Street): { text: string; color: string } | null {
    const j = v.judgement;
    if (!j) return null;
    const mine = choiceLabel(v.choices[v.chosen], street !== "preflop");
    const head = `${mine.label} ${street === "preflop" ? "vor dem Flop" : `am ${STREET_NAME[street]}`}`;
    const better = j.best !== v.chosen && j.loss_bb !== null && j.loss_bb > 0.005 ? ` – ${choiceLabel(v.choices[j.best]).label} wäre ${num(j.loss_bb, 2)} BB besser` : "";
    return { text: `${head}: ${GRADE[j.grade].label}${better}.`, color: GRADE[j.grade].color };
  }
  /** While you decide anew: how your last move went. */
  const lastWord = $derived.by(() => {
    if (!decision || !moves.length) return null;
    const m = moves[moves.length - 1];
    const w = verdictWords(m.v, m.street);
    return w ? { ...w, text: `Dein letzter Zug – ${w.text}` } : null;
  });
  /** At the end the lesson of the hand: your most expensive move that was not right. */
  const lesson = $derived.by(() => {
    const bad = moves.filter((m) => m.v.judgement && !["best", "mix", "good"].includes(m.v.judgement.grade));
    if (!over || !bad.length) return null;
    const m = bad.reduce((a, b) => ((b.v.judgement!.loss_bb ?? 0) > (a.v.judgement!.loss_bb ?? 0) ? b : a));
    const j = m.v.judgement!;
    const main = m.v.choices[j.main];
    const share = m.v.strategy ? ` (${Math.round((m.v.strategy[j.main] ?? 0) * 100)} %)` : "";
    const w = verdictWords(m.v, m.street)!;
    return { text: `${w.text} Die Lösung spielt hier meist ${choiceLabel(main, m.street !== "preflop").label}${share}.`, color: w.color };
  });
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
  // Your buttons, or at the end the answer, sit at the bottom edge when they would be cut off or
  // the history above the table is out of sight: then as much as fits shows above them.
  let actionsEl = $state<HTMLElement>();
  let endEl = $state<HTMLElement>();
  let tableEl = $state<HTMLElement>();
  $effect(() => {
    const target = decision ? actionsEl : over && settled ? endEl : null;
    if (!target || !tableEl) return;
    const box = scrollParent(tableEl)?.getBoundingClientRect() ?? { top: 0, bottom: window.innerHeight };
    const r = target.getBoundingClientRect();
    const top = (tableEl.parentElement ?? tableEl).getBoundingClientRect().top;
    if (top < box.top - 1 || r.bottom > box.bottom) target.scrollIntoView({ block: "end", behavior: ms ? "smooth" : "auto" });
  });

  function scrollParent(el: HTMLElement): HTMLElement | null {
    for (let p = el.parentElement; p; p = p.parentElement) if (/(auto|scroll)/.test(getComputedStyle(p).overflowY)) return p;
    return null;
  }

  function start(c: PlayConfig) {
    saveConfig(c);
    session.hands = [];
    // The last table may have another size: it must not stand there while the new hand is dealt.
    session.hand = null;
    session.review = null;
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
    } else if ((e.key === "h" || e.key === "H") && decision && decision.graded && !decision.hinted) {
      e.preventDefault();
      askHint();
    } else if (e.key === "Escape" && solving) {
      e.preventDefault();
      cancelLive();
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
    <Start config={session.config} busy={session.busy} onChange={(c) => (saveConfig(c), prepare(c))} onStart={start} />
    <div class="foot"><PoolChip pool={session.pool} onCancel={playApi.poolCancel} onResume={resumePool} /></div>
    {#if session.error}<p class="error" role="alert">{session.error}</p>{/if}
  {:else}
    <div class="top">
      <button class="btn ghost back" onclick={() => (session.playing = false)} title="Zurück zur Auswahl">
        <svg viewBox="0 0 24 24" width="15" height="15" aria-hidden="true"><path d="M19 12H5M11 6l-6 6 6 6" /></svg>
        <span>{configLine(session.config)}</span>
      </button>
      <div class="topend">
        <div class="speed">
          <span class="faint">Tempo</span>
          <SegmentedControl size="sm" label="Tempo der Animation" options={speedOptions} value={speed.id} onchange={(v: Speed) => setSpeed(v)} />
        </div>
        <PoolChip compact pool={session.pool} onCancel={playApi.poolCancel} onResume={resumePool} />
      </div>
    </div>

    <div class="body" class:left={leftOpen} class:right={rightOpen}>
      <aside class="side l" aria-label="Session">
        {#if leftOpen}
          <div class="side-head"><b>Session</b><IconButton size="sm" label="Session-Statistik einklappen" onclick={() => (leftOpen = false)}><svg viewBox="0 0 24 24" width="14" height="14" aria-hidden="true"><path d="M15 6l-6 6 6 6" /></svg></IconButton></div>
          <SessionStats {s} />
        {:else}
          <button class="rail" onclick={() => (leftOpen = true)} aria-label="Session-Statistik aufklappen">
            <span class="score num">{s.score === null ? "–" : Math.round(s.score)}</span><small>Punkte</small><span class="vt">Session · {s.hands} {s.hands === 1 ? "Hand" : "Hände"}</span>
          </button>
        {/if}
      </aside>

      <main class="center">
        <div class="history"><Timeline {view} {at} {upto} onLook={look} /></div>

        {#if session.review}
          <div class="banner">
            <span>Rückblick auf Hand #{session.review.n}</span>
            <Button variant="quiet" size="sm" kbd="Esc" onclick={() => (session.review = null)}>Zur aktuellen Hand</Button>
          </div>
        {:else if view.free && freeShown}
          <div class="banner" role="note">
            <span>{freeLine(view.free)}</span>
          </div>
        {:else if view.waiting && live && !view.spot && !view.solving}
          <div class="banner">
            <span>Kein Spot vorbereitet – erreicht die Hand einen Heads-up-Flop, rechnet Ringside ihn live.</span>
          </div>
        {/if}

        <div class="tablebar">
          <p class="head" aria-live="polite">
            {#if head}
              <b>{head.street}</b> · {head.players}: Du ({view.positions[view.hero]},
              <Term term={head.inPosition ? "ip" : "oop"}>{head.inPosition ? "mit Position" : "ohne Position"}</Term>) gegen {head.others}
              · Pot <span class="num">{num(head.pot)} BB</span> · <Term term="effective">effektiv</Term>
              <span class="num">{num(head.effective, 0)} BB</span>
              {#if view.spot?.source === "live" && view.snapshots[at].street !== "preflop"}
                · <span class="live">live gerechnet, Genauigkeit <span class="num">{num(view.spot.exploitability_pct)} %</span></span>
              {/if}
            {:else}
              <b>Preflop</b> · {view.players} Spieler · <span class="num">{num(view.stack_bb, 0)} BB</span> Stacks
            {/if}
          </p>
        </div>

        <!-- A click on the table shows the end of the running animation. -->
        <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
        <div class="tablebox" bind:this={tableEl} onclick={() => pb.running && pb.skip()}>
          <Table {view} {at} final={finalShown} {ms} solving={solving ? (liveRun?.fraction ?? null) : undefined} />
          {#if pb.running}<span class="skip faint">Überspringen <Kbd keys="Leertaste" /></span>{/if}
        </div>

        <div class="dock">
          {#if solving}
            <div class="solving" role="status" aria-live="polite" data-part="solving">
              <ProgressRing value={liveRun?.fraction ?? null} size={44} label="Spot wird gerechnet">
                {#if liveRun}<span class="num">{Math.round(liveRun.fraction * 100)}</span>{/if}
              </ProgressRing>
              <div class="solving-text">
                <b>{solvingLine(solving.why)}</b>
                <span class="faint">{solvingDetail}</span>
              </div>
              <Button variant="quiet" onclick={cancelLive} disabled={session.busy} kbd="Esc" title="Die Hand geht ohne Lösung weiter; deine Züge ab hier werden nicht bewertet.">Abbrechen</Button>
            </div>
          {:else if decision && sit}
            <p class="situation" aria-live="polite">
              <span class="lead">
                {#if sit.board}<CardText text={sit.board} />.&nbsp;{/if}{sit.lead}{#if sit.raise}&nbsp;(<Term term="threebet">{sit.raise}</Term>){/if}{sit.lead && sit.folds ? ", " : ""}{sit.folds}.
              </span>
              <span><CardText text={sit.you} /></span>
              {#if lastWord}<span class="last" style:--g={lastWord.color}>{lastWord.text}</span>{/if}
              {#if sit.need !== null}
                <span class="odds"><Term term="potodds">Pot Odds</Term>: Ein Call braucht rund <b class="num">{Math.round(sit.need * 100)} %</b> Equity.</span>
              {/if}
            </p>
            <div class="actions" role="group" aria-label="Deine Entscheidung" bind:this={actionsEl}>
              {#each decision.choices as c, k (k)}
                {@const l = choiceLabel(c, decision.street !== "preflop")}
                <button class="act" style:--c={colors[k]} onclick={() => act(k)} disabled={session.busy}>
                  <Kbd keys={String(k + 1)} />
                  <b>{l.label}</b>
                  {#if l.size}<small class="num">{l.size}</small>{/if}
                </button>
              {/each}
              {#if decision.graded}
                <Button variant="quiet" onclick={askHint} disabled={decision.hinted || session.busy} title="Zeigt die Strategie für deine Hand. Der Zug zählt dann nur halb." kbd="H">
                  {decision.hinted ? "Tipp angezeigt" : "Tipp anzeigen"}
                </Button>
              {:else}
                <span class="unrated faint">Nicht bewertet</span>
              {/if}
            </div>
          {:else if session.look !== null}
            <div class="banner">
              <span>Du siehst Schritt {at + 1} von {last + 1}. Mit <Kbd keys="←" /> <Kbd keys="→" /> gehst du weiter.</span>
              <Button variant="quiet" size="sm" kbd="Esc" onclick={() => look(null)}>Zur aktuellen Stelle</Button>
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
                  <div class="answer">
                  <small class="cap">Ergebnis am Tisch</small>
                  <p class="result" class:win={net > 0.04} class:loss={net < -0.04}>
                    <span class="amount num">{signedBB(net)}</span>
                    <span class="text">{resultLine(view, false)}</span>
                  </p>
                  {#if view.outcome?.reason === "showdown" && view.board.length === 5}
                    <p class="hands" aria-label="Die Hände beim Showdown">
                      {#each [view.hero, ...view.outcome.shown.map((c, s) => (c && s !== view.hero ? s : -1)).filter((s) => s >= 0)] as s (s)}
                        {@const cards = s === view.hero ? view.hero_cards : (view.outcome.shown[s] ?? [])}
                        <span class:won={view.outcome.winners.includes(s)}>
                          <b>{s === view.hero ? "Du" : view.positions[s]}</b>
                          {#each cards as c (c)}<MiniCard card={c} />{/each}
                          {madeOf(view, s)}
                        </span>
                      {/each}
                    </p>
                  {/if}
                  </div>
                  {#if lesson}<p class="lesson" style:--g={lesson.color}>{lesson.text}</p>{/if}
                  {#if summary}<p class="summary"><b>Deine Entscheidungen</b> · {summary}</p>{/if}
                  <ol class="streets" aria-label="Deine Züge je Straße">
                    {#each streetGrades.filter((g) => g.played) as g (g.st)}
                      <li style:--g={g.grade ? GRADE[g.grade].color : "var(--line-2)"} title={g.grade ? `${STREET_NAME[g.st]}: ${GRADE[g.grade].label}` : undefined}>
                        {#if g.grade}<i aria-hidden="true">{GRADE[g.grade].mark}</i>{/if}{STREET_NAME[g.st]}
                      </li>
                    {/each}
                  </ol>
                </div>
                <div class="buttons" bind:this={endEl}>
                  <Button variant="primary" size="lg" onclick={nextHand} disabled={session.busy} kbd="Leertaste">Nächste Hand</Button>
                  {#if live}<Button variant="quiet" onclick={repeatHand} disabled={session.busy} kbd="R">Hand wiederholen</Button>{/if}
                  {#if canSolver}<Button variant="quiet" onclick={openInSolver}>Im Solver öffnen</Button>{/if}
                </div>
              </div>
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
                          <span class="faint">{UNRATED_SHORT[m.v.unrated ?? "not_reached"]}</span>
                        {/if}
                        <span class="chev" aria-hidden="true">{open === m.n ? "▴" : "▾"}</span>
                      </button>
                      {#if open === m.n}
                        <div class="detail">
                          <VerdictCard {view} lesson={m.v} n={m.n} />
                          <Button variant="quiet" size="sm" onclick={() => look(m.snapshot)}>Am Tisch ansehen</Button>
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
          <div class="side-head"><b>Info</b><IconButton size="sm" label="Info einklappen" onclick={() => (rightOpen = false)}><svg viewBox="0 0 24 24" width="14" height="14" aria-hidden="true"><path d="M9 6l6 6-6 6" /></svg></IconButton></div>
          <InfoPanel {view} lesson={infoLesson} past={!!decision && !session.hint} hands={session.hands} reviewing={session.review?.n ?? null} onOpen={review} />
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
    max-width: 1360px;
    margin: 14px auto 0;
  }
  .top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 10px;
    flex-wrap: wrap;
  }
  .topend {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 16px;
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
  .side-head svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
  }
  .rail {
    width: 40px;
    min-height: 180px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    padding: 12px 0;
    border-radius: var(--r);
    border: 1px solid var(--line);
    background: var(--surface);
    color: var(--text-2);
    cursor: pointer;
    font-size: var(--fs-sm);
    transition: border-color var(--dur) var(--ease-standard);
  }
  .rail:hover {
    border-color: var(--line-2);
    color: var(--text);
  }
  .rail .score {
    font-size: var(--fs-lg);
    font-weight: 600;
    color: var(--text);
  }
  .rail small {
    font-size: var(--fs-xs);
    color: var(--text-3);
    margin-bottom: 8px;
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
  .head .live {
    color: var(--info);
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
    container: play-dock / inline-size;
    display: grid;
    gap: 12px;
  }
  .solving {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 14px;
    padding: 12px 16px;
    border-radius: var(--r);
    background: var(--surface);
    border: 1px solid var(--line-2);
  }
  .solving-text {
    display: grid;
    gap: 2px;
    min-width: 0;
    font-size: var(--fs-md);
  }
  .solving-text b {
    font-size: var(--fs-base);
  }
  .unrated {
    align-self: center;
    font-size: var(--fs-sm);
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
    font-size: var(--fs-lg);
    color: var(--text);
    font-weight: 600;
  }
  .last {
    padding-left: 8px;
    border-left: 2px solid var(--g);
    font-size: var(--fs-sm);
  }
  .lesson {
    margin: 0;
    padding: 6px 10px;
    border-left: 3px solid var(--g);
    border-radius: 0 var(--r-sm) var(--r-sm) 0;
    background: color-mix(in srgb, var(--g) 10%, var(--surface-2));
    font-size: var(--fs-base);
    color: var(--text);
  }
  .odds {
    font-size: var(--fs-sm);
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
    min-width: 112px;
    display: grid;
    grid-template-columns: auto 1fr;
    grid-template-rows: auto auto;
    column-gap: 10px;
    align-items: center;
    min-height: 52px;
    padding: 6px 14px 6px 12px;
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
  .act b:last-child {
    grid-row: span 2;
  }
  .act b {
    font-size: var(--fs-lg);
  }
  .act small {
    color: var(--text-2);
    font-size: var(--fs-sm);
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
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: 14px 20px;
  }
  .endrow:not(:has(.ring)) {
    grid-template-columns: minmax(0, 1fr) auto;
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
    font-size: var(--fs-2xl);
    line-height: 1;
    color: var(--c);
  }
  .ring small {
    font-size: var(--fs-xs);
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
    font-size: var(--fs-xs);
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
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
    font-size: var(--fs-4xl);
    font-weight: 700;
    letter-spacing: -0.01em;
  }
  .result.win .amount {
    color: var(--pos);
  }
  .result.loss .amount {
    color: var(--neg);
  }
  .answer {
    display: grid;
    gap: 6px;
  }
  .endtext .cap {
    margin-bottom: -4px;
    font-size: var(--fs-xs);
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-3);
  }
  .hands {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 18px;
    margin: 0;
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .hands span {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .hands b {
    margin-right: 2px;
    color: var(--text);
  }
  .hands .won {
    color: var(--pos);
  }
  .summary b {
    color: var(--text);
    font-weight: 600;
  }
  /* The hand so far stays in sight while the page scrolls to your buttons. */
  .history {
    container: history / inline-size;
  }
  @media (min-height: 820px) {
    .history {
      position: sticky;
      top: 0;
      z-index: 4;
      margin: -6px 0 0;
      padding: 6px 0;
      background: var(--bg);
    }
  }
  .result .text {
    font-size: var(--fs-lg);
    font-weight: 600;
  }
  .showdown {
    display: flex;
    flex-wrap: wrap;
    gap: 12px 28px;
    padding-top: 12px;
    border-top: 1px solid var(--line);
  }
  .showdown .who {
    display: grid;
    gap: 4px;
  }
  .summary {
    margin: 0;
    color: var(--text-2);
    font-size: var(--fs-md);
  }
  .buttons {
    display: grid;
    justify-items: stretch;
    gap: 6px;
  }
  @container play-dock (width < 640px) {
    .endrow {
      grid-template-columns: auto minmax(0, 1fr);
    }
    .ring {
      width: 64px;
      height: 64px;
    }
    .amount {
      font-size: var(--fs-3xl);
    }
    /* Narrow, the answer is followed by the next hand; the marks per move and the hands' names
       are in the history and on the plates already. */
    .endtext {
      display: contents;
    }
    .answer {
      grid-column: 2;
    }
    .lesson {
      order: 1;
      grid-column: 1 / -1;
    }
    .buttons {
      order: 2;
    }
    .summary {
      order: 3;
      grid-column: 1 / -1;
    }
    .streets,
    .hands {
      display: none;
    }
    .buttons {
      grid-column: 1 / -1;
      display: flex;
      flex-wrap: wrap;
    }
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
