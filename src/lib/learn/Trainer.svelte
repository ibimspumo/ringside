<script lang="ts">
  import { untrack } from "svelte";
  import { api } from "../api";
  import type { Exercise, LearnKind, LearnProgress } from "../types";
  import { LEVEL_TEXT, SKILLS, TRAINABLE, UNITS, errorText, fmtPct, nextLevel } from "./catalog";
  import SkillBadge from "./SkillBadge.svelte";
  import Task from "./Task.svelte";
  import { ARTICLE_BY_ID } from "./wissen/articles";

  let {
    progress,
    pick,
    onProgress,
    onRead,
  }: {
    progress: LearnProgress | null;
    pick: { kind: LearnKind; level: number } | null;
    onProgress: () => Promise<void>;
    onRead: (id: string) => void;
  } = $props();

  const byKind = $derived(new Map((progress?.skills ?? []).map((s) => [s.id, s])));

  let kind = $state<LearnKind>(untrack(() => pick?.kind ?? "outs"));
  let level = $state(untrack(() => pick?.level ?? 1));
  let exercise = $state<Exercise | null>(null);
  let error = $state<string | null>(null);
  let count = $state(0);
  let sum = $state(0);
  let right = $state(0);
  let round = $state(0);

  /** The next exercise, fetched while the current one is answered: table push/fold may solve for seconds. */
  let queued: { key: string; ex: Promise<Exercise> } | null = null;

  async function load() {
    const k = kind;
    const l = level;
    const key = `${k}:${l}`;
    exercise = null;
    error = null;
    try {
      const pending = queued?.key === key ? queued.ex : api.learnExercise(k, l);
      queued = null;
      const ex = await pending;
      if (k !== kind || l !== level) return;
      exercise = ex;
      queued = { key, ex: api.learnExercise(k, l) };
      queued.ex.catch(() => {
        // Reported when it is used.
      });
    } catch (e) {
      error = errorText(e);
    }
  }

  $effect(() => {
    void kind;
    void level;
    void round;
    untrack(load);
  });

  function choose(k: LearnKind) {
    if (k === kind) return;
    kind = k;
    level = nextLevel(byKind.get(k));
    count = sum = right = 0;
  }

  function setLevel(l: number) {
    level = l;
    count = sum = right = 0;
  }

  const meta = $derived(SKILLS[kind]);
  /** Trainable skills grouped by their unit on the path. */
  const GROUPS = UNITS.map((u) => ({ title: u.title, kinds: TRAINABLE.filter((k) => u.skills.includes(k)) })).filter((g) => g.kinds.length);
</script>

<div class="trainer">
  <nav class="skills" aria-label="Fertigkeit wählen">
    {#each GROUPS as g (g.title)}
      <div class="group">
        <span class="gtitle">{g.title}</span>
        {#each g.kinds as k (k)}
          <button class="skill" class:on={kind === k} aria-pressed={kind === k} onclick={() => choose(k)}>
            <SkillBadge kind={k} skill={byKind.get(k)} size={32} />
            <span class="txt">
              <b>{SKILLS[k].short}</b>
              <span class="dim">{LEVEL_TEXT[byKind.get(k)?.level ?? 0]}</span>
            </span>
          </button>
        {/each}
      </div>
    {/each}
  </nav>

  <div class="main">
    <div class="bar panel">
      <div class="title">
        <SkillBadge {kind} skill={byKind.get(kind)} size={46} />
        <div class="tt">
          <h1>{meta.short}</h1>
          <span class="session">
            {#if count}
              Diese Sitzung: <b class="num">{count}</b> {count === 1 ? "Aufgabe" : "Aufgaben"} · <b class="num">{right}</b> richtig · <b class="num"
                >{fmtPct((sum / count) * 100)}</b
              > im Schnitt
            {:else}
              Endlos üben. Wechsle Fertigkeit oder Stufe, wann du willst.
            {/if}
          </span>
        </div>
      </div>
      <div class="levels" role="group" aria-label="Stufe">
        {#each [1, 2, 3] as l (l)}
          <button class="level" class:on={level === l} aria-pressed={level === l} onclick={() => setLevel(l)}>
            <span class="pips" aria-hidden="true">{#each [1, 2, 3] as n (n)}<i class:on={n <= l}></i>{/each}</span>
            <b>Stufe {l}</b>
            <span>{meta.levels[l - 1]}</span>
          </button>
        {/each}
      </div>
      {#if meta.articles.length}
        <div class="reads">
          <span class="faint">Nachlesen:</span>
          {#each meta.articles as id (id)}
            {#if ARTICLE_BY_ID[id]}<button class="link" onclick={() => onRead(id)}>{ARTICLE_BY_ID[id].title}</button>{/if}
          {/each}
        </div>
      {/if}
    </div>

    <div class="panel body">
      {#if error}
        <p class="error" role="alert">{error}</p>
        <button class="btn" onclick={load}>Nochmal versuchen</button>
      {:else if exercise}
        {#key `${exercise.kind}:${exercise.level}:${exercise.seed}`}
          <Task
            {exercise}
            check={(a) => api.learnCheck(exercise as Exercise, a)}
            onGraded={(fb) => {
              count++;
              sum += fb.score;
              if (fb.correct) right++;
              onProgress();
            }}
            onNext={() => round++}
            nextLabel="Nächste Aufgabe"
          />
        {/key}
      {:else}
        <p class="faint loading">Aufgabe wird gemischt …</p>
      {/if}
    </div>
  </div>
</div>

<style>
  .trainer {
    display: grid;
    grid-template-columns: 236px minmax(0, 1fr);
    gap: 18px;
    align-items: start;
  }
  .skills {
    position: sticky;
    top: 0;
    display: grid;
    gap: 14px;
  }
  .group {
    display: grid;
    gap: 2px;
  }
  .gtitle {
    padding: 0 10px 4px;
    font-size: 11.5px;
    font-weight: 600;
    color: var(--text-3);
  }
  .skill {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 5px 10px 5px 6px;
    border: 1px solid transparent;
    border-radius: var(--r);
    background: transparent;
    text-align: left;
    cursor: pointer;
    transition: background 0.12s;
  }
  .skill:hover {
    background: var(--surface);
  }
  .skill.on {
    background: var(--surface-2);
    border-color: var(--line-2);
  }
  .txt {
    display: grid;
    min-width: 0;
    line-height: 1.25;
  }
  .txt b {
    font-size: 13px;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .txt span {
    font-size: 11.5px;
  }
  .main {
    display: grid;
    gap: 12px;
    min-width: 0;
  }
  .bar {
    display: grid;
    gap: 14px;
    padding: 16px 18px;
  }
  .title {
    display: flex;
    align-items: center;
    gap: 14px;
    min-width: 0;
  }
  .tt {
    display: grid;
    gap: 2px;
    min-width: 0;
  }
  h1 {
    margin: 0;
    font-family: var(--font-num);
    font-size: 22px;
    font-weight: 600;
  }
  .session {
    font-size: 13px;
    color: var(--text-2);
  }
  .session b {
    color: var(--text);
  }
  .levels {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 8px;
  }
  .level {
    display: grid;
    justify-items: start;
    align-content: start;
    gap: 3px;
    padding: 10px 12px;
    border-radius: var(--r);
    border: 1px solid var(--line);
    background: var(--surface-2);
    text-align: left;
    cursor: pointer;
    transition:
      border-color 0.12s,
      background 0.12s;
  }
  .level:hover {
    border-color: var(--line-2);
  }
  .level.on {
    border-color: var(--text-2);
    background: var(--surface-3);
  }
  .level b {
    font-size: 13px;
  }
  .level > span:last-child {
    font-size: 12px;
    color: var(--text-2);
  }
  .pips {
    display: flex;
    gap: 3px;
    margin-bottom: 2px;
  }
  .pips i {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--surface-3);
  }
  .level.on .pips i {
    background: var(--line-2);
  }
  .pips i.on,
  .level.on .pips i.on {
    background: var(--text);
  }
  .reads {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 12px;
    font-size: 12.5px;
  }
  .link {
    padding: 0;
    border: 0;
    background: none;
    color: var(--text);
    text-decoration: underline;
    text-decoration-color: var(--line-2);
    text-underline-offset: 3px;
    cursor: pointer;
  }
  .link:hover {
    text-decoration-color: var(--text);
  }
  .body {
    padding: 20px 22px;
  }
  .loading {
    margin: 30px 0;
    text-align: center;
  }
  .error {
    margin: 0 0 10px;
    color: #ff9d93;
  }
  @container (max-width: 820px) {
    .trainer {
      grid-template-columns: 1fr;
    }
    .skills {
      position: static;
      display: flex;
      flex-wrap: wrap;
      gap: 6px;
    }
    .group {
      display: contents;
    }
    .gtitle {
      display: none;
    }
    .skill {
      padding: 3px 12px 3px 3px;
      border-color: var(--line);
      border-radius: 99px;
    }
    .txt span {
      display: none;
    }
  }
  @container (max-width: 560px) {
    .levels {
      grid-template-columns: 1fr;
    }
    .body {
      padding: 16px;
    }
  }
</style>
