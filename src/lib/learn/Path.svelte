<script lang="ts">
  import type { LearnKind, LearnProgress } from "../types";
  import { LEVEL_HINT, LEVEL_TEXT, SKILLS, UNITS, currentUnit, fmtPct, mastery, nextLevel } from "./catalog";
    import Ring from "./Ring.svelte";
  import SkillBadge from "./SkillBadge.svelte";
  import Weaknesses from "./Weaknesses.svelte";
  import Button from "../ui/Button.svelte";
  import { ARTICLE_BY_ID } from "./wissen/articles";
  import { loadRead } from "./wissen/read";
  import type { LearnTarget } from "../app/Leaks.svelte";
  import type { NextStep, StepHands } from "../next/types";

  let {
    progress,
    steps,
    onTrain,
    onRead,
    onLearn,
    onHands,
  }: {
    progress: LearnProgress | null;
    /** „Was als Nächstes?“, heaviest first. */
    steps: NextStep[];
    onTrain: (kind: LearnKind, level: number) => void;
    onRead: (id: string) => void;
    onLearn: (target: LearnTarget) => void;
    onHands?: (hands: StepHands) => void;
  } = $props();

  const read = loadRead();
  const byKind = $derived(new Map((progress?.skills ?? []).map((s) => [s.id, s])));

  /** Skills count fully; units without exercises yet show how much of their reading is done. */
  function unitShare(skills: LearnKind[], articles: string[]): number {
    if (skills.length) return skills.reduce((a, k) => a + mastery(byKind.get(k)), 0) / skills.length;
    return articles.length ? articles.filter((a) => read.has(a)).length / articles.length : 0;
  }

  const units = $derived(UNITS.map((u) => ({ ...u, share: unitShare(u.skills, u.articles) })));
  /** First unit with a skill that is not yet "mit Hilfe". */
  const here = $derived(currentUnit(byKind)?.id ?? null);
  /** The one skill the path suggests next: the first in the current unit that is not yet "mit Hilfe". */
  const nextKind = $derived(UNITS.find((u) => u.id === here)?.skills.find((k) => (byKind.get(k)?.level ?? 0) < 2) ?? null);
  /** With weak spots from the own game or the coach, the first of them is the one main action on this page. */
  const weakFirst = $derived(!!steps[0]?.practice);
  /** The three costliest first, so the path itself stays in view. */
  let allWeak = $state(false);
  const total = $derived(progress?.skills.filter((s) => s.id !== "all_in" && s.id !== "own_decision") ?? []);
  const solid = $derived(total.filter((s) => s.level >= 3).length);
</script>

<div class="path">
  <header class="head">
    <div class="sum" title="Selbstständig: {LEVEL_HINT[3]}">
      <div class="sum-line">
        <b class="num">{solid}</b><span class="faint num">/ {total.length}</span>
        <span class="dim">Fertigkeiten sitzen</span>
      </div>
      <span class="sum-bar" aria-hidden="true"><i style="width:{total.length ? (solid / total.length) * 100 : 0}%"></i></span>
    </div>
  </header>

  <div class="ladder" aria-label="So wächst eine Fertigkeit">
    <span class="ladder-title">So wächst eine Fertigkeit</span>
    <ol>
      {#each [1, 2, 3, 4] as l (l)}
        <li title={LEVEL_HINT[l]}>
          <SkillBadge level={l} size={20} />
          <b>{LEVEL_TEXT[l]}</b>
          {#if l < 4}<span class="arrow" aria-hidden="true">→</span>{/if}
        </li>
      {/each}
    </ol>
    <span class="dim small">Zeig auf eine Stufe, um zu sehen, was sie verlangt.</span>
  </div>

  <Weaknesses {steps} limit={allWeak ? 6 : 3} primary={weakFirst} {onLearn} {onHands} />
  {#if steps.length > 3 && !allWeak}
    <div class="more-weak"><Button variant="quiet" size="sm" onclick={() => (allWeak = true)}>Alle {Math.min(steps.length, 6)} Schwachstellen zeigen</Button></div>
  {/if}

<ol class="units">
    {#each units as u (u.id)}
      <li class="stop" class:here={here === u.id} class:reading={!u.skills.length}>
        <div class="node">
          <Ring value={u.share} size={52} stroke={4} tone={u.share >= 0.75 ? "var(--pos)" : "var(--text)"} label="Einheit {u.n}: {Math.round(u.share * 100)} % geschafft">
            {#if u.share >= 0.999}
              <svg class="check" viewBox="0 0 24 24" width="20" height="20" aria-hidden="true"><path d="M5 12.5l4.5 4.5L19 7.5" /></svg>
            {:else}
              <b class="num n">{u.n}</b>
            {/if}
          </Ring>
        </div>
        <div class="card panel">
          <div class="top">
            <h2>{u.title}</h2>
            {#if here === u.id}<span class="tag now">Hier weitermachen</span>{/if}
            {#if !u.skills.length}<span class="tag">Bisher nur Wissen</span>{/if}
          </div>
          <p class="goal dim">{u.goal}</p>

          {#if u.skills.length}
            <ul class="skills">
              {#each u.skills as k (k)}
                {@const s = byKind.get(k)}
                <li>
                  <SkillBadge kind={k} skill={s} size={38} />
                  <span class="sname">
                    <b>{SKILLS[k].short}</b>
                    <span class="dim small">
                      {#if s && s.attempts}
                        {s.attempts} {s.attempts === 1 ? "Antwort" : "Antworten"}{s.mean_score !== null ? ` · ${fmtPct(s.mean_score * 100)} im Schnitt` : ""}
                      {:else}
                        Noch nicht geübt
                      {/if}
                    </span>
                  </span>
                  <span class="lvl" title={LEVEL_HINT[s?.level ?? 0]}>{LEVEL_TEXT[s?.level ?? 0]}</span>
                  {#if k !== "all_in"}
                    <button class="btn" class:primary={!weakFirst && k === nextKind} onclick={() => onTrain(k, nextLevel(s))}>Üben</button>
                  {/if}
                </li>
              {/each}
            </ul>
          {/if}

          {#if u.articles.length}
            <div class="reads">
              <span class="dim small">Wissen</span>
              {#each u.articles as id (id)}
                {@const a = ARTICLE_BY_ID[id]}
                {#if a}
                  <button class="chip-btn" class:done={read.has(id)} onclick={() => onRead(id)}>
                    {#if read.has(id)}<svg viewBox="0 0 24 24" width="12" height="12" aria-label="gelesen"><path d="M5 12.5l4.5 4.5L19 7.5" /></svg>{/if}
                    {a.title}
                  </button>
                {/if}
              {/each}
            </div>
          {/if}

          {#if u.next.length}
            <p class="next dim small">Kommt noch: {u.next.join(", ")}</p>
          {/if}
        </div>
      </li>
    {/each}
  </ol>
</div>

<style>
  .path {
    display: grid;
    gap: 18px;
  }
  .head {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 12px 20px;
  }
  .sum {
    display: grid;
    gap: 6px;
    min-width: 260px;
    font-size: 13px;
  }
  .sum-line {
    display: flex;
    align-items: baseline;
    gap: 5px;
  }
  .sum-line b {
    font-size: 24px;
    font-weight: 600;
  }
  .sum-line .dim {
    margin-left: 4px;
  }
  .sum-bar {
    height: 6px;
    border-radius: 99px;
    background: var(--surface-3);
    overflow: hidden;
  }
  .sum-bar i {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: var(--text);
  }
  .ladder {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 18px;
    padding: 10px 16px;
    border-radius: var(--r);
    border: 1px solid var(--line);
  }
  .ladder-title {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-2);
  }
  .ladder ol {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 12px;
    margin: 0;
    padding: 0;
    list-style: none;
    counter-reset: none;
  }
  .ladder li {
    display: flex;
    align-items: center;
    gap: 7px;
    font-size: 12.5px;
    cursor: help;
  }
  .arrow {
    margin-left: 5px;
    color: var(--text-3);
  }
  .check {
    color: var(--pos);
    stroke-width: 2.4;
  }
  .units {
    position: relative;
    display: grid;
    gap: 12px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .units::before {
    content: "";
    position: absolute;
    left: 25px;
    top: 26px;
    bottom: 26px;
    border-left: 2px dashed var(--surface-3);
  }
  .stop {
    position: relative;
    display: grid;
    grid-template-columns: 52px minmax(0, 1fr);
    gap: 14px;
    align-items: start;
  }
  .node {
    position: sticky;
    top: 8px;
    border-radius: 50%;
    background: var(--bg);
  }
  .n {
    font-size: 18px;
    font-weight: 600;
  }
  .card {
    display: grid;
    gap: 10px;
    padding: 14px 16px;
    transition: border-color 0.15s;
  }
  .card {
    padding: 16px 18px;
  }
  .here .card {
    border-color: var(--line-2);
    background: var(--surface);
    box-shadow:
      0 0 0 1px var(--line-2),
      0 10px 30px -18px rgba(0, 0, 0, 0.8);
  }
  .stop:not(.here) .card {
    background: color-mix(in srgb, var(--surface) 60%, var(--bg));
  }
  .reading .card {
    background: transparent;
  }
  .top {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 10px;
  }
  h2 {
    margin: 0;
    font-size: 16px;
    font-weight: 600;
  }
  .tag {
    padding: 1px 8px;
    border-radius: 99px;
    border: 1px solid var(--line-2);
    font-size: 11.5px;
    color: var(--text-2);
  }
  .tag.now {
    border-color: transparent;
    padding-left: 0;
    color: var(--text);
    font-weight: 600;
  }
  .tag.now::before {
    content: "← ";
    color: var(--text-2);
  }
  .goal {
    margin: -4px 0 0;
    font-size: 13px;
  }
  .skills {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(min(100%, 520px), 1fr));
    gap: 6px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .skills li {
    display: grid;
    grid-template-columns: 38px minmax(0, 1fr) auto auto;
    align-items: center;
    gap: 12px;
    padding: 8px 10px;
    border-radius: var(--r-sm);
    background: var(--surface-2);
  }
  .lvl {
    font-size: 12.5px;
    color: var(--text-2);
  }
  svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 1.7;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .sname {
    display: grid;
    min-width: 0;
  }
  .sname b {
    font-size: 13.5px;
    font-weight: 600;
  }
  .small {
    font-size: 12.5px;
  }
  .reads {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }
  .chip-btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 26px;
    padding: 0 10px;
    border-radius: 99px;
    border: 1px solid var(--line-2);
    background: transparent;
    font-size: 12.5px;
    cursor: pointer;
  }
  .chip-btn:hover {
    background: var(--surface-2);
  }
  .chip-btn.done {
    color: var(--text-2);
  }
  .chip-btn svg {
    stroke-width: 2.6;
  }
  .next {
    margin: 0;
  }
  @container (max-width: 640px) {
    .skills li {
      grid-template-columns: 38px minmax(0, 1fr) auto;
    }
    .skills li .lvl {
      grid-column: 2;
    }
    .skills li .btn {
      grid-column: 3;
      grid-row: 1 / span 2;
    }
  }
  .more-weak {
    margin-top: -6px;
  }
</style>
