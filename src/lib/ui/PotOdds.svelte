<script lang="ts">
  import { bb, pct } from "./format";
  import Term from "./Term.svelte";

  /**
   * Pot odds as a picture: the pot after your call, your call's part of it marked. That part is the
   * share of hands you have to win for the call to pay off over time.
   */
  let { pot, call }: { /** In big blinds, already holding the bet you face. */ pot: number; /** In big blinds. */ call: number } = $props();

  const need = $derived(call > 0 ? call / (pot + call) : 0);
  /** "4 von 10" reads easier than a percentage; small shares as "jedes 12. Mal". */
  const often = $derived(need >= 0.15 ? `etwa ${Math.round(need * 10)} von 10 Mal` : `etwa jedes ${Math.max(2, Math.round(1 / need))}. Mal`);
</script>

<div class="ui-potodds">
  <div class="head">
    <span class="label"><Term tip="Equity: wie oft deine Hand im Schnitt gewinnt, wenn alle Karten gegeben sind.">Equity</Term>, die du brauchst</span>
    <b class="value num">{pct(need)}</b>
  </div>
  <div class="bar" role="img" aria-label="Pot nach deinem Call {bb(pot + call)}, davon dein Call {bb(call)}: {pct(need)}">
    <span class="part pot" style:flex-grow={pot}></span>
    <span class="part call" style:flex-grow={call}></span>
  </div>
  <div class="legend num">
    <span><i class="pot"></i>Pot {bb(pot)}</span>
    <span><i class="call"></i>dein Call {bb(call)}</span>
  </div>
  <p class="meaning">Gewinnst du öfter als {often}, bringt der Call auf Dauer mehr, als er kostet.</p>
</div>

<style>
  .ui-potodds {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    min-width: 0;
  }
  .head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: var(--sp-2);
  }
  .label {
    color: var(--text-2);
    font-size: var(--fs-sm);
  }
  .value {
    font-size: var(--fs-2xl);
    font-weight: var(--fw-semibold);
    line-height: 1;
  }
  .bar {
    display: flex;
    gap: 2px;
    height: 12px;
    overflow: hidden;
    border-radius: var(--r-xs);
  }
  .part {
    flex-basis: 0;
    min-width: 4px;
    animation: ui-potodds-grow var(--dur-slow) var(--ease-enter) both;
    transform-origin: left;
  }
  .part.pot {
    background: var(--surface-3);
  }
  .part.call {
    background: var(--act-call);
    animation-delay: 80ms;
  }
  @keyframes ui-potodds-grow {
    from {
      transform: scaleX(0.4);
      opacity: 0;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .part {
      animation: none;
    }
  }
  .legend {
    display: flex;
    flex-wrap: wrap;
    justify-content: space-between;
    gap: var(--sp-1) var(--sp-3);
    color: var(--text-2);
    font-size: var(--fs-sm);
  }
  .legend span {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  i {
    width: 8px;
    height: 8px;
    border-radius: 2px;
  }
  i.pot {
    background: var(--surface-3);
    box-shadow: inset 0 0 0 1px var(--line-2);
  }
  i.call {
    background: var(--act-call);
  }
  .meaning {
    margin: 0;
    color: var(--text-2);
    font-size: var(--fs-sm);
    line-height: var(--lh-base);
  }
</style>
