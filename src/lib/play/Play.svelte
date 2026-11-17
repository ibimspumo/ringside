<script lang="ts">
  import { onMount } from "svelte";
  import { solver } from "../solver/store.svelte";
  import { playApi } from "./api";
  import InfoPanel from "./InfoPanel.svelte";
  import { GRADE, choiceColors, choiceLabel, choiceShort, configLine, etaText, num, resultLine } from "./labels";
  import { openSolverTab } from "./nav.svelte";
  import PoolChip from "./PoolChip.svelte";
  import { act, askHint, listen, nextHand, prepare, repeatHand, resume, saveConfig, session, stats, type SessionHand } from "./session.svelte";
  import SessionStats from "./SessionStats.svelte";
  import Start from "./Start.svelte";
  import Table from "./Table.svelte";
  import Timeline from "./Timeline.svelte";
  import type { PlayConfig, Verdict } from "./types";
  import VerdictCard from "./VerdictCard.svelte";

  const wide = (px: number) => typeof window !== "undefined" && window.innerWidth >= px;
  let leftOpen = $state(wide(1500));
  let rightOpen = $state(wide(1220));

  onMount(() => {
    listen();
    resume();
    prepare($state.snapshot(session.config));
  });

  /** The hand on the table: the live one, or one from the session list under review. */
  const view = $derived(session.review?.view ?? session.hand);
  const live = $derived(!session.review);
  const last = $derived(view ? view.snapshots.length - 1 : 0);
  const at = $derived(session.look ?? last);
  const over = $derived(!!view?.outcome);
  const decision = $derived(live && session.look === null ? (view?.decision ?? null) : null);
  const s = $derived(
    stats(session.hand && !session.hand.outcome ? [...session.hands, { n: 0, view: session.hand, repeat: false }] : session.hands),
  );

  /** The verdict to show: the one of the moment looked at, else the latest. */
  const shown = $derived.by((): Verdict | null => {
    if (!view?.verdicts.length) return null;
    if (session.look !== null) {
      const e = view.events.find((x) => x.type === "action" && x.verdict !== null && x.snapshot === session.look);
      if (e && e.type === "action" && e.verdict !== null) return view.verdicts[e.verdict];
    }
    return view.verdicts[view.verdicts.length - 1];
  });
  const infoLesson = $derived(decision && session.hint ? session.hint : shown);
  const colors = $derived(decision ? choiceColors(decision.choices) : []);
  const canSolver = $derived(over && live && view?.spot?.source === "solver" && view.verdicts.some((v) => v.path));

  function start(c: PlayConfig) {
    saveConfig(c);
    session.hands = [];
    nextHand();
  }

  function look(snapshot: number | null) {
    session.look = snapshot;
  }

  function review(h: SessionHand) {
    if (session.hand && h.view === session.hand) {
      session.review = null;
    } else {
      session.review = h;
    }
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
    const onButton = t?.tagName === "BUTTON";
    if (/^[1-9]$/.test(e.key) && decision) {
      const k = Number(e.key) - 1;
      if (k < decision.choices.length) {
        e.preventDefault();
        act(k);
      }
    } else if ((e.key === " " || e.key === "Enter") && !onButton && (over || session.review)) {
      e.preventDefault();
      nextHand();
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
        <Timeline {view} {at} onLook={look} />

        {#if session.review}
          <div class="banner">
            <span>Rückblick auf Hand #{session.review.n}</span>
            <button class="link" onclick={() => (session.review = null)}>Zur aktuellen Hand (Esc)</button>
          </div>
        {:else if view.waiting && live}
          <div class="banner">
            <span>
              Spot wird gerechnet{session.pool?.next_secs ? `, ${etaText(session.pool.next_secs)}` : ""} – solange spielst du eine Preflop-Hand.
            </span>
          </div>
        {/if}

        <Table {view} {at} final={over && at === last} />

        <div class="dock">
          {#if decision}
            <div class="actions" role="group" aria-label="Deine Entscheidung">
              {#each decision.choices as c, k (k)}
                {@const l = choiceLabel(c)}
                <button class="act" style:--c={colors[k]} onclick={() => act(k)} disabled={session.busy}>
                  <kbd>{k + 1}</kbd>
                  <b>{l.label}</b>
                  {#if l.size}<small class="num">{l.size}</small>{/if}
                </button>
              {/each}
              <button class="btn ghost hint" onclick={askHint} disabled={decision.hinted || session.busy} title="Zeigt die Strategie für deine Hand. Der Zug zählt dann nur halb.">
                {decision.hinted ? "Tipp angezeigt" : "Tipp anzeigen"} <kbd>H</kbd>
              </button>
            </div>
          {:else if session.look !== null && !over}
            <div class="banner">
              <span>Du siehst einen früheren Moment der Hand.</span>
              <button class="link" onclick={() => look(null)}>Zur aktuellen Stelle (Esc)</button>
            </div>
          {/if}

          {#if over}
            <div class="end" class:review={!live}>
              <p class="result">{resultLine(view)}</p>
              {#if view.verdicts.length}
                <ol class="moves" aria-label="Deine Züge in dieser Hand">
                  {#each view.events as e, i (i)}
                    {#if e.type === "action" && e.verdict !== null}
                      {@const v = view.verdicts[e.verdict]}
                      <li>
                        <button class:on={at === e.snapshot} onclick={() => look(e.snapshot === last ? null : e.snapshot)} style:--g={v.judgement ? GRADE[v.judgement.grade].color : "var(--surface-3)"}>
                          <span class="st">{e.street === "preflop" ? "Preflop" : e.street === "flop" ? "Flop" : e.street === "turn" ? "Turn" : "River"}</span>
                          <b>{choiceShort(e.choice)}</b>
                          <span class="g">{v.judgement ? GRADE[v.judgement.grade].label : "nicht bewertet"}</span>
                          {#if v.judgement?.loss_bb != null && v.judgement.loss_bb > 0.005}<span class="num faint">−{num(v.judgement.loss_bb, 2)} BB</span>{/if}
                        </button>
                      </li>
                    {/if}
                  {/each}
                </ol>
              {/if}
              <div class="buttons">
                <button class="btn primary" onclick={nextHand} disabled={session.busy}>Nächste Hand <kbd>Leertaste</kbd></button>
                {#if live}<button class="btn" onclick={repeatHand} disabled={session.busy}>Hand wiederholen <kbd>R</kbd></button>{/if}
                {#if canSolver}<button class="btn" onclick={openInSolver}>Im Solver öffnen</button>{/if}
              </div>
            </div>
          {/if}

          {#if decision && session.hint}
            <VerdictCard {view} lesson={session.hint} title="Tipp für deine Hand" />
          {/if}
          {#if shown && !(decision && session.hint)}
            {#key `${view.seed}:${view.verdicts.indexOf(shown)}`}
              <VerdictCard {view} lesson={shown} />
            {/key}
          {/if}
          {#if session.error}<p class="error" role="alert">{session.error}</p>{/if}
        </div>
      </main>

      <aside class="side r" aria-label="Info">
        {#if rightOpen}
          <div class="side-head"><b>Info</b><button class="fold" onclick={() => (rightOpen = false)} aria-label="Info einklappen">›</button></div>
          <InfoPanel view={view} lesson={infoLesson} hands={session.hands} reviewing={session.review?.n ?? null} onOpen={review} />
        {:else}
          <button class="rail" onclick={() => (rightOpen = true)} aria-label="Info aufklappen"><span class="vt">Strategie · Hände</span></button>
        {/if}
      </aside>
    </div>
  {/if}
</div>

<style>
  .play {
    --g-best: #78c142;
    --g-good: #b3cf4f;
    --g-inaccurate: #f2b12e;
    --g-mistake: #ee7d33;
    --g-blunder: #e5483b;
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
  .dock {
    display: grid;
    gap: 12px;
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
    display: grid;
    grid-template-columns: auto 1fr;
    grid-template-rows: auto auto;
    column-gap: 10px;
    align-items: center;
    min-width: 150px;
    padding: 10px 16px 10px 12px;
    border-radius: var(--r);
    border: 1px solid var(--line-2);
    background: var(--surface-2);
    text-align: left;
    cursor: pointer;
    overflow: hidden;
    transition: background 0.12s;
  }
  .act::before {
    content: "";
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    width: 4px;
    background: var(--c);
  }
  .act:hover:not(:disabled) {
    background: var(--surface-3);
  }
  .act kbd {
    grid-row: span 2;
  }
  .act b {
    font-size: 16px;
  }
  .act small {
    color: var(--text-2);
    font-size: 12.5px;
  }
  kbd {
    display: inline-grid;
    place-items: center;
    min-width: 22px;
    height: 22px;
    padding: 0 5px;
    border-radius: 5px;
    border: 1px solid var(--line-2);
    background: var(--surface);
    font-family: var(--font-num);
    font-size: 11.5px;
    color: var(--text-2);
  }
  .hint {
    align-self: center;
    gap: 8px;
  }
  .end {
    display: grid;
    gap: 10px;
    padding: 14px 16px;
    border-radius: var(--r);
    background: var(--surface);
    border: 1px solid var(--line);
  }
  .result {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
  }
  .moves {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .moves button {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    height: 30px;
    padding: 0 10px;
    border-radius: 999px;
    border: 1px solid var(--line-2);
    background: var(--surface-2);
    font-size: 12.5px;
    cursor: pointer;
    box-shadow: inset 3px 0 0 var(--g);
  }
  .moves button.on {
    border-color: var(--focus);
  }
  .moves .st {
    color: var(--text-3);
    font-size: 11px;
  }
  .moves .g {
    color: var(--g);
  }
  .buttons {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .buttons kbd {
    margin-left: 4px;
    height: 18px;
    font-size: 10.5px;
    background: transparent;
    border-color: currentColor;
    color: inherit;
    opacity: 0.6;
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
