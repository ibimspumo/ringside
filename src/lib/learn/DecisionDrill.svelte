<script lang="ts">
  import { untrack } from "svelte";
  import Replayer from "../app/Replayer.svelte";
  import DecisionCard from "../analysis/DecisionCard.svelte";
  import { KIT_SOURCE, RATING, SOURCE } from "../analysis/labels";
  import { behind, bb, scene } from "../coach/drill";
  import { positionHint } from "../coach/meaning";
  import { appear, rise } from "../coach/motion";
  import Badge from "../ui/Badge.svelte";
  import Button from "../ui/Button.svelte";
  import CardText from "../ui/CardText.svelte";
  import PlayingCard from "../ui/PlayingCard.svelte";
  import SourceNote from "../ui/SourceNote.svelte";
  import { errorText } from "./catalog";
  import { preview } from "./preview";
  import { bbText, dayOf, type DrillResult, type DrillSpot } from "./profile";

  /** One decision of the playing profile to make again: a real one or a variation with another hand. */
  let {
    spot,
    check,
    onGraded,
    onNext,
    nextLabel = "Weiter",
  }: {
    spot: DrillSpot;
    check: (choice: number) => Promise<DrillResult>;
    onGraded?: (r: DrillResult) => void;
    onNext?: () => void;
    nextLabel?: string;
  } = $props();

  let result = $state<DrillResult | null>(null);
  let busy = $state(false);
  let error = $state<string | null>(null);

  const real = $derived(spot.seed === null);
  const view = $derived(scene(spot.hand, spot.action_index));
  const stack = $derived(bb(behind(spot.hand, spot.action_index)));
  const kit = $derived(spot.source ? KIT_SOURCE[spot.source] : null);
  const sourceHint = $derived.by(() => {
    if (!spot.source) return undefined;
    const base = SOURCE[spot.source].explain;
    if (spot.source !== "estimate" || spot.reliability === null) return base;
    return `${base} In etwa ${Math.round(spot.reliability * 10)} von 10 Prüf-Spots lag sie höchstens eine Stufe neben der Lösung.`;
  });
  const then = $derived(result?.then ?? null);

  async function choose(i: number) {
    if (busy || result) return;
    busy = true;
    error = null;
    try {
      result = await check(i);
      onGraded?.(result);
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = false;
    }
  }

  $effect(() => {
    if (preview("answer")) untrack(() => choose(0));
  });

  /** 1–9 pick a choice; Enter goes on once answered. */
  function onKey(e: KeyboardEvent) {
    if (e.altKey || e.ctrlKey || e.metaKey) return;
    if ((e.target as HTMLElement | null)?.closest("input, textarea, select, [contenteditable]")) return;
    if (result) {
      if (e.key === "Enter" && onNext) {
        e.preventDefault();
        onNext();
      }
      return;
    }
    const n = Number(e.key);
    if (Number.isInteger(n) && n >= 1 && n <= spot.choices.length) {
      e.preventDefault();
      choose(n - 1);
    }
  }
</script>

<svelte:window onkeydown={onKey} />

{#snippet arrow()}<svg viewBox="0 0 24 24" width="14" height="14" aria-hidden="true"><path d="M5 12h14M13 6l6 6-6 6" /></svg>{/snippet}

<div class="drill">
  <div class="stage">
    <div class="felt-box">
      <Replayer hand={spot.hand} upTo={spot.action_index} quiz={!result} inBigBlinds quietPlay solve={false}>
        <div class="hand-title">
          <span class="stop" class:done={!!result}>{result ? "Beantwortet" : "Stopp vor deiner Entscheidung"}</span>
        </div>
      </Replayer>
    </div>

    <aside class="side">
      <div class="ask">
        <div class="origin">
          {#if real}
            <Badge tone="info" size="sm">Echte Hand</Badge>
            <span>aus deiner Hand vom {dayOf(spot.played_at)}</span>
          {:else}
            <Badge size="sm">Neu gegeben</Badge>
            <span>Lage wie in deiner Hand vom {dayOf(spot.played_at)}, andere Karten</span>
          {/if}
        </div>
        <p class="eyebrow">{spot.title} · {spot.detail}</p>
        <h2 class="question">
          {#if spot.position}Du sitzt im <abbr title={positionHint(spot.position)}>{spot.position}</abbr> mit{:else}Du hast{/if}
          <CardText hands text={spot.hero_cards.join("")} />, noch <span class="num">{stack}</span> im Stack. Was spielst du?
        </h2>
        <p class="scene">
          <span class="street">{view.street}</span>
          {#if view.board.length}
            <span class="board">{#each view.board as c (c)}<PlayingCard card={c} size={20} />{/each}</span>
          {/if}
          <span class="facing">{view.facing}</span>
          <span>Pot <b class="num">{view.pot}</b></span>
        </p>
        {#if kit}
          <p class="rated-by">Bewertet wie in der Hand-Analyse: <SourceNote kind={kit} hint={sourceHint} /></p>
        {/if}
      </div>

      <div class="act">
        {#if !result}
          <div class="choices" role="group" aria-label="Deine Entscheidung" in:appear>
            {#each spot.choices as c, i (i)}
              <Button size="lg" full kbd={String(i + 1)} disabled={busy} onclick={() => choose(i)}>{c.label}</Button>
            {/each}
          </div>
          {#if error}<p class="error" role="alert">{error}</p>{/if}
        {:else}
          <div class="after" in:rise>
            {#if then && then.rating}
              <p class="then">
                Damals hast du <b>{then.played}</b> gespielt:
                <span style:color={RATING[then.rating].color}>{RATING[then.rating].label}</span>{#if (then.ev_loss_bb ?? 0) > 0.005}, das kostete
                  etwa {bbText(then.ev_loss_bb ?? 0)}{/if}.
              </p>
            {/if}
            {#if onNext}
              <Button variant="primary" size="lg" icon={arrow} kbd="Enter" onclick={onNext}>{nextLabel}</Button>
            {/if}
          </div>
        {/if}
      </div>
    </aside>
  </div>

  {#if result}
    <div class="verdict" in:rise>
      <DecisionCard decision={result.decision} />
    </div>
  {/if}
</div>

<style>
  .drill {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
  }
  .stage {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(300px, min(420px, 36%));
    gap: 16px;
    align-items: start;
  }
  .side {
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-width: 0;
  }
  .felt-box {
    display: flex;
    height: clamp(460px, 62vh, 640px);
    min-width: 0;
    overflow: hidden;
    padding: 10px;
    border-radius: var(--r-sm);
    background: var(--bg);
    border: 1px solid var(--line);
  }
  .hand-title {
    display: flex;
    gap: 10px;
    white-space: nowrap;
  }
  .stop {
    padding: 2px 9px;
    border-radius: var(--r-pill);
    border: 1px solid var(--line-2);
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .stop.done {
    color: var(--text);
    border-color: var(--text-2);
  }
  .origin {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 10px;
    margin-bottom: 10px;
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .eyebrow {
    margin: 0 0 4px;
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .question {
    margin: 0;
    font-family: var(--font-num);
    font-size: var(--fs-2xl);
    font-weight: var(--fw-semibold);
    line-height: 1.35;
  }
  abbr {
    text-decoration: underline dotted var(--text-3);
    text-underline-offset: 4px;
    cursor: help;
  }
  .scene {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 14px;
    margin: 12px 0 0;
    padding: 9px 12px;
    border-radius: var(--r-sm);
    background: var(--surface-2);
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .scene .street,
  .scene .facing,
  .scene b {
    color: var(--text);
  }
  .scene .street,
  .scene b {
    font-weight: var(--fw-semibold);
  }
  .scene .board {
    display: flex;
    gap: 2px;
  }
  .rated-by {
    margin: 10px 0 0;
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .act {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .choices {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 8px;
  }
  .after {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 12px;
  }
  .then {
    margin: 0;
    font-size: var(--fs-base);
    line-height: 1.5;
  }
  .then b {
    font-weight: var(--fw-semibold);
  }
  .error {
    margin: 0;
    color: var(--neg);
  }
  @container (max-width: 980px) {
    .stage,
    .side {
      display: contents;
    }
    .ask {
      order: -3;
    }
    .act {
      order: -2;
    }
    .felt-box {
      height: 520px;
    }
    .choices {
      grid-template-columns: repeat(auto-fit, minmax(130px, 1fr));
    }
  }
</style>
