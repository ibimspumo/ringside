<script lang="ts">
  import { SMALL_SAMPLE } from "./reads";
</script>

<p class="legend">
  <span class="item"><i class="band" aria-hidden="true"></i>üblicher Bereich laut deinem HUD</span>
  <span class="item"><i class="dot" aria-hidden="true"></i>Wert, Farbe wie im HUD</span>
  <span class="item"><i class="dot hollow" aria-hidden="true"></i>unter {SMALL_SAMPLE} Gelegenheiten: noch unsicher</span>
</p>

<style>
  .legend {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-1) var(--sp-5);
    margin: 0;
    color: var(--text-3);
    font-size: var(--fs-sm);
  }
  /* Without the scales (narrow list) there is nothing to explain. */
  @container player-list (max-width: 820px) {
    .legend {
      display: none;
    }
  }
  .item {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-2);
  }
  i {
    flex-shrink: 0;
  }
  .band {
    width: 22px;
    height: 6px;
    border-radius: var(--r-pill);
    background: color-mix(in srgb, var(--text) 24%, transparent);
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--text-2);
  }
  .hollow {
    background: none;
    box-shadow: inset 0 0 0 2px var(--text-2);
  }
</style>
