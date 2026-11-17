<script lang="ts">
  import Term from "../solver/Term.svelte";
  import { GRADE, GRADES, THRESHOLD_TEXT, num, signedBB } from "./labels";
  import type { Stats } from "./session.svelte";

  let { s }: { s: Stats } = $props();
  const total = $derived(GRADES.reduce((a, g) => a + s.counts[g], 0));
</script>

<section class="stats" aria-label="Session-Statistik">
  <div class="score">
    <span class="big num" class:none={s.score === null}>{s.score === null ? "–" : Math.round(s.score)}</span>
    <span class="unit">
      <b>Punkte</b>
      <small class="faint">von 100, Schnitt deiner Züge</small>
    </span>
  </div>

  <div class="mix" role="img" aria-label="Verteilung der Bewertungen">
    {#each GRADES as g (g)}
      {#if s.counts[g] > 0}<i style="flex:{s.counts[g]}; background:{GRADE[g].color}"></i>{/if}
    {/each}
    {#if total === 0}<i class="empty"></i>{/if}
  </div>

  <ul class="grades">
    {#each GRADES as g (g)}
      <li title={THRESHOLD_TEXT[g]}>
        <span class="dot" style:background={GRADE[g].color}></span>
        <span>{GRADE[g].label}</span>
        <b class="num">{s.counts[g]}</b>
      </li>
    {/each}
  </ul>

  <dl>
    <div><dt>Hände</dt><dd class="num">{s.hands}</dd></div>
    <div><dt>Züge</dt><dd class="num">{s.moves}</dd></div>
    <div>
      <dt><Term term="evloss">EV-Verlust</Term> gesamt</dt>
      <dd class="num">{num(s.evLoss, 2)} BB</dd>
    </div>
    <div>
      <dt>je Hand</dt>
      <dd class="num">{s.evLossPerHand === null ? "–" : `${num(s.evLossPerHand, 2)} BB`}</dd>
    </div>
    <div>
      <dt title="Was die Karten gebracht haben – zählt nicht für die Punkte">Ergebnis am Tisch</dt>
      <dd class="num" class:pos={s.net > 0.04} class:neg={s.net < -0.04}>{signedBB(s.net)}</dd>
    </div>
  </dl>
  <p class="faint note">Punkte zählen die Entscheidung, nicht das Ergebnis: Auch richtig gespielte Hände gehen mal verloren.</p>
</section>

<style>
  .stats {
    display: grid;
    gap: 14px;
  }
  .score {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .big {
    font-size: 46px;
    line-height: 1;
    font-weight: 600;
    letter-spacing: -0.02em;
  }
  .big.none {
    color: var(--text-3);
    font-weight: 400;
  }
  .unit {
    display: grid;
    line-height: 1.25;
  }
  .unit small {
    font-size: 11.5px;
  }
  .mix {
    display: flex;
    gap: 2px;
    height: 8px;
    border-radius: 4px;
    overflow: hidden;
  }
  .mix i {
    display: block;
    min-width: 3px;
  }
  .mix .empty {
    flex: 1;
    background: var(--surface-3);
  }
  .grades {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 5px;
    font-size: 13px;
  }
  .grades li {
    display: grid;
    grid-template-columns: 10px 1fr auto;
    align-items: center;
    gap: 8px;
  }
  .dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
  }
  dl {
    margin: 0;
    display: grid;
    gap: 6px;
    padding-top: 10px;
    border-top: 1px solid var(--line);
    font-size: 13px;
  }
  dl div {
    display: flex;
    justify-content: space-between;
    gap: 8px;
  }
  dt {
    color: var(--text-2);
  }
  dd {
    margin: 0;
  }
  .note {
    margin: 0;
    font-size: 11.5px;
  }
</style>
