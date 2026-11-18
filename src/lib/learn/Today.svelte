<script lang="ts">
  import { api } from "../api";
  import type { ConceptCount } from "../coach/types";
  import type { Exercise, LearnKind, LearnProgress, SkillProgress } from "../types";
  import { LEVEL_TEXT, SKILLS, TRAINABLE, currentUnit, errorText, isDue, nextLevel, trainableKind, weakConcepts } from "./catalog";
  import Ring from "./Ring.svelte";
  import SkillBadge from "./SkillBadge.svelte";
  import Task from "./Task.svelte";
  import { openPlay } from "../play/nav.svelte";
  import { preview } from "./preview";

  let {
    progress,
    concepts,
    onProgress,
    onPath,
    onOwn,
    onTrainer,
  }: {
    progress: LearnProgress | null;
    concepts: ConceptCount[];
    onProgress: () => Promise<void>;
    onPath: () => void;
    onOwn: () => void;
    onTrainer: () => void;
  } = $props();

  type Why = "Schwachstelle" | "Wiederholen" | "Neu" | "Festigen";
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

  /** Weak spots from coach reports first, then due reviews, something new and the weakest skills; no skill twice in a row. */
  function buildPlan(p: LearnProgress | null, weak: ConceptCount[]): Item[] {
    const skills = TRAINABLE.map((k) => p?.skills.find((s) => s.id === k)).filter((s): s is SkillProgress => !!s);
    const items: Item[] = [];
    // The two weakest concepts with an exercise get two tasks each.
    const weakKinds = [...new Set(weakConcepts(weak).map(trainableKind).filter((k): k is LearnKind => !!k))].slice(0, 2);
    for (const k of weakKinds) {
      const s = skills.find((x) => x.id === k);
      for (let n = 0; n < 2; n++) items.push({ kind: k, level: nextLevel(s), why: "Schwachstelle" });
    }
    const due = skills.filter(isDue).sort((a, b) => a.level - b.level || (a.last_at ?? 0) - (b.last_at ?? 0));
    for (const s of due.filter((x) => !weakKinds.includes(x.id)).slice(0, 4)) items.push({ kind: s.id, level: nextLevel(s), why: "Wiederholen" });
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

  const plan = $derived(buildPlan(progress, concepts));
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

  /** Exercises fetched ahead by index: table push/fold may solve for seconds. */
  let ahead = new Map<number, Promise<Exercise>>();
  function fetchAt(i: number): Promise<Exercise> {
    const p = ahead.get(i) ?? api.learnExercise(items[i].kind, items[i].level);
    ahead.delete(i);
    return p;
  }

  async function load(i: number) {
    exercise = null;
    error = null;
    try {
      const ex = await fetchAt(i);
      if (i !== index) return;
      exercise = ex;
      if (i + 1 < items.length && !ahead.has(i + 1)) {
        const next = api.learnExercise(items[i + 1].kind, items[i + 1].level);
        next.catch(() => {
          // Reported when it is used.
        });
        ahead.set(i + 1, next);
      }
    } catch (e) {
      error = errorText(e);
    }
  }

  function start() {
    ahead = new Map();
    items = plan;
    before = new Map(byKind);
    xpBefore = progress?.xp_week ?? 0;
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
  /** Progress reloads after a round; until it has, this round's answers still count toward the week. */
  let xpBefore = $state(0);
  const xpWeek = $derived(Math.max(progress?.xp_week ?? 0, phase === "done" ? xpBefore + answered : 0));
  /** This round's answers grouped by skill, in the order they came. */
  const trail = $derived(
    [...new Set(items.slice(0, answered).map((x) => x.kind))].map((kind) => ({
      kind,
      at: items.slice(0, answered).flatMap((x, i) => (x.kind === kind ? [i] : [])),
    })),
  );

  const previewSession = preview("session");
  $effect(() => {
    if (!progress || !previewSession || phase !== "start") return;
    start();
    if (previewSession !== "done") return;
    const demo = [1, 1, 0.6, 1, 0, 1, 0.85, 1];
    scores = items.map((_, i) => demo[i % demo.length]);
    correct = scores.map((s) => (s ?? 0) >= 0.999);
    phase = "done";
  });

  const LANES: { why: Why; label: string }[] = [
    { why: "Schwachstelle", label: "Zuerst: deine Schwachstellen" },
    { why: "Wiederholen", label: "Wiederholen, damit es sitzt" },
    { why: "Festigen", label: "Festigen" },
    { why: "Neu", label: "Neu für dich" },
  ];
  const lanes = $derived(LANES.map((l) => ({ ...l, groups: planGroups.filter((g) => g.why === l.why) })).filter((l) => l.groups.length));
  const unit = $derived(currentUnit(byKind));
  /** Why a weak skill is in the round, from the coach concept behind it (weakest first). */
  const weakWhy = $derived(
    new Map(
      weakConcepts(concepts)
        .map((c) => [trainableKind(c), c] as const)
        .filter((x): x is readonly [LearnKind, ConceptCount] => !!x[0])
        .reverse(),
    ),
  );
  function whyText(kind: LearnKind): string | null {
    const c = weakWhy.get(kind);
    if (!c) return null;
    if (c.mistakes) return `${c.mistakes} Fehler`;
    if (c.questionable) return `${c.questionable} fraglich`;
    return null;
  }
  const left = $derived(Math.max(0, goal - xpWeek));

  function tone(s: number | null) {
    if (s === null) return "";
    return s >= 0.999 ? "ok" : s >= 0.5 ? "close" : "miss";
  }
</script>

{#if phase === "start"}
  <div class="start">
    <section class="hero panel">
      <div class="intro">
        <span class="eyebrow">Deine Runde für heute · etwa 5 Minuten</span>
        <h2 class="lead-title">{LENGTH} kurze Aufgaben</h2>
        <p class="dim lead">
          {#if lanes.some((l) => l.why === "Schwachstelle")}Zusammengestellt aus deinen Coach-Berichten und deinem Stand.{:else}Zusammengestellt aus
            deinem Stand.{/if} Nach jeder Antwort siehst du sofort, warum.
        </p>
        <div class="cta">
          <button class="btn primary go" onclick={start} disabled={!progress}>
            Runde starten
            <svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true"><path d="M5 12h14M13 6l6 6-6 6" /></svg>
          </button>
          <span class="dim small">Fehler kosten nichts. Was danebengeht, kommt bald zur Übung wieder.</span>
        </div>
        <div class="plan" aria-label="Plan für heute">
          {#each lanes as l (l.why)}
            <div class="lane">
              <span class="lane-label" class:weak={l.why === "Schwachstelle"}>
                {l.label}
                {#if l.why === "Schwachstelle"}<button class="lane-link" onclick={onPath}>alle im Lernpfad</button>{/if}
              </span>
              <ul>
                {#each l.groups as g (g.kind)}
                  <li>
                    <SkillBadge kind={g.kind} skill={byKind.get(g.kind)} size={34} />
                    <span class="pname">
                      <b>{SKILLS[g.kind].short}</b>
                      <span class="dim"
                        >{LEVEL_TEXT[byKind.get(g.kind)?.level ?? 0]} · {g.n === 1 ? "1 Aufgabe" : `${g.n} Aufgaben`}{g.why === "Schwachstelle" && whyText(g.kind) ? ` · ${whyText(g.kind)} im Coach` : ""}</span
                      >
                    </span>
                  </li>
                {/each}
              </ul>
            </div>
          {/each}
        </div>
      </div>
      <aside class="week" aria-label="Wochenziel">
        <Ring value={xpWeek / goal} size={128} stroke={10} tone={xpWeek >= goal ? "var(--pos)" : "var(--text)"} label="{xpWeek} von {goal} Antworten diese Woche">
          <b class="num big">{xpWeek}</b>
          <span class="dim small">von {goal}</span>
        </Ring>
        <div class="week-text">
          <b>Antworten diese Woche</b>
          <span class="dim small">
            {#if left === 0}Wochenziel geschafft. Stark!{:else}Noch {left} bis zum Ziel, etwa {Math.ceil(left / LENGTH)}
              {Math.ceil(left / LENGTH) === 1 ? "Runde" : "Runden"}.{/if}
          </span>
        </div>
        <div class="goal">
          <span class="dim small">Ziel</span>
          <div class="seg" role="group" aria-label="Wochenziel in Antworten">
            {#each GOALS as g (g)}
              <button class:on={goal === g} onclick={() => setGoal(g)}>{g}</button>
            {/each}
          </div>
        </div>
      </aside>
    </section>

    <div class="doors">
      <button class="door" onclick={onOwn}>
        <span class="door-ico" aria-hidden="true"><svg viewBox="0 0 24 24" width="20" height="20"><path d="M10 4h8a1 1 0 0 1 1 1v11a1 1 0 0 1-1 1h-8a1 1 0 0 1-1-1V5a1 1 0 0 1 1-1zM6 7.5V19a1 1 0 0 0 1 1h8" /></svg></span>
        <span class="door-txt">
          <b>Deine Hände</b>
          <span>Deine All-ins und Push/Fold-Spots noch einmal entscheiden.</span>
        </span>
      </button>
      <button class="door" onclick={onPath}>
        <span class="door-ico" aria-hidden="true"><svg viewBox="0 0 24 24" width="20" height="20"><path d="M6 19a2 2 0 1 0 0-4 2 2 0 0 0 0 4zM18 9a2 2 0 1 0 0-4 2 2 0 0 0 0 4zM8 17h6a3 3 0 0 0 0-6h-4a3 3 0 0 1 0-6h6" /></svg></span>
        <span class="door-txt">
          <b>Lernpfad</b>
          <span>{#if unit}Weiter bei Einheit {unit.n}: {unit.title}.{:else}Elf Einheiten von den Grundlagen bis ICM.{/if}</span>
        </span>
      </button>
      <button class="door" onclick={onTrainer}>
        <span class="door-ico" aria-hidden="true"><svg viewBox="0 0 24 24" width="20" height="20"><path d="M4 12h3l2-6 4 12 2-6h5" /></svg></span>
        <span class="door-txt">
          <b>Trainer</b>
          <span>Eine Fertigkeit gezielt üben, so lange du willst.</span>
        </span>
      </button>
      <button class="door" onclick={openPlay}>
        <b>Ganze Hände spielen</b>
        <span>Zufällige Hände am Tisch, jede Entscheidung sofort bewertet.</span>
      </button>
    </div>
  </div>
{:else if phase === "run"}
  <div class="run">
    <div class="bar">
      <span class="count num">{index + 1} / {items.length}</span>
      <ol class="steps" aria-label="Fortschritt der Runde">
        {#each items as it, i (i)}
          <li class={tone(scores[i])} class:now={i === index} title="{SKILLS[it.kind].short} · {it.why}"></li>
        {/each}
      </ol>
      <button class="btn ghost" onclick={finish}>Runde beenden</button>
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
      <div class="score">
      <Ring value={mean} size={150} stroke={12} tone={mean >= 0.8 ? "var(--pos)" : mean >= 0.5 ? "var(--amber)" : "var(--neg)"} label="Punktzahl {Math.round(mean * 100)} von 100">
        <b class="num huge">{Math.round(mean * 100)}</b>
        <span class="faint small">von 100 Punkten</span>
      </Ring>
      <p class="ring-note dim small">Schnitt deiner Antworten.<br />Knapp daneben gibt Teilpunkte.</p>
      </div>
      <div class="numbers">
        <h2 class="lead-title">{answered === 0 ? "Runde beendet" : mean >= 0.8 ? "Starke Runde!" : mean >= 0.5 ? "Solide Runde" : "Gut, dass du’s geübt hast"}</h2>
        <div class="stats">
          <div><b class="num">{rights}<span class="of">/{answered}</span></b><span>richtig</span></div>
          <div><b class="num">+{answered}</b><span>Antworten fürs Wochenziel</span></div>
          <div><b class="num">{xpWeek}<span class="of">/{goal}</span></b><span>diese Woche</span></div>
        </div>
        <p class="dim small">
          Was danebenging, kommt in den nächsten Tagen wieder. So bleibt es hängen.
        </p>
      </div>
    </div>
    {#if answered}
      <div class="grown">
        <div class="grown-head">
          <h2>Deine Antworten nach Fertigkeit</h2>
          <div class="legend" aria-hidden="true">
            <span>Ein Kästchen je Aufgabe:</span>
            <span><i class="ok">✓</i> richtig</span>
            <span><i class="close">≈</i> teilweise richtig</span>
            <span><i class="miss">×</i> daneben</span>
          </div>
        </div>
        <ul class="changes">
          {#each practiced as k (k)}
            {@const b = before.get(k)}
            {@const a = byKind.get(k)}
            <li class:up={!!(a && b && a.level > b.level)}>
              <SkillBadge kind={k} skill={a} size={44} />
              <span class="cname">
                <b>{SKILLS[k].short}</b>
                {#if a && b && a.level > b.level}
                  <span class="lvl-up">Neue Stufe: {LEVEL_TEXT[a.level]}</span>
                {:else if a && b && a.level < b.level}
                  <span class="faint">Zurück auf {LEVEL_TEXT[a.level]}</span>
                {:else}
                  <span class="dim">{LEVEL_TEXT[a?.level ?? 0]}</span>
                {/if}
                <span class="dots" aria-label="Antworten in dieser Runde">
                  {#each trail.find((t) => t.kind === k)?.at ?? [] as i (i)}<i class={tone(scores[i])} title="{Math.round((scores[i] ?? 0) * 100)} Punkte"
                      >{tone(scores[i]) === "ok" ? "✓" : tone(scores[i]) === "close" ? "≈" : "×"}</i
                    >{/each}
                </span>
              </span>
            </li>
          {/each}
        </ul>
      </div>
    {/if}
    <div class="actions">
      <button class="btn primary big-btn" onclick={() => (phase = "start")}>Noch eine Runde</button>
      <button class="btn big-btn" onclick={onPath}>Zum Lernpfad</button>
    </div>
  </section>
{/if}

<style>
  .start {
    display: grid;
    gap: 14px;
  }
  @container (min-width: 1800px) {
    .start {
      grid-template-columns: minmax(0, 1fr) 340px;
      align-items: stretch;
    }
    .doors {
      grid-template-columns: 1fr;
      align-content: start;
    }
  }

  .hero {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 260px;
    overflow: hidden;
    padding: 0;
  }
  .intro {
    display: grid;
    align-content: start;
    gap: 0;
    padding: 26px 28px 24px;
    min-width: 0;
  }
  .eyebrow {
    font-size: 12.5px;
    color: var(--text-2);
  }
  .lead-title {
    margin: 4px 0 6px;
    font-family: var(--font-num);
    font-size: var(--fs-3xl);
    font-weight: 600;
    letter-spacing: -0.01em;
  }
  .lead {
    margin: 0;
    max-width: 62ch;
  }
  .plan {
    display: flex;
    flex-wrap: wrap;
    gap: 14px 28px;
    margin: 24px 0 0;
  }
  .lane {
    display: grid;
    align-content: start;
    gap: 8px;
  }
  .lane-label {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-2);
  }
  .lane-label.weak {
    color: var(--neg);
  }
  .lane-link {
    margin-left: 8px;
    padding: 0;
    border: 0;
    background: none;
    color: var(--text-2);
    font-size: 12px;
    font-weight: 400;
    text-decoration: underline;
    text-decoration-color: var(--line-2);
    text-underline-offset: 3px;
    cursor: pointer;
  }
  .lane-link:hover {
    color: var(--text);
  }
  .lane ul {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .lane li {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 5px 14px 5px 5px;
    border-radius: 99px;
    background: var(--surface-2);
    border: 1px solid var(--line);
  }
  .pname {
    display: grid;
    line-height: 1.25;
  }
  .pname b {
    font-size: 13.5px;
    font-weight: 600;
  }
  .pname .dim {
    font-size: 11.5px;
  }
  .cta {
    margin-top: 18px;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px 18px;
  }
  .go {
    height: 44px;
    padding: 0 24px;
    font-size: 15px;
    border-radius: var(--r);
  }
  .week {
    display: grid;
    justify-items: center;
    align-content: center;
    gap: 14px;
    padding: 24px 20px;
    border-left: 1px solid var(--line);
    background: var(--surface-2);
    text-align: center;
  }
  .week-text {
    display: grid;
    gap: 2px;
    text-wrap: balance;
  }
  .week-text b {
    font-size: 13.5px;
    font-weight: 600;
  }
  .goal {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .big {
    font-size: 34px;
    font-weight: 600;
  }
  .huge {
    font-size: 46px;
    font-weight: 600;
  }
  .small {
    font-size: 12.5px;
  }
  .doors {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(230px, 1fr));
    gap: 10px;
  }
  .door {
    display: flex;
    align-items: flex-start;
    gap: 14px;
    padding: 16px 18px;
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
  .door-ico {
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    flex-shrink: 0;
    border-radius: 10px;
    background: var(--surface-3);
  }
  .door-txt {
    display: grid;
    gap: 3px;
  }
  .door b {
    font-size: 14.5px;
  }
  .door-txt span {
    color: var(--text-2);
    font-size: 13px;
    line-height: 1.4;
  }
  svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 1.7;
    stroke-linecap: round;
    stroke-linejoin: round;
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
    gap: 24px;
    padding: 28px;
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
    align-items: center;
    gap: 22px 32px;
  }
  .score {
    display: grid;
    justify-items: center;
    gap: 10px;
  }
  .ring-note {
    margin: 0;
    text-align: center;
  }
  .numbers {
    display: grid;
    gap: 14px;
    min-width: 0;
  }
  .numbers .lead-title,
  .numbers p {
    margin: 0;
  }
  .dots {
    display: inline-flex;
    gap: 3px;
    margin-top: 3px;
  }
  .legend span {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }
  .dots i,
  .legend i {
    --t: var(--text-3);
    display: grid;
    place-items: center;
    width: 14px;
    height: 14px;
    border-radius: 4px;
    background: var(--t);
    color: var(--bg);
    font-size: 10px;
    font-style: normal;
    font-weight: 700;
    line-height: 1;
  }
  .dots i.ok,
  .legend i.ok {
    --t: var(--pos);
  }
  .dots i.close,
  .legend i.close {
    --t: var(--amber);
  }
  .dots i.miss,
  .legend i.miss {
    --t: var(--neg);
  }
  @keyframes pop {
    from {
      opacity: 0;
      transform: scale(0.7);
    }
  }
  .grown-head {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    justify-content: space-between;
    gap: 6px 18px;
  }
  .legend {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 14px;
    font-size: 12px;
    color: var(--text-2);
  }
  .stats {
    display: flex;
    flex-wrap: wrap;
    gap: 10px 32px;
  }
  .stats div {
    display: grid;
  }
  .stats b {
    font-size: 26px;
    font-weight: 600;
  }
  .of {
    font-size: 16px;
    color: var(--text-3);
    font-weight: 500;
  }
  .stats div > span {
    color: var(--text-2);
    font-size: 12.5px;
  }
  .grown {
    display: grid;
    gap: 10px;
    padding-top: 20px;
    border-top: 1px solid var(--line);
  }
  .grown h2 {
    margin: 0;
    font-size: 14.5px;
    font-weight: 600;
  }
  .changes {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(max(200px, calc((100% - 16px) / 3)), 1fr));
    gap: 8px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .changes li {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 14px 10px 10px;
    border-radius: var(--r);
    background: var(--surface-2);
    border: 1px solid var(--line);
  }
  .changes li.up {
    border-color: color-mix(in srgb, var(--pos) 45%, transparent);
    background: color-mix(in srgb, var(--pos) 8%, var(--surface-2));
  }
  .cname {
    display: grid;
    gap: 1px;
    min-width: 0;
    font-size: 12.5px;
  }
  .cname b {
    font-size: 14px;
  }
  .lvl-up {
    color: var(--pos);
    font-weight: 600;
    animation: rise 0.4s 0.3s both;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .big-btn {
    height: 40px;
    padding: 0 20px;
    font-size: 14px;
  }
  @container (max-width: 560px) {
    .result {
      flex-wrap: wrap;
    }
  }
  @container (max-width: 1100px) {
    .changes {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }
  @container (max-width: 760px) {
    .hero {
      grid-template-columns: 1fr;
    }
    .week {
      grid-template-columns: auto minmax(0, 1fr);
      justify-items: start;
      text-align: left;
      border-left: 0;
      border-top: 1px solid var(--line);
    }
    .goal {
      grid-column: 1 / -1;
    }
    .intro {
      padding: 20px;
    }
    .body {
      padding: 16px;
    }
  }
</style>
