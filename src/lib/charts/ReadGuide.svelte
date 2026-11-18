<script lang="ts">
  /** How the 13×13 grid is laid out and how to get the details of a hand. */
  let { fill = "Teilweise gefüllte Felder spielst du gemischt." }: { fill?: string } = $props();
</script>

<section class="panel guide">
  <div class="panel-head"><h2 class="panel-title">So liest du den Chart</h2></div>
  <div class="row">
    <svg viewBox="0 0 78 78" width="78" height="78" aria-hidden="true">
      <path class="s" d="M3 3h72v72z" />
      <path class="o" d="M3 3v72h72z" />
      <path class="p" d="M3 3l72 72" />
      <text x="56" y="22" text-anchor="middle">s</text>
      <text x="22" y="62" text-anchor="middle">o</text>
      <text x="16" y="12" class="pt">AA</text>
      <text x="62" y="73" class="pt" text-anchor="end">22</text>
    </svg>
    <ul>
      <li><b>Diagonale:</b> Paare, von AA oben links bis 22 unten rechts.</li>
      <li><b>Oben rechts (s):</b> suited, beide Karten in einer Farbe.</li>
      <li><b>Unten links (o):</b> offsuit, verschiedene Farben.</li>
    </ul>
  </div>
  <p class="note">{fill} Fahr mit der Maus über eine Hand für die Kurzfassung, <b>klick</b> für alle Zahlen. Pfeiltasten wandern durch das Raster.</p>
</section>

<style>
  .guide {
    padding-bottom: 16px;
  }
  .row {
    display: flex;
    gap: 16px;
    align-items: center;
    margin: 12px 18px 0;
  }
  svg {
    flex-shrink: 0;
  }
  .s {
    fill: color-mix(in srgb, var(--text) 9%, var(--surface-2));
  }
  .o {
    fill: var(--surface-2);
    stroke: var(--line-2);
    stroke-width: 1;
  }
  .p {
    stroke: var(--text-2);
    stroke-width: 5;
    stroke-linecap: round;
  }
  text {
    fill: var(--text-2);
    font-family: var(--font-num);
    font-size: var(--fs-md);
    font-weight: 600;
  }
  text.pt {
    font-size: var(--fs-xs);
    fill: var(--text-3);
  }
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: var(--fs-sm);
    line-height: 1.45;
    color: var(--text-2);
  }
  b {
    color: var(--text);
    font-weight: 600;
  }
  .note {
    margin: 12px 18px 0;
    font-size: var(--fs-sm);
    line-height: 1.5;
    color: var(--text-2);
  }
</style>
