<script lang="ts">
  import type { SkillProgress } from "../types";
  import { LEVEL_HINT, LEVEL_TEXT } from "./catalog";

  /** Four segments: kennengelernt, mit Hilfe, selbstständig, bestätigt. */
  let { skill, label = true }: { skill: SkillProgress | undefined; label?: boolean } = $props();
  const level = $derived(skill?.level ?? 0);
</script>

<span class="mastery" title="{LEVEL_TEXT[level]}: {LEVEL_HINT[level]}">
  <span class="pips" role="img" aria-label="Kompetenz: {LEVEL_TEXT[level]} ({level} von 4)">
    {#each [1, 2, 3, 4] as n (n)}
      <i class:on={n <= level} class:top={n === 4 && level === 4}></i>
    {/each}
  </span>
  {#if label}<span class="txt" class:faint={level === 0}>{LEVEL_TEXT[level]}</span>{/if}
</span>

<style>
  .mastery {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-size: 12.5px;
    color: var(--text-2);
    white-space: nowrap;
  }
  .pips {
    display: inline-flex;
    gap: 3px;
  }
  i {
    width: 14px;
    height: 5px;
    border-radius: 99px;
    background: var(--surface-3);
    transition: background 0.3s;
  }
  i.on {
    background: var(--text-2);
  }
  i.on.top,
  .pips:has(i.top) i.on {
    background: var(--pos);
  }
</style>
