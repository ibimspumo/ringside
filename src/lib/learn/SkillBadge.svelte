<script lang="ts">
  import type { LearnKind, SkillProgress } from "../types";
  import { LEVEL_HINT, LEVEL_TEXT, SKILLS } from "./catalog";

  /** A skill's icon inside a four-segment ring: one segment per mastery level, all green once confirmed. */
  let { kind, skill, size = 40 }: { kind: LearnKind; skill: SkillProgress | undefined; size?: number } = $props();

  const level = $derived(skill?.level ?? 0);
  const stroke = $derived(Math.max(2.5, size / 13));
  const r = $derived((size - stroke) / 2);
  const c = $derived(size / 2);
  const GAP = 16;

  function arc(i: number): string {
    const a0 = ((i * 90 + GAP / 2 - 90) * Math.PI) / 180;
    const a1 = (((i + 1) * 90 - GAP / 2 - 90) * Math.PI) / 180;
    return `M ${c + r * Math.cos(a0)} ${c + r * Math.sin(a0)} A ${r} ${r} 0 0 1 ${c + r * Math.cos(a1)} ${c + r * Math.sin(a1)}`;
  }
</script>

<span
  class="badge"
  class:top={level === 4}
  style="--s:{size}px"
  role="img"
  aria-label="{SKILLS[kind].short}: {LEVEL_TEXT[level]}"
  title="{LEVEL_TEXT[level]}: {LEVEL_HINT[level]}"
>
  <svg class="ring" width={size} height={size} viewBox="0 0 {size} {size}" aria-hidden="true">
    {#each [0, 1, 2, 3] as i (i)}
      <path d={arc(i)} stroke-width={stroke} class:on={i < level} style="--d:{i * 70}ms" />
    {/each}
  </svg>
  <svg class="ico" viewBox="0 0 24 24" width={size * 0.44} height={size * 0.44} aria-hidden="true"><path d={SKILLS[kind].icon} /></svg>
</span>

<style>
  .badge {
    position: relative;
    display: inline-grid;
    place-items: center;
    width: var(--s);
    height: var(--s);
    flex-shrink: 0;
    color: var(--text);
  }
  .ring {
    position: absolute;
    inset: 0;
  }
  .ring path {
    fill: none;
    stroke: var(--surface-3);
    stroke-linecap: round;
    transition: stroke 0.4s var(--d);
  }
  .ring path.on {
    stroke: var(--text-2);
  }
  .top .ring path.on {
    stroke: var(--pos);
  }
  .ico {
    fill: none;
    stroke: currentColor;
    stroke-width: 1.7;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
</style>
