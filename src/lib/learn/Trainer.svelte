<script lang="ts">
  import { untrack } from "svelte";
  import { api } from "../api";
  import type { Exercise, LearnKind, LearnProgress } from "../types";
  import { SKILLS, TRAINABLE, errorText, fmtPct, nextLevel } from "./catalog";
  import Mastery from "./Mastery.svelte";
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
</script>

<div class="trainer">
  <aside class="skills" aria-label="Fertigkeit wählen">
    {#each TRAINABLE as k (k)}
      <button class="skill" class:on={kind === k} aria-pressed={kind === k} onclick={() => choose(k)}>
        <span class="ico" aria-hidden="true"><svg viewBox="0 0 24 24" width="17" height="17"><path d={SKILLS[k].icon} /></svg></span>
        <span class="txt">
          <b>{SKILLS[k].short}</b>
          <Mastery skill={byKind.get(k)} label={false} />
        </span>
      </button>
    {/each}
  </aside>

  <div class="main">
    <div class="bar panel">
      <div class="title">
        <h1>{byKind.get(kind)?.name_de ?? meta.short}</h1>
        <Mastery skill={byKind.get(kind)} />
      </div>
      <div class="seg levels" role="group" aria-label="Stufe">
        {#each [1, 2, 3] as l (l)}
          <button class:on={level === l} onclick={() => setLevel(l)} title={meta.levels[l - 1]}>Stufe {l}</button>
        {/each}
      </div>
      <p class="faint lvl-text">{meta.levels[level - 1]}</p>
      <div class="session">
        {#if count}
          <span><b class="num">{count}</b> Aufgaben</span>
          <span><b class="num">{right}</b> richtig</span>
          <span>Ø <b class="num">{fmtPct((sum / count) * 100)}</b></span>
        {:else}
          <span class="faint">Endlos üben. Wechsel Fertigkeit oder Stufe, wann du willst.</span>
        {/if}
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
    grid-template-columns: 210px minmax(0, 1fr);
    gap: 16px;
    align-items: start;
  }
  .skills {
    position: sticky;
    top: 0;
    display: grid;
    gap: 3px;
  }
  .skill {
    display: grid;
    grid-template-columns: 30px minmax(0, 1fr);
    align-items: center;
    gap: 10px;
    padding: 7px 9px;
    border: 1px solid transparent;
    border-radius: var(--r-sm);
    background: transparent;
    text-align: left;
    cursor: pointer;
  }
  .skill:hover {
    background: var(--surface);
  }
  .skill.on {
    background: var(--surface-2);
    border-color: var(--line-2);
  }
  .ico {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border-radius: 8px;
    background: var(--surface-3);
  }
  svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 1.7;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .txt {
    display: grid;
    gap: 3px;
    min-width: 0;
  }
  .txt b {
    font-size: 13px;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .main {
    display: grid;
    gap: 12px;
    min-width: 0;
  }
  .bar {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 8px 16px;
    padding: 14px 18px;
  }
  .title {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 14px;
    min-width: 0;
  }
  h1 {
    margin: 0;
    font-family: var(--font-num);
    font-size: 21px;
    font-weight: 600;
  }
  .levels {
    align-self: start;
  }
  .lvl-text {
    grid-column: 2;
    margin: -2px 0 0;
    font-size: 12px;
    text-align: right;
  }
  .session {
    grid-column: 1;
    grid-row: 2 / span 2;
    display: flex;
    flex-wrap: wrap;
    gap: 4px 16px;
    font-size: 13px;
    color: var(--text-2);
  }
  .session b {
    color: var(--text);
  }
  .reads {
    grid-column: 1 / -1;
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
    }
    .skill {
      grid-template-columns: 26px auto;
      padding: 5px 9px 5px 5px;
      border-color: var(--line);
    }
    .ico {
      width: 26px;
      height: 26px;
    }
  }
  @container (max-width: 560px) {
    .bar {
      grid-template-columns: 1fr;
    }
    .lvl-text,
    .session {
      grid-column: 1;
      grid-row: auto;
      text-align: left;
    }
    .body {
      padding: 16px;
    }
  }
</style>
