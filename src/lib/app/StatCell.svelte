<script lang="ts">
  import { STAT_META, fmt, fmtUnit, stat, tone, toneColor } from "../stats";
  import type { HudConfig, StatId, StatValue } from "../types";
  import StatMeter from "../ui/StatMeter.svelte";
  import { SMALL_SAMPLE, sampleNote } from "./reads";

  /** One value of a player in a list: the number in its HUD colour, with `meter` its place on the scale. */
  let { stats, id, hud, meter = false }: { stats: StatValue[]; id: StatId; hud: HudConfig; meter?: boolean } = $props();

  const s = $derived(stat(stats, id));
  const thin = $derived(id !== "hands" && (s?.opp ?? 0) < SMALL_SAMPLE);
  const range = $derived(hud.thresholds[id]);
  const max = $derived(STAT_META[id].ratio ? 6 : 100);
  const word = $derived.by(() => {
    const t = tone(hud, s);
    return t === "low" ? "unter üblich" : t === "high" ? "über üblich" : t === "mid" ? "üblich" : "";
  });
  const title = $derived(
    id === "hands" || !s
      ? undefined
      : `${STAT_META[id].name} ${fmtUnit(s)}${word ? `, ${word}` : ""} – ${sampleNote(s)}${range ? `. Üblich: ${range.low}–${range.high}${STAT_META[id].ratio ? "×" : " %"}` : ""}`,
  );
</script>

<span class="cell" class:thin class:meter {title}>
  <span class="val num" style={id === "hands" ? "" : `color:${toneColor(hud, s, "var(--text-3)")}`}>{fmt(s)}</span>
  {#if meter && range}
    <StatMeter size="sm" value={s?.value ?? null} low={range.low} high={range.high} {max} color={toneColor(hud, s, "var(--text-3)")} {thin} label={title ?? STAT_META[id].name} />
  {/if}
</span>

<style>
  .cell {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: var(--sp-2);
    width: 100%;
  }
  .meter > :global(.ui-meter) {
    flex: 1;
    min-width: 36px;
  }
  /* A narrow list keeps the numbers and drops the scale. */
  @container player-list (max-width: 820px) {
    .meter > :global(.ui-meter) {
      display: none;
    }
  }
  .val {
    min-width: 26px;
    text-align: right;
    font-size: var(--fs-base);
    font-variant-numeric: tabular-nums;
  }
  .thin .val {
    opacity: 0.5;
  }
</style>
