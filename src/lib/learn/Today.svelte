<script lang="ts">
  import { untrack } from "svelte";
  import Button from "../ui/Button.svelte";
  import EmptyState from "../ui/EmptyState.svelte";
  import { api } from "../api";
  import type { ConceptCount } from "../coach/types";
  import type { Exercise, LearnKind, LearnProgress, SkillProgress } from "../types";
  import { LEVEL_TEXT, SKILLS, TRAINABLE, currentUnit, errorText, isDue, nextLevel, trainableKind, weakConcepts } from "./catalog";
  import Ring from "./Ring.svelte";
  import SkillBadge from "./SkillBadge.svelte";
  import Task from "./Task.svelte";
  import { openPlay } from "../play/nav.svelte";
  import { preview } from "./preview";
  import DecisionDrill from "./DecisionDrill.svelte";
  import { bbText, costLine, type DrillSpot, type PlayProfile } from "./profile";
  import type { LearnTarget } from "../app/Leaks.svelte";
  import type { NextStep, StepHands } from "../next/types";
  import Weaknesses from "./Weaknesses.svelte";

  let {
    progress,
    concepts,
    profile,
    steps,
    focus = null,
    onFocusDone = () => {},
    onProgress,
    onPath,
    onOwn,
    onTrainer,
    onSpots,
    onLearn,
    onHands,
  }: {
    progress: LearnProgress | null;
    concepts: ConceptCount[];
    /** The playing profile; null while it loads or while locked. */
    profile: PlayProfile | null;
    /** „Was als Nächstes?“, heaviest first. */
    steps: NextStep[];
    /** A kind of spot to practise right away (from "Üben" elsewhere). */
    focus?: string | null;
    onFocusDone?: () => void;
    onProgress: () => Promise<void>;
    onPath: () => void;
    onOwn: () => void;
    onTrainer: () => void;
    /** „Wichtige Spots“ in the learning area. */
    onSpots: () => void;
    onLearn: (target: LearnTarget) => void;
    onHands?: (hands: StepHands) => void;
  } = $props();

  type Why = "Spiel" | "Tische" | "Schwachstelle" | "Wiederholen" | "Neu" | "Festigen";
  type SkillItem = { type: "skill"; kind: LearnKind; level: number; why: Why };
  /** A decision of the playing profile: a real one, or a variation with another hand. */
  type DrillItem = { type: "drill"; key: string; real: boolean; why: "Spiel" };
  type Item = SkillItem | DrillItem;
  type Current = { type: "skill"; exercise: Exercise } | { type: "drill"; spot: DrillSpot };
  const LENGTH = 8;
  /** Tasks of a round on one kind of spot. */
  const FOCUS_LENGTH = 6;
  const GOALS = [20, 50, 100];
  /** Rated decisions at tables of two to five players before their own exercise joins the plan. */
  const SHORT_TABLES_MIN = 5;
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
    }
  }

  const byKind = $derived(new Map((progress?.skills ?? []).map((s) => [s.id, s])));

  /** What the item trains: the skill, or "own_decision" for a spot of the playing profile. */
  const kindOf = (x: Item): LearnKind => (x.type === "skill" ? x.kind : "own_decision");
  /** Items of the same id are not asked twice in a row. */
  const idOf = (x: Item) => (x.type === "skill" ? x.kind : x.key);

  /**
   * In the order of „Was als Nächstes?“: its first kinds of spot from the user's own hands (a real
   * decision and a variation of each), opening at short tables when the history has them, then its
   * exercises for weak spots, due reviews, something new and the weakest skills.
   */
  function buildPlan(p: LearnProgress | null, steps: NextStep[], short: number): Item[] {
    const skills = TRAINABLE.map((k) => p?.skills.find((s) => s.id === k)).filter((s): s is SkillProgress => !!s);
    const items: Item[] = [];
    const top = own(steps).slice(0, 2);
    for (const key of top) {
      items.push({ type: "drill", key, real: true, why: "Spiel" });
      items.push({ type: "drill", key, real: false, why: "Spiel" });
    }
    if (top.length === 1) items.push({ type: "drill", key: top[0], real: false, why: "Spiel" });
    if (short >= SHORT_TABLES_MIN) {
      const s = skills.find((x) => x.id === "open_fold_short");
      for (let n = 0; n < 2; n++) items.push({ type: "skill", kind: "open_fold_short", level: nextLevel(s), why: "Tische" });
    }
    // With spots from the own game, coach concepts get one task each, else two.
    const perConcept = top.length ? 1 : 2;
    const weakKinds = [...new Set(steps.filter((s) => s.kind !== "review").flatMap((s) => (s.practice && "kind" in s.practice ? [s.practice.kind] : [])))].slice(0, 2);
    for (const k of weakKinds) {
      const s = skills.find((x) => x.id === k);
      for (let n = 0; n < perConcept; n++) items.push({ type: "skill", kind: k, level: nextLevel(s), why: "Schwachstelle" });
    }
    const due = skills.filter(isDue).sort((a, b) => a.level - b.level || (a.last_at ?? 0) - (b.last_at ?? 0));
    for (const s of due.filter((x) => !weakKinds.includes(x.id)).slice(0, 4))
      items.push({ type: "skill", kind: s.id, level: nextLevel(s), why: "Wiederholen" });
    const fresh = skills.filter((s) => s.attempts === 0).slice(0, 2);
    // A new skill gets two tries, so the first one is not the only impression.
    fresh.forEach((s, i) => {
      for (let n = 0; n < (i === 0 ? 2 : 1); n++) items.push({ type: "skill", kind: s.id, level: 1, why: "Neu" });
    });
    const weakest = skills
      .filter((s) => s.attempts > 0)
      .sort((a, b) => (a.mean_score ?? 0) - (b.mean_score ?? 0) || a.level - b.level);
    for (let i = 0; items.length < LENGTH && weakest.length; i++) {
      const s = weakest[i % weakest.length];
      items.push({ type: "skill", kind: s.id, level: nextLevel(s), why: "Festigen" });
    }
    for (let i = 0; items.length < LENGTH; i++) items.push({ type: "skill", kind: TRAINABLE[i % TRAINABLE.length], level: 1, why: "Neu" });
    return interleave(items.slice(0, LENGTH));
  }

  /** A round on one kind of spot: its real decisions first, then variations. */
  function focusPlan(key: string): Item[] {
    return Array.from({ length: FOCUS_LENGTH }, (_, i) => ({ type: "drill", key, real: i < 2, why: "Spiel" }) as DrillItem);
  }

  function interleave(items: Item[]): Item[] {
    const rest = [...items];
    const out: Item[] = [];
    while (rest.length) {
      const last = out.at(-1);
      const i = rest.findIndex((x) => !last || idOf(x) !== idOf(last));
      out.push(...rest.splice(i < 0 ? 0 : i, 1));
    }
    return out;
  }

  /** Kinds of spot of the own game in the list, in its order. */
  const own = (steps: NextStep[]) => steps.flatMap((s) => (s.practice && "spot" in s.practice ? [s.practice.spot] : []));
  /** Rated decisions of the history at heads-up and three- to five-handed tables. */
  const shortTables = $derived((profile?.groups ?? []).filter((g) => g.table === "heads_up" || g.table === "few").reduce((t, g) => t + g.decisions, 0));
  const plan = $derived(buildPlan(progress, steps, shortTables));
  const groupOf = (key: string) => profile?.groups.find((g) => g.key === key) ?? null;
  const skillGroups = $derived(
    [...new Map(plan.filter((x): x is SkillItem => x.type === "skill").map((x) => [x.kind, x])).values()].map((x) => ({
      ...x,
      n: plan.filter((y) => y.type === "skill" && y.kind === x.kind).length,
    })),
  );
  const drillGroups = $derived(
    [...new Set(plan.filter((x): x is DrillItem => x.type === "drill").map((x) => x.key))].map((key) => ({
      key,
      group: groupOf(key),
      real: plan.filter((y) => y.type === "drill" && y.key === key && y.real).length,
      made: plan.filter((y) => y.type === "drill" && y.key === key && !y.real).length,
    })),
  );

  let phase = $state<"start" | "run" | "done">("start");
  let items = $state<Item[]>([]);
  let index = $state(0);
  let current = $state<Current | null>(null);
  let error = $state<string | null>(null);
  let scores = $state<(number | null)[]>([]);
  let correct = $state<boolean[]>([]);
  let before = $state<Map<LearnKind, SkillProgress>>(new Map());

  /** Real decisions per kind of spot, fetched once per round; each is asked once. */
  let reals = new Map<string, Promise<DrillSpot[]>>();
  let realsUsed = new Map<string, number>();
  const seed = () => Math.floor(Math.random() * 2 ** 52);

  async function fetchDrill(x: DrillItem): Promise<DrillSpot> {
    if (x.real) {
      if (!reals.has(x.key)) reals.set(x.key, api.learnDrillReal(x.key, FOCUS_LENGTH));
      const list = await reals.get(x.key)!;
      const n = realsUsed.get(x.key) ?? 0;
      realsUsed.set(x.key, n + 1);
      // Without another real decision the item becomes a variation.
      if (n < list.length) return list[n];
    }
    return api.learnDrillVariant(x.key, seed());
  }

  function fetchItem(x: Item): Promise<Current> {
    return x.type === "skill"
      ? api.learnExercise(x.kind, x.level).then((exercise) => ({ type: "skill", exercise }) as Current)
      : fetchDrill(x).then((spot) => ({ type: "drill", spot }) as Current);
  }

  /** Tasks fetched ahead by index: table push/fold may solve for seconds, a variation is analysed first. */
  let ahead = new Map<number, Promise<Current>>();
  function fetchAt(i: number): Promise<Current> {
    const p = ahead.get(i) ?? fetchItem(items[i]);
    ahead.delete(i);
    return p;
  }

  async function load(i: number) {
    current = null;
    error = null;
    try {
      const c = await fetchAt(i);
      if (i !== index) return;
      current = c;
      if (i + 1 < items.length && !ahead.has(i + 1)) {
        const next = fetchItem(items[i + 1]);
        next.catch(() => {
          // Reported when it is used.
        });
        ahead.set(i + 1, next);
      }
    } catch (e) {
      error = errorText(e);
    }
  }

  function start(list: Item[] = plan) {
    ahead = new Map();
    reals = new Map();
    realsUsed = new Map();
    items = list;
    before = new Map(byKind);
    xpBefore = progress?.xp_week ?? 0;
    scores = items.map(() => null);
    correct = items.map(() => false);
    index = Math.min(items.length - 1, Number(preview("task")) || 0);
    phase = "run";
    load(index);
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
  const practiced = $derived([...new Set(items.slice(0, Math.max(answered, 1)).map(kindOf))]);
  /** Progress reloads after a round; until it has, this round's answers still count toward the week. */
  let xpBefore = $state(0);
  const xpWeek = $derived(Math.max(progress?.xp_week ?? 0, phase === "done" ? xpBefore + answered : 0));
  /** This round's answers grouped by skill, in the order they came. */
  const trail = $derived(
    [...new Set(items.slice(0, answered).map(kindOf))].map((kind) => ({
      kind,
      at: items.slice(0, answered).flatMap((x, i) => (kindOf(x) === kind ? [i] : [])),
    })),
  );

  /** "Üben" on a kind of spot starts a round on it. */
  function practice(key: string) {
    start(focusPlan(key));
  }
  $effect(() => {
    if (!focus) return;
    const key = focus;
    untrack(() => {
      practice(key);
      onFocusDone();
    });
  });

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
    { why: "Tische", label: "Für deine Tische mit 2 bis 5 Spielern" },
    { why: "Schwachstelle", label: "Aus deinen Schwachstellen" },
    { why: "Wiederholen", label: "Wiederholen, damit es sitzt" },
    { why: "Festigen", label: "Festigen" },
    { why: "Neu", label: "Neu für dich" },
  ];
  const lanes = $derived(LANES.map((l) => ({ ...l, groups: skillGroups.filter((g) => g.why === l.why) })).filter((l) => l.groups.length));
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
          Zusammengestellt aus {[drillGroups.length ? "deinen eigenen Händen" : "", lanes.some((l) => l.why === "Schwachstelle") ? "deinen Schwachstellen" : "", "deinem Stand"]
            .filter(Boolean)
            .join(", ")
            .replace(/, ([^,]*)$/, " und $1")}. Nach jeder Antwort siehst du sofort, warum.
        </p>
        <div class="cta">
          <button class="btn primary go" onclick={() => start()} disabled={!progress}>
            Runde starten
            <svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true"><path d="M5 12h14M13 6l6 6-6 6" /></svg>
          </button>
          <span class="dim small">Fehler kosten nichts. Was danebengeht, kommt bald zur Übung wieder.</span>
        </div>
        <div class="plan" aria-label="Plan für heute">
          {#if drillGroups.length}
            <div class="lane own" data-lane="spiel">
              <span class="lane-label weak">Zuerst: aus deinem Spiel</span>
              <ul>
                {#each drillGroups as d (d.key)}
                  <li>
                    <span class="own-mark" aria-hidden="true"><svg viewBox="0 0 24 24" width="16" height="16"><path d="M4 19V5M4 19h16M8 15l3-4 3 2 5-6" /></svg></span>
                    <span class="pname">
                      <b>{d.group?.title ?? "Deine Spots"}</b>
                      <span class="sub" title={[d.real ? `${d.real} echte ${d.real === 1 ? "Hand" : "Hände"}` : "", d.made ? `${d.made} neu gegeben` : ""].filter(Boolean).join(" + ")}
                        >{d.group ? `kostet dich etwa ${bbText(d.group.mean_loss_bb)} je Entscheidung` : `${d.real + d.made} Aufgaben`}</span
                      >
                    </span>
                  </li>
                {/each}
              </ul>
            </div>
          {/if}
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
                        >{LEVEL_TEXT[byKind.get(g.kind)?.level ?? 0]} · {g.n === 1 ? "1 Aufgabe" : `${g.n} Aufgaben`}{g.why === "Schwachstelle" && whyText(g.kind) ? ` · ${whyText(g.kind)} im Coach` : ""}{g.why === "Tische"
                          ? ` · ${shortTables} Entscheidungen an solchen Tischen in deinen Händen`
                          : ""}</span
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

    <div class="weak-spots">
      <Weaknesses {steps} limit={4} empty {onLearn} {onHands} />
    </div>

    <div class="doors">
      <button class="door" onclick={onSpots}>
        <span class="door-ico" aria-hidden="true"><svg viewBox="0 0 24 24" width="20" height="20"><path d="M12 3.5l2.6 5.3 5.9.9-4.3 4.1 1 5.8-5.2-2.7-5.2 2.7 1-5.8-4.3-4.1 5.9-.9z" /></svg></span>
        <span class="door-txt">
          <b>Wichtige Spots</b>
          <span>Die Entscheidungen, die in kleinen Turnieren am meisten kosten.</span>
        </span>
      </button>
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
        <span class="door-ico" aria-hidden="true"><svg viewBox="0 0 24 24" width="20" height="20"><path d="M12 5.5c4.4 0 8 2.9 8 6.5s-3.6 6.5-8 6.5-8-2.9-8-6.5 3.6-6.5 8-6.5zM9 10h2.6v4H9zM12.4 10H15v4h-2.6z" /></svg></span>
        <span class="door-txt">
          <b>Ganze Hände spielen</b>
          <span>Zufällige Hände am Tisch, jede Entscheidung sofort bewertet.</span>
        </span>
      </button>
    </div>
  </div>
{:else if phase === "run"}
  <div class="run">
    <div class="bar">
      <span class="count num">{index + 1} / {items.length}</span>
      <ol class="steps" aria-label="Fortschritt der Runde">
        {#each items as it, i (i)}
          <li
            class={tone(scores[i])}
            class:now={i === index}
            title={it.type === "skill" ? `${SKILLS[it.kind].short} · ${it.why}` : `${groupOf(it.key)?.title ?? "Dein Spot"} · ${it.real ? "echte Hand" : "neu gegeben"}`}
          ></li>
        {/each}
      </ol>
      <button class="btn ghost" onclick={finish}>Runde beenden</button>
    </div>
    <div class="panel body">
      {#if error}
        <EmptyState tone="error" title="Die Aufgabe konnte nicht geladen werden" text={error}>
          {#snippet action()}<Button variant="primary" onclick={() => load(index)}>Erneut laden</Button>{/snippet}
        </EmptyState>
      {:else if current?.type === "skill"}
        {@const exercise = current.exercise}
        {#key `${index}:${exercise.seed}`}
          <Task
            {exercise}
            check={(a) => api.learnCheck(exercise, a)}
            onGraded={(fb) => {
              scores[index] = fb.score;
              correct[index] = fb.correct;
            }}
            onNext={next}
            nextLabel={index + 1 < items.length ? "Weiter" : "Zum Ergebnis"}
          />
        {/key}
      {:else if current?.type === "drill"}
        {@const spot = current.spot}
        {#key `${index}:${spot.hand_id}:${spot.seed}`}
          <DecisionDrill
            {spot}
            check={(choice) => api.learnDrillCheck(spot, choice)}
            onGraded={(r) => {
              scores[index] = r.score;
              correct[index] = r.correct;
            }}
            onNext={next}
            nextLabel={index + 1 < items.length ? "Weiter" : "Zum Ergebnis"}
            why={groupOf(spot.key) ? costLine(groupOf(spot.key)!) : null}
          />
        {/key}
      {:else}
        <EmptyState tone="loading" title={items[index]?.type === "drill" ? "Spot aus deinem Spiel wird vorbereitet …" : "Aufgabe wird gemischt …"} />
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
  .weak-spots {
    min-width: 0;
  }
  /* Wide: the doors take a column of their own; the plan and the weak spots keep the width. */
  @container (min-width: 1800px) {
    .start {
      grid-template-columns: minmax(0, 1fr) 340px;
      grid-template-areas: "hero doors" "weak-spots doors";
      align-items: start;
    }
    .hero {
      grid-area: hero;
    }
    .weak-spots {
      grid-area: weak-spots;
    }
    .doors {
      grid-area: doors;
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
    font-size: var(--fs-sm);
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
    font-size: var(--fs-xs);
    font-weight: 600;
    color: var(--text-2);
  }
  .own li {
    border-color: color-mix(in srgb, var(--neg) 35%, var(--line));
  }
  .own-mark {
    display: grid;
    place-items: center;
    width: 34px;
    height: 34px;
    border-radius: 50%;
    background: color-mix(in srgb, var(--neg) 14%, var(--surface-3));
  }
  .pname .sub {
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .lane-link {
    margin-left: 8px;
    padding: 0;
    border: 0;
    background: none;
    color: var(--text-2);
    font-size: var(--fs-xs);
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
    font-size: var(--fs-md);
    font-weight: 600;
  }
  .pname .dim {
    font-size: var(--fs-xs);
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
    font-size: var(--fs-base);
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
    font-size: var(--fs-md);
    font-weight: 600;
  }
  .goal {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .big {
    font-size: var(--fs-4xl);
    font-weight: 600;
  }
  .huge {
    font-size: var(--fs-hero);
    font-weight: 600;
  }
  .small {
    font-size: var(--fs-sm);
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
    font-size: var(--fs-base);
  }
  .door-txt span {
    color: var(--text-2);
    font-size: var(--fs-sm);
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
    font-size: var(--fs-lg);
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
    transition: background var(--dur-slow) var(--ease-standard);
  }
  .steps li.now {
    background: var(--text-2);
    animation: breathe calc(2 * var(--dur-loop)) var(--ease-spin) infinite;
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
    font-size: var(--fs-xs);
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
    font-size: var(--fs-xs);
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
    font-size: var(--fs-3xl);
    font-weight: 600;
  }
  .of {
    font-size: var(--fs-lg);
    color: var(--text-3);
    font-weight: 500;
  }
  .stats div > span {
    color: var(--text-2);
    font-size: var(--fs-sm);
  }
  .grown {
    display: grid;
    gap: 10px;
    padding-top: 20px;
    border-top: 1px solid var(--line);
  }
  .grown h2 {
    margin: 0;
    font-size: var(--fs-base);
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
    font-size: var(--fs-sm);
  }
  .cname b {
    font-size: var(--fs-md);
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
    font-size: var(--fs-md);
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
