<script lang="ts">
  import { untrack } from "svelte";
  import { api } from "../api";
  import Replayer from "../app/Replayer.svelte";
  import PlayingCard from "../ui/PlayingCard.svelte";
  import type { Feedback, Hand, LearnAnswer, LearnKind, OwnSpot } from "../types";
  import { errorText, fmtBB, fmtPct } from "./catalog";
  import Task from "./Task.svelte";

  let { onProgress, onRead }: { onProgress: () => Promise<void>; onRead: (id: string) => void } = $props();

  const KINDS: { id: LearnKind | null; label: string }[] = [
    { id: null, label: "Alle" },
    { id: "all_in", label: "All-ins" },
    { id: "push_fold_open", label: "All-in nach Folds" },
    { id: "push_fold_call", label: "Call gegen All-in" },
  ];
  const KIND_LABEL: Partial<Record<LearnKind, string>> = {
    all_in: "All-in",
    push_fold_open: "All-in nach Folds",
    push_fold_call: "Call gegen All-in",
  };

  let kind = $state<LearnKind | null>(null);
  let spots = $state<OwnSpot[] | null>(null);
  let error = $state<string | null>(null);
  let selected = $state<string | null>(null);
  let hand = $state<Hand | null>(null);
  let reveal = $state(false);
  /** Answers given in this visit, by spot id. */
  let results = $state<Record<string, Feedback>>({});

  $effect(() => {
    const k = kind;
    untrack(() => loadSpots(k));
  });

  async function loadSpots(k: LearnKind | null) {
    spots = null;
    error = null;
    try {
      const list = await api.learnOwnSpots({ game: null, since: null, tournament_id: null }, k, 80);
      if (k !== kind) return;
      spots = list;
      if (!list.some((s) => s.id === selected)) open(list.find((s) => !results[s.id])?.id ?? list[0]?.id ?? null);
    } catch (e) {
      error = errorText(e);
    }
  }

  async function open(id: string | null) {
    selected = id;
    reveal = false;
    hand = null;
    const spot = spots?.find((s) => s.id === id);
    if (!spot) return;
    prepare(spot);
    const following = spots?.slice((spots?.indexOf(spot) ?? 0) + 1).find((s) => !results[s.id]);
    if (following) prepare(following);
    const h = await api.hand(spot.hand_id);
    if (selected === id) hand = h;
  }

  /** A push/fold spot's table takes seconds to solve; start while the user looks at the hand. */
  const prepared = new Set<string>();
  function prepare(s: OwnSpot) {
    if (s.exercise.prompt.type !== "push_fold_table" || prepared.has(s.id)) return;
    prepared.add(s.id);
    api.learnPrepareOwn(s.id).catch(() => prepared.delete(s.id));
  }

  const spot = $derived(spots?.find((s) => s.id === selected) ?? null);
  const index = $derived(spots?.findIndex((s) => s.id === selected) ?? -1);
  const nextSpot = $derived(spots && index >= 0 ? (spots.slice(index + 1).find((s) => !results[s.id]) ?? null) : null);

  function when(t: string) {
    const [d, time] = t.split(" ");
    const [y, m, day] = d.split("/");
    return `${day}.${m}.${y}, ${time.slice(0, 5)}`;
  }

  function cards(s: OwnSpot): string[] {
    const p = s.exercise.prompt;
    return p.type === "push_fold" || p.type === "push_fold_table" ? p.hand : p.type === "all_in" ? p.hero : [];
  }

  function stack(s: OwnSpot): string | null {
    const p = s.exercise.prompt;
    if (p.type === "push_fold") return fmtBB(p.stack_bb);
    if (p.type === "push_fold_table") return `${p.positions[p.hero]} · ${p.positions.length} Spieler · ${fmtBB(p.effective_bb)}`;
    if (p.type === "all_in") return `Pot ${fmtBB(p.pots.reduce((a, x) => a + x.amount_bb, 0))}`;
    return null;
  }

  /** The all-in block below shows these, so the fact list leaves them out. */
  const IN_BARS = new Set(["Erwartet (EV)", "Tatsächlich", "Glück", "Pot"]);

  async function checkOwn(s: OwnSpot, a: LearnAnswer): Promise<Feedback> {
    const fb = await api.learnCheckOwn(s.id, a);
    return s.all_in ? { ...fb, facts: fb.facts.filter((f) => !IN_BARS.has(f.label)) } : fb;
  }

  function graded(id: string, fb: Feedback) {
    results[id] = fb;
    onProgress();
  }

  function tone(fb: Feedback | undefined) {
    if (!fb) return "";
    return fb.correct ? "ok" : fb.score >= 0.5 ? "close" : "miss";
  }

  /** Bar width for the all-in summary, relative to the largest of the three values. */
  function w(v: number, max: number) {
    return `${max > 0 ? (Math.abs(v) / max) * 50 : 0}%`;
  }
</script>

<div class="own">
  <header class="head">
    <div class="seg" role="group" aria-label="Art der Spots">
      {#each KINDS as k (k.label)}
        <button class:on={kind === k.id} onclick={() => (kind = k.id)}>{k.label}</button>
      {/each}
    </div>
  </header>

  {#if error}
    <p class="error" role="alert">{error}</p>
  {:else if spots === null}
    <p class="faint loading">Spots werden gesucht …</p>
  {:else if !spots.length}
    <div class="empty-state panel">
      <p>Noch keine passenden Spots gefunden.</p>
      <p class="faint">
        Sobald du in Turnieren all-in gehst oder mit höchstens 20 bb effektiv vor einer Push/Fold-Entscheidung stehst, als
        Erster nach lauter Folds oder gegen einen Shove, tauchen die Hände hier auf.
      </p>
    </div>
  {:else}
    <div class="split">
      <ol class="list panel" aria-label="Spots">
        {#each spots as s (s.id)}
          <li>
            <button class="item" class:on={selected === s.id} aria-current={selected === s.id ? "true" : undefined} onclick={() => open(s.id)}>
              <span class="cards">{#each cards(s) as c (c)}<PlayingCard card={c} size={22} />{/each}</span>
              <span class="txt">
                <b>{KIND_LABEL[s.kind] ?? s.kind}</b>
                {#if s.exercise.prompt.type === "push_fold_table"}
                  <span class="dim">{stack(s)}</span>
                  <span class="dim">{when(s.played_at)}</span>
                {:else}
                  {#if stack(s)}<span class="dim">{stack(s)}</span>{/if}
                  <span class="dim">{when(s.played_at)}</span>
                {/if}
              </span>
              <span class="state {tone(results[s.id])}" title={results[s.id] ? `${Math.round(results[s.id].score * 100)} Punkte` : "Noch offen"}></span>
            </button>
          </li>
        {/each}
      </ol>

      <div class="detail">
        {#if spot}
          <div class="replay panel">
            {#if hand}
              <Replayer {hand} upTo={reveal ? null : spot.stop_before_action}>
                <div class="hand-title">
                  <b>{KIND_LABEL[spot.kind]}{#if spot.exercise.prompt.type === "push_fold_table"} · {stack(spot)}{/if}</b>
                  <span class="faint">Hand {spot.hand_id} · {when(spot.played_at)}</span>
                  {#if results[spot.id]}<span class="stop done">Aufgelöst</span>{:else if !reveal}<span class="stop">Stopp vor deiner Entscheidung</span>{/if}
                </div>
              </Replayer>
            {:else}
              <p class="faint loading">Hand wird geladen …</p>
            {/if}
          </div>

          <div class="panel body">
            {#key spot.id}
              <Task
                exercise={spot.exercise}
                showStage={spot.kind === "all_in"}
                check={(a) => checkOwn(spot, a)}
                onGraded={(fb) => graded(spot.id, fb)}
                onNext={nextSpot ? () => open(nextSpot.id) : undefined}
                nextLabel="Nächster Spot"
              >
                {#snippet after()}
                  {#if spot.all_in}
                    {@const r = spot.all_in}
                    {@const max = Math.max(Math.abs(r.ev_bb), Math.abs(r.net_bb), Math.abs(r.luck_bb), 0.01)}
                    <div class="allin">
                      <h4>Und dein All-in damals: Entscheidung gegen Ergebnis</h4>
                      {#each [{ label: "Erwartet (EV)", v: r.ev_bb }, { label: "Tatsächlich", v: r.net_bb }, { label: "Glück", v: r.luck_bb }] as row, i (row.label)}
                        <div class="row" style="--i:{i}">
                          <span class="lbl">{row.label}</span>
                          <span class="track">
                            <span class="fill" class:neg={row.v < 0} style="width:{w(row.v, max)}"></span>
                          </span>
                          <b class="num" class:pos={row.v > 0.004} class:neg={row.v < -0.004}>{fmtBB(row.v, true)}</b>
                        </div>
                      {/each}
                      <p class="dim small">
                        Equity {fmtPct(r.equity_pct, 1)} · Pot {fmtBB(r.pot_bb)}{#if r.rake_bb > 0} · Rake {fmtBB(r.rake_bb)}{/if}. Glück ist das
                        Ergebnis minus EV. Über viele Hände gleicht es sich aus.
                      </p>
                    </div>
                  {/if}
                  <div class="more">
                    {#if !reveal && hand}<button class="btn" onclick={() => (reveal = true)}>Hand zu Ende abspielen</button>{/if}
                    <button class="link" onclick={() => onRead(spot.kind === "all_in" ? "entscheidung-ergebnis" : "push-fold")}>
                      Nachlesen: {spot.kind === "all_in" ? "Entscheidung statt Ergebnis" : "Push/Fold"}
                    </button>
                  </div>
                {/snippet}
              </Task>
            {/key}
            {#if results[spot.id] && !nextSpot}
              <p class="faint small end">Alle Spots dieser Liste beantwortet.</p>
            {/if}
          </div>
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  .own {
    display: grid;
    gap: 16px;
  }
  .head {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 12px 20px;
  }
  .head .seg {
    flex-wrap: wrap;
    max-width: 100%;
  }
  .split {
    display: grid;
    grid-template-columns: 240px minmax(0, 1fr);
    gap: 14px;
    align-items: start;
  }
  .list {
    position: sticky;
    top: 0;
    max-height: calc(100vh - 220px);
    overflow-y: auto;
    margin: 0;
    padding: 4px;
    list-style: none;
  }
  .item {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 8px;
    border: 0;
    border-radius: var(--r-sm);
    background: transparent;
    text-align: left;
    cursor: pointer;
  }
  .item:hover {
    background: var(--surface-2);
  }
  .item.on {
    background: var(--surface-3);
  }
  .item .cards {
    display: inline-flex;
    gap: 2px;
  }
  .txt {
    display: grid;
    min-width: 0;
    font-size: 12.5px;
  }
  .txt b {
    font-size: 13px;
  }
  .txt span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .state {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    border: 1.5px solid var(--text-3);
  }
  .state.ok {
    background: var(--pos);
    border-color: var(--pos);
  }
  .state.close {
    background: var(--amber);
    border-color: var(--amber);
  }
  .state.miss {
    background: var(--neg);
    border-color: var(--neg);
  }
  .detail {
    display: grid;
    gap: 12px;
    min-width: 0;
  }
  /* Wide windows: replay and question side by side, so both stay in view. */
  @container (min-width: 1800px) {
    .detail {
      grid-template-columns: minmax(0, 6fr) minmax(460px, 5fr);
      align-items: start;
    }
    .detail .replay {
      position: sticky;
      top: 0;
    }
  }
  .replay {
    display: flex;
    height: clamp(460px, 76vh, 820px);
    padding: 12px;
    min-width: 0;
  }
  .hand-title {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 4px 10px;
    font-size: 13px;
  }
  .stop {
    padding: 1px 8px;
    border-radius: 99px;
    background: var(--text);
    color: var(--bg);
    font-size: 11.5px;
    font-weight: 600;
  }
  .stop.done {
    background: transparent;
    border: 1px solid var(--line-2);
    color: var(--text-2);
  }
  .body {
    padding: 18px 20px;
  }
  .allin {
    display: grid;
    gap: 8px;
    padding: 12px 14px;
    border-radius: var(--r-sm);
    background: var(--surface-2);
  }
  h4 {
    margin: 0 0 2px;
    font-size: 13px;
    font-weight: 600;
  }
  .row {
    display: grid;
    grid-template-columns: 110px minmax(0, 1fr) 90px;
    align-items: center;
    gap: 10px;
    font-size: 13px;
  }
  .row b {
    text-align: right;
    font-size: 15px;
  }
  .track {
    position: relative;
    height: 10px;
    border-radius: 99px;
    background: var(--surface-3);
  }
  .track::after {
    content: "";
    position: absolute;
    left: 50%;
    top: -3px;
    bottom: -3px;
    border-left: 1px solid var(--text-3);
  }
  .fill {
    position: absolute;
    left: 50%;
    top: 0;
    bottom: 0;
    border-radius: 0 99px 99px 0;
    background: var(--pos);
    animation: grow 0.5s cubic-bezier(0.2, 0.8, 0.2, 1) both;
    animation-delay: calc(var(--i) * 90ms + 0.15s);
    transform-origin: left;
  }
  .fill.neg {
    left: auto;
    right: 50%;
    border-radius: 99px 0 0 99px;
    background: var(--neg);
    transform-origin: right;
  }
  @keyframes grow {
    from {
      transform: scaleX(0);
    }
  }
  .more {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 18px;
  }
  .link {
    padding: 0;
    border: 0;
    background: none;
    color: var(--text-2);
    font-size: 13px;
    text-decoration: underline;
    text-decoration-color: var(--line-2);
    text-underline-offset: 3px;
    cursor: pointer;
  }
  .link:hover {
    color: var(--text);
  }
  .small {
    margin: 0;
    font-size: 12.5px;
  }
  .end {
    margin-top: 12px;
  }
  .loading {
    margin: 30px 0;
    text-align: center;
  }
  .empty-state {
    padding: 28px 24px;
  }
  .empty-state p {
    margin: 0 0 6px;
  }
  .error {
    margin: 0;
    color: #ff9d93;
  }
  @container (max-width: 820px) {
    .split {
      grid-template-columns: 1fr;
    }
    .list {
      position: static;
      max-height: 230px;
    }
  }
  @container (max-width: 520px) {
    .row {
      grid-template-columns: 84px minmax(0, 1fr) 76px;
    }
    .body {
      padding: 16px;
    }
  }
</style>
