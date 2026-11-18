<script lang="ts">
  import { SMALL_SAMPLE } from "./reads";

  /** `meters`: the list shows scales; without, only numbers. */
  let { meters = true, types = false }: { meters?: boolean; types?: boolean } = $props();
</script>

<p class="legend" class:meters>
  {#if meters}
    <span class="item scale"><i class="band" aria-hidden="true"></i>üblicher Bereich laut deinem HUD</span>
    <span class="item scale"><i class="dot" aria-hidden="true"></i>Wert; die Farbe sagt niedrig, mittel oder hoch wie im HUD – keine Note</span>
  {:else}
    <span class="item">Alle Werte in %, Aggression (AF) als Faktor ×. Fahr über einen Spaltenkopf für die Erklärung.</span>
  {/if}
  <span class="item"><i class="dot hollow" aria-hidden="true"></i>hohl: unter {SMALL_SAMPLE} Gelegenheiten, noch unsicher</span>
  {#if types}<span class="item"><i class="dashed" aria-hidden="true"></i>gestrichelter Typ: noch zu wenig Hände für ein sicheres Urteil</span>{/if}
</p>

<style>
  .legend {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-1) var(--sp-5);
    margin: 0;
    color: var(--text-2);
    font-size: var(--fs-sm);
  }
  /* Without the scales (narrow list) there is nothing to explain. */
  @container player-list (max-width: 760px) {
    .item.scale {
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
  .dashed {
    width: 18px;
    height: 10px;
    border: 1px dashed var(--text-2);
    border-radius: var(--r-pill);
  }
  .hollow {
    background: none;
    box-shadow: inset 0 0 0 2px var(--text-2);
  }
</style>
