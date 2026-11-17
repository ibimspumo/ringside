<script lang="ts">
  import { api } from "../api";
  import type { Exercise, LearnKind, LearnProgress, SkillProgress } from "../types";
  import { LEVEL_TEXT, SKILLS, TRAINABLE, errorText, isDue, nextLevel } from "./catalog";
  import Mastery from "./Mastery.svelte";
  import Ring from "./Ring.svelte";
  import Task from "./Task.svelte";

  let {
    progress,
    onProgress,
    onPath,
    onOwn,
    onTrainer,
  }: {
    progress: LearnProgress | null;
    onProgress: () => Promise<void>;
    onPath: () => void;
    onOwn: () => void;
    onTrainer: () => void;
  } = $props();

  type Why = "Wiederholen" | "Neu" | "Festigen";
  type Item = { kind: LearnKind; level: number; why: Why };
  const LENGTH = 8;
  const GOALS = [20, 50, 100];
  const GOAL_STORE = "ringside.learn.goal";

  let goal = $state(loadGoal());
  function loadGoal(): number {
    try {
      const g = Number(localStorage.getItem(GOAL_STORE));
      return GOALS.includes(g) ? g : 50;
    } catch {
      return 50;
    }
  }
  function setGoal(g: number) {
    goal = g;
    try {
      localStorage.setItem(GOAL_STORE, String(g));
    } catch {
      // ignore
    }
  }

  const byKind = $derived(new Map((progress?.skills ?? []).map((s) => [s.id, s])));

  /** Due reviews first, then something new, then the weakest skills; no skill twice in a row. */
  function buildPlan(p: LearnProgress | null): Item[] {
    const skills = TRAINABLE.map((k) => p?.skills.find((s) => s.id === k)).filter((s): s is SkillProgress => !!s);
    const items: Item[] = [];
    const due = skills.filter(isDue).sort((a, b) => a.level - b.level || (a.last_at ?? 0) - (b.last_at ?? 0));
    for (const s of due.slice(0, 4)) items.push({ kind: s.id, level: nextLevel(s), why: "Wiederholen" });
    const fresh = skills.filter((s) => s.attempts === 0).slice(0, 2);
    // A new skill gets two tries, so the first one is not the only impression.
    fresh.forEach((s, i) => {
      for (let n = 0; n < (i === 0 ? 2 : 1); n++) items.push({ kind: s.id, level: 1, why: "Neu" });
    });
    const weakest = skills
      .filter((s) => s.attempts > 0)
      .sort((a, b) => (a.mean_score ?? 0) - (b.mean_score ?? 0) || a.level - b.level);
    for (let i = 0; items.length < LENGTH && weakest.length; i++) {
      const s = weakest[i % weakest.length];
      items.push({ kind: s.id, level: nextLevel(s), why: "Festigen" });
    }
    for (let i = 0; items.length < LENGTH; i++) items.push({ kind: TRAINABLE[i % TRAINABLE.length], level: 1, why: "Neu" });
    return interleave(items.slice(0, LENGTH));
  }

  function interleave(items: Item[]): Item[] {
    const rest = [...items];
    const out: Item[] = [];
    while (rest.length) {
      const i = rest.findIndex((x) => x.kind !== out.at(-1)?.kind);
      out.push(...rest.splice(i < 0 ? 0 : i, 1));
    }
    return out;
  }

  const plan = $derived(buildPlan(progress));
  const planGroups = $derived(
    [...new Map(plan.map((x) => [x.kind, x])).values()].map((x) => ({ ...x, n: plan.filter((y) => y.kind === x.kind).length })),
  );

  let phase = $state<"start" | "run" | "done">("start");
  let items = $state<Item[]>([]);
  let index = $state(0);
  let exercise = $state<Exercise | null>(null);
  let error = $state<string | null>(null);
  let scores = $state<(number | null)[]>([]);
  let correct = $state<boolean[]>([]);
  let before = $state<Map<LearnKind, SkillProgress>>(new Map());

  async function load(i: number) {
    exercise = null;
    error = null;
    try {
      exercise = await api.learnExercise(items[i].kind, items[i].level);
    } catch (e) {
      error = errorText(e);
    }
  }

  function start() {
    items = plan;
    before = new Map(byKind);
    scores = items.map(() => null);
    correct = items.map(() => false);
    index = 0;
    phase = "run";
    load(0);
  }

  async function next() {
    if (index + 1 < items.length) {
      index++;
      load(index);
    } else {
      await finish();
    }
  }

  async function finish() {
    phase = "done";
    await onProgress();
  }

  const answered = $derived(scores.filter((s) => s !== null).length);
  const mean = $derived(answered ? scores.reduce((a: number, s) => a + (s ?? 0), 0) / answered : 0);
  const rights = $derived(correct.filter(Boolean).length);
  const practiced = $derived([...new Set(items.slice(0, Math.max(answered, 1)).map((x) => x.kind))]);
  const xpWeek = $derived(progress?.xp_week ?? 0);

  function tone(s: number | null) {
    if (s === null) return "";
    return s >= 0.999 ? "ok" : s >= 0.5 ? "close" : "miss";
  }
</script>

{#if phase === "start"}
  <div class="start">
    <section class="hero panel">
      <div class="intro">
        <span class="eyebrow">Heute lernen · etwa 5 Minuten</span>
        <h1>{LENGTH} Aufgaben, gemischt</h1>
        <p class="dim">
          Fällige Wiederholungen zuerst, dazu etwas Neues. Nach jeder Antwort siehst du sofort, warum. Fehler kosten nichts, sie
          kommen nur bald wieder.
        </p>
        <ul class="plan" aria-label="Plan für heute">
          {#each planGroups as g (g.kind)}
            <li>
              <span class="why {g.why === 'Neu' ? 'new' : g.why === 'Wiederholen' ? 'due' : ''}">{g.why}</span>
              <span class="name">{SKILLS[g.kind].short}</span>
              {#if g.n > 1}<span class="faint num">×{g.n}</span>{/if}
            </li>
          {/each}
        </ul>
        <button class="btn primary go" onclick={start} disabled={!progress}>
          Los geht’s
          <svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true"><path d="M5 12h14M13 6l6 6-6 6" /></svg>
        </button>
      </div>
      <div class="week">
        <Ring value={xpWeek / goal} size={132} stroke={10} label="{xpWeek} von {goal} Antworten diese Woche">
          <b class="num big">{xpWeek}</b>
          <span class="faint small">von {goal} XP</span>
        </Ring>
        <span class="dim small">Wochenziel</span>
        <div class="seg" role="group" aria-label="Wochenziel">
          {#each GOALS as g (g)}
            <button class:on={goal === g} onclick={() => setGoal(g)}>{g}</button>
          {/each}
        </div>
        <span class="faint tiny">1 XP pro Antwort, richtig oder falsch.</span>
      </div>
    </section>

    <div class="doors">
      <button class="door" onclick={onOwn}>
        <b>Aus deinen Händen</b>
        <span>Deine All-ins und Push/Fold-Spots noch einmal entscheiden.</span>
      </button>
      <button class="door" onclick={onPath}>
        <b>Lernpfad</b>
        <span>Elf Einheiten von den Grundlagen bis ICM, mit deinem Stand.</span>
      </button>
      <button class="door" onclick={onTrainer}>
        <b>Trainer</b>
        <span>Eine Fertigkeit gezielt üben, so lange du willst.</span>
      </button>
    </div>
  </div>
{:else if phase === "run"}
  <div class="run">
    <div class="bar">
      <span class="count num">{index + 1} / {items.length}</span>
      <ol class="steps" aria-label="Fortschritt der Sitzung">
        {#each items as it, i (i)}
          <li class={tone(scores[i])} class:now={i === index} title="{SKILLS[it.kind].short} · {it.why}"></li>
        {/each}
      </ol>
      <button class="btn ghost" onclick={finish}>Beenden</button>
    </div>
    <div class="panel body">
      {#if error}
        <p class="error" role="alert">{error}</p>
        <button class="btn" onclick={() => load(index)}>Nochmal versuchen</button>
      {:else if exercise}
        {#key `${index}:${exercise.seed}`}
          <Task
            {exercise}
            check={(a) => api.learnCheck(exercise as Exercise, a)}
            onGraded={(fb) => {
              scores[index] = fb.score;
              correct[index] = fb.correct;
            }}
            onNext={next}
            nextLabel={index + 1 < items.length ? "Weiter" : "Zum Ergebnis"}
          />
        {/key}
      {:else}
        <p class="faint loading">Aufgabe wird gemischt …</p>
      {/if}
    </div>
  </div>
{:else}
  <section class="done panel">
    <div class="result">
      <Ring value={mean} size={150} stroke={12} tone={mean >= 0.8 ? "var(--pos)" : mean >= 0.5 ? "var(--amber)" : "var(--neg)"} label="Punktzahl {Math.round(mean * 100)} %">
        <b class="num huge">{Math.round(mean * 100)}</b>
        <span class="faint small">Punkte</span>
      </Ring>
      <div class="numbers">
        <h1>{answered === 0 ? "Sitzung beendet" : mean >= 0.8 ? "Starke Runde" : mean >= 0.5 ? "Solide Runde" : "Gut, dass du’s geübt hast"}</h1>
        <div class="stats">
          <div><b class="num">{rights}</b><span>von {answered} richtig</span></div>
          <div><b class="num">+{answered}</b><span>XP</span></div>
          <div><b class="num">{xpWeek}</b><span>XP diese Woche</span></div>
        </div>
        <p class="dim small">Was danebenging, kommt in den nächsten Tagen wieder. So bleibt es hängen.</p>
      </div>
    </div>
    {#if answered}
      <ul class="changes">
        {#each practiced as k (k)}
          {@const b = before.get(k)}
          {@const a = byKind.get(k)}
          <li>
            <span class="name">{SKILLS[k].short}</span>
            <Mastery skill={a} />
            {#if a && b && a.level > b.level}
              <span class="up">Neue Stufe: {LEVEL_TEXT[a.level]}</span>
            {:else if a && b && a.level < b.level}
              <span class="faint">zurück auf {LEVEL_TEXT[a.level]}</span>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
    <div class="actions">
      <button class="btn primary" onclick={() => (phase = "start")}>Noch eine Runde</button>
      <button class="btn" onclick={onPath}>Zum Lernpfad</button>
    </div>
  </section>
{/if}

<style>
  .start {
    display: grid;
    gap: 14px;
  }
  .hero {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 28px;
    padding: 24px 26px;
    overflow: hidden;
    background:
      radial-gradient(circle at 92% 30%, rgba(236, 235, 231, 0.05), transparent 45%),
      var(--surface);
  }
  .eyebrow {
    font-size: 12.5px;
    color: var(--text-2);
  }
  h1 {
    margin: 4px 0 8px;
    font-family: var(--font-num);
    font-size: 28px;
    font-weight: 600;
  }
  .intro p {
    margin: 0;
    max-width: 58ch;
  }
  .plan {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin: 18px 0 20px;
    padding: 0;
    list-style: none;
  }
  .plan li {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    height: 30px;
    padding: 0 11px 0 4px;
    border-radius: 99px;
    background: var(--surface-2);
    border: 1px solid var(--line);
    font-size: 13px;
  }
  .why {
    padding: 2px 8px;
    border-radius: 99px;
    background: var(--surface-3);
    color: var(--text-2);
    font-size: 11.5px;
    font-weight: 600;
  }
  .why.new {
    background: var(--text);
    color: var(--bg);
  }
  .why.due {
    color: var(--text);
  }
  .go {
    height: 42px;
    padding: 0 22px;
    font-size: 14.5px;
  }
  .week {
    display: grid;
    justify-items: center;
    align-content: center;
    gap: 8px;
    min-width: 170px;
  }
  .big {
    font-size: 34px;
    font-weight: 600;
  }
  .huge {
    font-size: 44px;
    font-weight: 600;
  }
  .small {
    font-size: 12.5px;
  }
  .tiny {
    font-size: 11.5px;
    text-align: center;
  }
  .doors {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: 10px;
  }
  .door {
    display: grid;
    align-content: start;
    gap: 4px;
    padding: 14px 16px;
    border-radius: var(--r);
    border: 1px solid var(--line);
    background: var(--surface);
    text-align: left;
    cursor: pointer;
    transition:
      background 0.12s,
      border-color 0.12s,
      transform 0.12s;
  }
  .door:hover {
    background: var(--surface-2);
    border-color: var(--line-2);
    transform: translateY(-1px);
  }
  .door b {
    font-size: 14.5px;
  }
  .door span {
    color: var(--text-2);
    font-size: 13px;
  }

  .run {
    display: grid;
    gap: 12px;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 14px;
  }
  .count {
    min-width: 44px;
    font-size: 15px;
  }
  .steps {
    flex: 1;
    display: flex;
    gap: 4px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .steps li {
    flex: 1;
    height: 8px;
    border-radius: 99px;
    background: var(--surface-3);
    transition: background 0.3s;
  }
  .steps li.now {
    background: var(--text-2);
    animation: breathe 1.8s ease-in-out infinite;
  }
  .steps li.ok {
    background: var(--pos);
  }
  .steps li.close {
    background: var(--amber);
  }
  .steps li.miss {
    background: var(--neg);
  }
  @keyframes breathe {
    50% {
      opacity: 0.55;
    }
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

  .done {
    display: grid;
    gap: 20px;
    padding: 26px;
    animation: rise 0.4s cubic-bezier(0.2, 0.8, 0.2, 1) both;
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(10px);
    }
  }
  .result {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 26px;
  }
  .numbers {
    display: grid;
    gap: 10px;
    min-width: 0;
  }
  .numbers h1 {
    margin: 0;
  }
  .numbers p {
    margin: 0;
  }
  .stats {
    display: flex;
    flex-wrap: wrap;
    gap: 10px 28px;
  }
  .stats div {
    display: grid;
  }
  .stats b {
    font-size: 26px;
    font-weight: 600;
  }
  .stats span {
    color: var(--text-2);
    font-size: 12.5px;
  }
  .changes {
    display: grid;
    gap: 6px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .changes li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 16px;
    padding: 8px 12px;
    border-radius: var(--r-sm);
    background: var(--surface-2);
  }
  .changes .name {
    min-width: 150px;
    font-weight: 600;
  }
  .up {
    color: var(--pos);
    font-weight: 600;
    font-size: 13px;
    animation: rise 0.4s 0.3s both;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  @container (max-width: 640px) {
    .hero {
      grid-template-columns: 1fr;
      padding: 20px;
    }
    .body {
      padding: 16px;
    }
  }
</style>
