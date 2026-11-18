<script lang="ts">
  import { GRADE_OF, KIT_SOURCE } from "../analysis/labels";
  import GradeIcon from "../ui/GradeIcon.svelte";
  import Button from "../ui/Button.svelte";
  import { bb, num } from "../ui/format";
  import MiniCard from "../ui/MiniCard.svelte";
  import Reason from "../ui/Reason.svelte";
  import SourceNote from "../ui/SourceNote.svelte";
  import Term from "../ui/Term.svelte";
  import { cardsOf, luckLine, outcomeLine, swingLine } from "./labels";
  import type { SessionReview, Swing } from "./types";

  /**
   * Every decision of the session that went the other way than it deserved. The verdict is the
   * rating when it was made, against the opponent's range; the cards shown afterwards only add
   * how it turned out.
   */
  let { review, onBack, onHand }: { review: SessionReview; onBack: () => void; onHand: (handId: number, actionIndex: number) => void } = $props();
</script>

{#snippet card(s: Swing, right: boolean)}
  <article class="lk-card" style="--lk-grade:var(--grade-{s.rating ? GRADE_OF[s.rating] : right ? 'best' : 'mistake'})">
    <header class="lk-top">
      {#if s.rating}<GradeIcon grade={GRADE_OF[s.rating]} size={18} />{/if}
      <span class="lk-verdict">{right ? "Schlau entschieden" : "Nicht schlau, auch wenn's gut ging"}</span>
      <span class="lk-net" class:lk-up={s.net_bb > 0} class:lk-down={s.net_bb < 0} title="Ergebnis der Hand">Hand {bb(s.net_bb, { sign: true })}</span>
    </header>
    <div class="lk-hand">
      <span class="lk-cards" title="Deine Karten">{#each cardsOf(s.hero_cards) as c (c)}<MiniCard card={c} />{/each}</span>
      {#each s.opponents as o, i (i)}
        <span class="lk-vs">gegen</span><span class="lk-cards" title="Gezeigte Karten">{#each o as c (c)}<MiniCard card={c} />{/each}</span>
      {/each}
      {#if s.board.length}<span class="lk-vs">Board</span><span class="lk-cards">{#each s.board as c (c)}<MiniCard card={c} />{/each}</span>{/if}
      <span class="lk-played">{s.position ?? "?"} · <b>{s.played}</b></span>
    </div>
    <p class="lk-line">{swingLine(s, right, false)}</p>
    {#if s.equity_pct !== null}
      <div class="lk-eq">
        <span class="lk-eq-label">Gewinnchance gegen die gezeigten Karten</span>
        <span class="lk-bar" role="img" aria-label="{num(s.equity_pct, 0)} %"><i style="width:{s.equity_pct}%"></i></span>
        <b class="lk-eq-num">{num(s.equity_pct, 0)} %</b>
      </div>
    {/if}
    <p class="lk-out">{outcomeLine(s, right)}</p>
    {#if s.why}<Reason text={s.why} />{/if}
    <footer class="lk-foot">
      {#if s.source && KIT_SOURCE[s.source]}<SourceNote kind={KIT_SOURCE[s.source]!} />{/if}
      <Button size="sm" onclick={() => onHand(s.hand_id, s.action_index)}>Abspielen</Button>
    </footer>
  </article>
{/snippet}

<div class="lk">
  <div class="lk-bar-top">
    <Button size="sm" variant="quiet" onclick={onBack}>
      {#snippet icon()}<svg class="lk-ico" viewBox="0 0 24 24" width="14" height="14" aria-hidden="true"><path d="M15 5l-7 7 7 7" /></svg>{/snippet}
      Zurück zum Rückblick
    </Button>
    <p class="lk-sum">{luckLine(review.report.luck)}</p>
  </div>

  <div class="lk-cols">
    <section aria-labelledby="lk-bad">
      <h2 id="lk-bad" class="lk-title">Gut entschieden, Pech gehabt</h2>
      <p class="lk-sub">Richtig nach <Term id="equilibrium" />, <Term id="model" /> oder <Term id="solved" /> – und trotzdem Chips verloren. Auf Dauer gewinnst du mit genau diesen Entscheidungen.</p>
      {#each review.bad_luck as s (s.hand_id)}{@render card(s, true)}{:else}<p class="lk-sub">Diesmal keine.</p>{/each}
    </section>
    <section aria-labelledby="lk-good">
      <h2 id="lk-good" class="lk-title">Glück gehabt</h2>
      <p class="lk-sub">Gewonnen, obwohl die Entscheidung auf Dauer Chips kostet. Kein Drama – aber merk dir den Spot.</p>
      {#each review.good_luck as s (s.hand_id)}{@render card(s, false)}{:else}<p class="lk-sub">Diesmal keine.</p>{/each}
    </section>
  </div>
</div>

<style>
  .lk {
    display: flex;
    flex-direction: column;
    gap: var(--sp-5);
  }
  .lk-bar-top {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    align-items: flex-start;
  }
  .lk-ico {
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
  }
  .lk-sum {
    margin: 0;
    font-size: var(--fs-lg);
  }
  .lk-cols {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(380px, 1fr));
    gap: var(--sp-6);
    align-items: start;
  }
  .lk-cols section {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }
  .lk-title {
    margin: 0;
    font-size: var(--fs-xl);
    font-weight: var(--fw-semibold);
  }
  .lk-sub {
    margin: 0;
    font-size: var(--fs-base);
    color: var(--text-2);
  }
  .lk-card {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    padding: var(--sp-4);
    background: var(--surface);
    border: 1px solid var(--line);
    border-left: 3px solid var(--lk-grade);
    border-radius: var(--r-lg);
  }
  .lk-top {
    display: flex;
    gap: var(--sp-2);
    align-items: center;
  }
  .lk-verdict {
    font-size: var(--fs-base);
    font-weight: var(--fw-semibold);
  }
  .lk-net {
    margin-left: auto;
    font-family: var(--font-num);
    font-weight: var(--fw-semibold);
  }
  .lk-up {
    color: var(--pos);
  }
  .lk-down {
    color: var(--neg);
  }
  .lk-hand {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-2);
    align-items: center;
    font-size: var(--fs-lg);
  }
  .lk-cards {
    display: inline-flex;
    gap: 2px;
  }
  .lk-vs {
    font-size: var(--fs-sm);
    color: var(--text-3);
  }
  .lk-played {
    margin-left: auto;
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .lk-line {
    margin: 0;
    font-size: var(--fs-base);
  }
  .lk-out {
    margin: 0;
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .lk-eq {
    display: grid;
    grid-template-columns: auto minmax(80px, 1fr) auto;
    gap: var(--sp-3);
    align-items: center;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .lk-bar {
    height: 6px;
    overflow: hidden;
    background: var(--surface-3);
    border-radius: var(--r-pill);
  }
  .lk-bar i {
    display: block;
    height: 100%;
    background: var(--text-2);
    border-radius: inherit;
  }
  .lk-eq-num {
    font-family: var(--font-num);
    color: var(--text);
  }
  .lk-foot {
    display: flex;
    gap: var(--sp-3);
    align-items: center;
    justify-content: space-between;
  }
</style>
