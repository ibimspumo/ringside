<script lang="ts">
  import { untrack } from "svelte";
  import Replayer from "../app/Replayer.svelte";
  import DecisionCard from "../analysis/DecisionCard.svelte";
  import { GRADE_OF, KIT_SOURCE, SOURCE } from "../analysis/labels";
  import { chain, heroPosition, scene } from "../coach/drill";
  import { positionHint } from "../coach/meaning";
  import { appear, rise } from "../coach/motion";
  import ActionChain from "../ui/ActionChain.svelte";
  import Badge from "../ui/Badge.svelte";
  import Button from "../ui/Button.svelte";
  import CardText from "../ui/CardText.svelte";
  import { bb, num, pct } from "../ui/format";
  import GradeBadge from "../ui/GradeBadge.svelte";
  import { actionColor, type ActionKind } from "../ui/poker";
  import { tableHeight } from "../ui/PokerTable.svelte";
  import PotOdds from "../ui/PotOdds.svelte";
  import SourceNote from "../ui/SourceNote.svelte";
  import Stat from "../ui/Stat.svelte";
  import Term from "../ui/Term.svelte";
  import { errorText } from "./catalog";
  import { preview } from "./preview";
  import { dayOf, type DrillResult, type DrillSpot } from "./profile";

  /** One decision of the playing profile to make again: a real one or a variation with another hand. */
  let {
    spot,
    check,
    onGraded,
    onNext,
    nextLabel = "Weiter",
    why = null,
  }: {
    spot: DrillSpot;
    check: (choice: number) => Promise<DrillResult>;
    onGraded?: (r: DrillResult) => void;
    onNext?: () => void;
    nextLabel?: string;
    /** Why this kind of spot is practised: "38-mal gespielt, kostet dich etwa 0,4 BB pro Mal". */
    why?: string | null;
  } = $props();

  let result = $state<DrillResult | null>(null);
  let busy = $state(false);
  let error = $state<string | null>(null);

  const real = $derived(spot.seed === null);
  const view = $derived(scene(spot.hand, spot.action_index));
  const moves = $derived(chain(spot.hand, spot.action_index));
  /** The seat as the table names it, so question, chain and table agree. */
  const seat = $derived(heroPosition(spot.hand) ?? spot.position);
  const kit = $derived(spot.source ? KIT_SOURCE[spot.source] : null);
  const sourceHint = $derived.by(() => {
    if (!spot.source) return undefined;
    const base = SOURCE[spot.source].explain;
    if (spot.source !== "estimate" || spot.reliability === null) return base;
    return `${base} In etwa ${Math.round(spot.reliability * 10)} von 10 Prüf-Spots lag sie höchstens eine Stufe neben der Lösung.`;
  });
  const then = $derived(result?.then ?? null);
  /** "Verlässlichkeit 7 von 10" beside an estimate, so its weight is seen without hovering. */
  const reliability = $derived(spot.source === "estimate" && spot.reliability !== null ? `trifft in ${Math.round(spot.reliability * 10)} von 10 Prüf-Spots` : null);

  /** "A7" reads like the hand's name, not "7A". */
  const highFirst = (cards: string[]) => [...cards].sort((a, b) => "23456789TJQKA".indexOf(b[0]) - "23456789TJQKA".indexOf(a[0]));
  const MOVE: Record<string, ActionKind> = { fold: "fold", check: "check", call: "call", limp: "limp", bet: "bet", raise: "raise", all_in: "allin" };

  // The table takes the window's height below it, so table, controls and choices fit without scrolling.
  let drillEl = $state<HTMLElement>();
  let feltEl = $state<HTMLElement>();
  let feltH = $state(520);
  function scroller(el: HTMLElement): HTMLElement | null {
    for (let p = el.parentElement; p; p = p.parentElement) if (/(auto|scroll)/.test(getComputedStyle(p).overflowY)) return p;
    return null;
  }
  function fit() {
    if (!feltEl || !drillEl) return;
    if (drillEl.clientWidth < STACK_BELOW) {
      feltH = 460;
      return;
    }
    const box = scroller(feltEl);
    const top = feltEl.getBoundingClientRect().top;
    const at = box ? top - box.getBoundingClientRect().top + box.scrollTop : top + window.scrollY;
    const view = box ? box.clientHeight : window.innerHeight;
    // No taller than the table at this width needs, with the replay controls under it.
    const need = tableHeight(Math.min(feltEl.clientWidth, 960)) + CONTROLS;
    feltH = Math.round(Math.min(need, Math.max(420, view - at - 44)));
  }
  const STACK_BELOW = 900;
  const CONTROLS = 96;
  $effect(() => {
    void result;
    void moves;
    const frame = requestAnimationFrame(fit);
    return () => cancelAnimationFrame(frame);
  });

  /** What is left behind after calling, against the pot then; without a bet against the pot now. */
  const facing = $derived(view.toCallBB > 0 && !view.unopened);
  const allIn = $derived(facing && view.toCallBB >= view.behindBB - 0.005);
  const spr = $derived(facing ? Math.max(0, view.behindBB - view.toCallBB) / (view.potBB + view.toCallBB) : view.behindBB / Math.max(view.potBB, 0.01));
  const river = $derived(view.street === "River");
  const sprMeaning = $derived.by(() => {
    if (allIn) return "Der Call ist dein ganzer Stack: danach ist alles drin.";
    if (spr < 0.5) return "Fast alles liegt schon im Pot: Mit dem Call bist du praktisch festgelegt.";
    if (spr < 2) return "Klein: Mit einem weiteren Einsatz ist alles drin.";
    if (view.street === "Turn") return "Genug Platz für einen vollen Einsatz auf dem River.";
    if (spr < 6) return "Mittel: Platz für Einsätze auf ein, zwei Straßen.";
    return "Tief: viel Spielraum, die nächsten Straßen entscheiden mit.";
  });
  /** "All-in und Fold": the choices of this task in words. */
  const offered = $derived(spot.choices.map((c) => c.label.split(" ")[0]).join(", ").replace(/, ([^,]*)$/, " und $1"));
  const stackMeaning = $derived(
    view.behindBB <= 20
      ? "Kurz: Meist geht es um All-in oder Fold."
      : view.behindBB <= 40
        ? "Mittel: Ein Raise bindet schon einen guten Teil davon."
        : "Tief: genug Platz, um die Hand nach dem Flop zu spielen.",
  );
  const SPR_HELP = "Stack zu Pot: wie oft der Pot in deinen restlichen Stack passt. Je kleiner die Zahl, desto eher geht es um alles.";

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
    if (preview("answer")) untrack(() => choose(Number(preview("pick") ?? 0) || 0));
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

<svelte:window onkeydown={onKey} onresize={fit} />

{#snippet arrow()}<svg viewBox="0 0 24 24" width="14" height="14" aria-hidden="true"><path d="M5 12h14M13 6l6 6-6 6" /></svg>{/snippet}

<div class="drill" class:answered={!!result} bind:this={drillEl}>
<div class="grid">
  <header class="top">
    <div class="origin">
      {#if real}
        <Badge tone="info" size="sm">Echte Hand</Badge>
        <span>vom {dayOf(spot.played_at)}</span>
      {:else}
        <Badge size="sm">Neu gegeben</Badge>
        <span>Lage wie am {dayOf(spot.played_at)}, andere Karten</span>
      {/if}
      <span class="spot">· {spot.title} · {spot.detail}</span>
    </div>
    {#if kit}
      <span class="rated-by">Bewertet mit <SourceNote kind={kit} hint={sourceHint} />{#if reliability}<span class="num">{reliability}</span>{/if}</span>
    {/if}
  </header>

  <div class="table-col">
    <div class="chain"><ActionChain streets={moves} turn={result ? undefined : { pos: `Du · ${seat ?? "?"}`, text: "bist dran" }} label="Bisher in der Hand" /></div>
    <div class="felt-box" bind:this={feltEl} style:height="{feltH}px">
      <Replayer hand={spot.hand} upTo={spot.action_index} quiz={!result} inBigBlinds quietPlay solve={false} moveList={false} />
    </div>
  </div>

  <aside class="side">
    <h2 class="question">
      {#if seat}Du sitzt im <Term tip={positionHint(seat)}>{seat}</Term> mit{:else}Du hast{/if}
      <CardText hands text={highFirst(spot.hero_cards).join("")} />{#if result}.{:else}. Was spielst du?{/if}
    </h2>

    {#if !result}
      <section class="lage" aria-label="Die Lage" in:appear>
        <div class="figures">
          {#if facing}
            <Stat
              label="Call kostet"
              value={bb(view.toCallBB)}
              meaning={allIn ? "Das ist dein ganzer Stack." : `${pct(view.toCallBB / view.behindBB)} deines Stacks von ${bb(view.behindBB)}.`}
            />
            {#if river}
              <Stat label="Pot nach dem Call" value={bb(view.potBB + view.toCallBB)} meaning="Keine Karte kommt mehr: Callst du, werden die Karten gezeigt." />
            {:else}
              <Stat label={allIn ? "Stack zu Pot" : "Stack zu Pot nach dem Call"} value={num(spr, spr < 10 ? 1 : 0)} meaning={sprMeaning} help={SPR_HELP} />
            {/if}
          {:else}
            <Stat label="Pot" value={bb(view.potBB)} meaning="liegt schon in der Mitte, Blinds und Antes eingerechnet." />
            {#if view.street === "Vor dem Flop"}
              <Stat label="Dein Stack" value={bb(view.behindBB)} meaning={stackMeaning} />
            {:else}
              <Stat label="Stack zu Pot" value={num(spr, spr < 10 ? 1 : 0)} meaning={sprMeaning} help={SPR_HELP} />
            {/if}
          {/if}
        </div>
        {#if facing}
          <PotOdds pot={view.potBB} call={view.toCallBB} />
        {:else}
          <p class="free">
            {#if view.unopened}Noch hat niemand eröffnet: Vor dir haben alle gefoldet, im Pot liegen nur Blinds und Antes.
            {:else}Noch hat niemand gesetzt: Du zahlst nichts, um weiterzuspielen.{/if}
            Du entscheidest zwischen {offered}.
          </p>
        {/if}
      </section>

      <div class="choices" role="group" aria-label="Deine Entscheidung" in:appear>
        {#each spot.choices as c, i (i)}
          {@const kind = MOVE[c.kind]}
          {#snippet mark()}<i class="mark" style:--c={kind ? actionColor(kind) : "var(--text-3)"}></i>{/snippet}
          <Button size="lg" full kbd={String(i + 1)} icon={mark} disabled={busy} onclick={() => choose(i)}>{c.label}</Button>
        {/each}
      </div>
      {#if error}<p class="error" role="alert">{error}</p>{/if}
      {#if why}
        <p class="why">
          <svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true"><path d="M4 19V5M4 19h16M8 15l3-4 3 2 5-6" /></svg>
          <span><b>Warum diese Lage?</b> In deinen Händen: {why}.</span>
        </p>
      {/if}
    {:else}
      <div class="after" in:rise>
        <div class="verdict"><DecisionCard decision={result.decision} quiet /></div>
        {#if then && then.rating}
          <p class="then">
            <span>Damals hast du <b>{then.played}</b> gespielt</span>
            <GradeBadge grade={GRADE_OF[then.rating]} cost={then.ev_loss_bb ?? undefined} size="sm" />
          </p>
        {/if}
        {#if onNext}
          <div class="next">
            <Button variant="primary" size="lg" full icon={arrow} kbd="Enter" onclick={onNext}>{nextLabel}</Button>
          </div>
        {/if}
      </div>
    {/if}
  </aside>
</div>
</div>

<style>
  .drill {
    container: drill / inline-size;
  }
  .grid {
    display: grid;
    grid-template-columns: minmax(0, 1fr) clamp(340px, 34%, 520px);
    grid-template-areas:
      "top top"
      "table side";
    gap: var(--sp-4) var(--sp-6);
    align-items: start;
  }
  .top {
    grid-area: top;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-2) var(--sp-4);
    color: var(--text-2);
    font-size: var(--fs-md);
  }
  .origin {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-1) var(--sp-2);
    min-width: 0;
  }
  .spot {
    color: var(--text-3);
  }
  .rated-by {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: var(--text-3);
  }
  .table-col {
    grid-area: table;
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    min-width: 0;
  }
  .felt-box {
    display: flex;
    min-width: 0;
  }
  .side {
    grid-area: side;
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    min-width: 0;
  }
  .question {
    margin: 0;
    font-size: var(--fs-2xl);
    font-weight: var(--fw-semibold);
    line-height: 1.35;
  }
  .lage {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    padding: var(--sp-4);
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    background: var(--surface-2);
  }
  .figures {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--sp-4);
  }
  .free {
    margin: 0;
    color: var(--text-2);
    font-size: var(--fs-sm);
    line-height: var(--lh-base);
  }
  .choices {
    display: grid;
    gap: var(--sp-2);
  }
  .mark {
    width: 10px;
    height: 10px;
    border-radius: 2px;
    background: var(--c);
  }
  .why {
    display: flex;
    gap: var(--sp-2);
    margin: 0;
    color: var(--text-2);
    font-size: var(--fs-sm);
    line-height: var(--lh-base);
  }
  .why b {
    color: var(--text);
    font-weight: var(--fw-semibold);
  }
  .why svg {
    flex-shrink: 0;
    margin-top: 2px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.7;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .after {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
  }
  /* The way on stays in reach while the reason below the verdict is read. */
  .next {
    position: sticky;
    bottom: 0;
    z-index: 1;
    padding: var(--sp-3) 0 var(--sp-4);
    margin-top: calc(-1 * var(--sp-1));
    margin-bottom: calc(-1 * var(--sp-4));
    border-top: 1px solid var(--line);
    background: var(--surface);
  }
  .then {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2);
    margin: 0;
    color: var(--text-2);
    font-size: var(--fs-base);
  }
  .then b {
    color: var(--text);
    font-weight: var(--fw-semibold);
  }
  .error {
    margin: 0;
    color: var(--neg);
  }
  @container drill (max-width: 900px) {
    .grid {
      grid-template-columns: minmax(0, 1fr);
      grid-template-areas: "top" "chain" "side" "table";
    }
    /* Narrow: what happened comes before the question, the table after the choices. */
    .table-col {
      display: contents;
    }
    .chain {
      grid-area: chain;
    }
    .felt-box {
      grid-area: table;
    }
    /* The choices follow the question, so they are reached without scrolling past the figures. */
    .lage {
      order: 1;
    }
    .why {
      order: 2;
    }
  }
</style>
