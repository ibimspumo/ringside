<script lang="ts">
  import { PLAIN_NAME } from "../stats";
  import type { StatId } from "../types";
  import { SMALL_SAMPLE } from "./reads";

  /** `meters`: the list shows scales; without, only numbers. `ring`: the HUD's four segments, clockwise from the top. */
  let { meters = true, types = false, ring = [] }: { meters?: boolean; types?: boolean; ring?: StatId[] } = $props();
</script>

<p class="legend" class:meters>
  <span class="item"
    ><svg class="ring" viewBox="0 0 16 16" aria-hidden="true"><path d="M8 2.5a5.5 5.5 0 0 1 5.5 5.5M13.5 8A5.5 5.5 0 0 1 8 13.5M8 13.5A5.5 5.5 0 0 1 2.5 8M2.5 8A5.5 5.5 0 0 1 8 2.5" /></svg>{ring.length ? `Ring wie im HUD, ab oben: ${ring.slice(0, 4).map((id) => PLAIN_NAME[id]).join(" · ")}` : "Ring: vier Werte wie im HUD-Ring"}</span
  >
  {#if meters}
    <span class="item scale"><i class="band" aria-hidden="true"></i>üblicher Bereich</span>
  {:else}
    <span class="item">Alle Werte in %, Aggression (AF) als Faktor ×. Fahr über einen Spaltenkopf für die Erklärung.</span>
  {/if}
  <span class="item"><i class="dot" aria-hidden="true"></i>Farbe: niedrig, mittel, hoch – keine Note</span>
  <span class="item"><i class="dot hollow" aria-hidden="true"></i><span title="Unter {SMALL_SAMPLE} Gelegenheiten">hohl: noch unsicher</span></span>
  {#if types}<span class="item"><i class="dashed" aria-hidden="true"></i>gestrichelter Typ: noch zu wenig Hände</span>{/if}
</p>

<style>
  .legend {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-1) var(--sp-5);
    margin: 0;
    color: var(--text-2);
    font-size: var(--fs-md);
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
  i,
  .ring {
    flex-shrink: 0;
  }
  .ring {
    width: 14px;
    height: 14px;
    fill: none;
    stroke: var(--text-2);
    stroke-width: 2.4;
    stroke-dasharray: 6.6 2;
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
