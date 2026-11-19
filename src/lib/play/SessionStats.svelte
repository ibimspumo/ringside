<script lang="ts">
  import Term from "../ui/Term.svelte";
  import GradeBars from "../ui/GradeBars.svelte";
  import { num, signedBB } from "./labels";
  import type { Stats } from "./session.svelte";

  let { s }: { s: Stats } = $props();
  /** What the points say, in a word. */
  // A word on the session only once there is enough to judge; before that the counts speak.
  const verdict = $derived(
    s.score === null ? "noch keine Bewertung" : s.graded < 8 ? `${s.graded} ${s.graded === 1 ? "Zug" : "Züge"} bewertet` : s.score >= 85 ? "stark gespielt" : s.score >= 60 ? "solide, mit Lücken" : "viele teure Fehler",
  );
</script>

<section class="stats" aria-label="Session-Statistik">
  <div class="score">
    <span class="big num" class:none={s.score === null}>{s.score === null ? "–" : Math.round(s.score)}</span>
    <span class="what">
      <b>{verdict}</b>
      <small class="faint">Punkte von 100, Schnitt deiner Züge</small>
    </span>
  </div>

  <GradeBars counts={s.counts} label="Bewertungen deiner Züge in dieser Session" />

  <dl>
    <div><dt>Hände</dt><dd class="num">{s.hands}</dd></div>
    <div><dt>Züge</dt><dd class="num">{s.moves}</dd></div>
    <div>
      <dt><Term id="ev_loss">Kostet im Schnitt</Term>, gesamt</dt>
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
    font-size: var(--fs-4xl);
    line-height: 1;
    font-weight: 600;
    letter-spacing: -0.02em;
  }
  .big.none {
    color: var(--text-3);
    font-weight: 400;
  }
  .what {
    display: grid;
    gap: 2px;
    line-height: 1.25;
    font-size: var(--fs-md);
  }
  .what small {
    font-size: var(--fs-sm);
  }
  dl {
    margin: 0;
    display: grid;
    gap: 6px;
    padding-top: 10px;
    border-top: 1px solid var(--line);
    font-size: var(--fs-md);
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
    font-size: var(--fs-sm);
    line-height: 1.45;
  }
</style>
