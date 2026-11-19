<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "../api";
  import { coachApi } from "../coach/api";
  import type { ConceptCount } from "../coach/types";
  import type { LearnTarget } from "../app/Leaks.svelte";
  import type { Filter, HudConfig, LearnKind, LearnProgress } from "../types";
  import { errorText } from "./catalog";
  import OwnHands from "./OwnHands.svelte";
  import KeySpots from "./KeySpots.svelte";
  import Path from "./Path.svelte";
  import Today from "./Today.svelte";
  import Trainer from "./Trainer.svelte";
  import Wissen from "./wissen/Wissen.svelte";
  import { preview } from "./preview";
  import { usePageHead } from "../ui/page.svelte";
  import type { PlayProfile } from "./profile";
  import type { NextStep, StepHands } from "../next/types";

  let {
    hud = null,
    filter,
    open = null,
    onOpened = () => {},
    onHands,
  }: {
    hud?: HudConfig | null;
    /** The app's game and period: „Was als Nächstes?“ reads the same list as the overview. */
    filter: Filter;
    /** An article or exercise to show first, e.g. from the leak finder. */
    open?: LearnTarget | null;
    onOpened?: () => void;
    /** „Hände ansehen“ of a step of „Was als Nächstes?“. */
    onHands?: (hands: StepHands) => void;
  } = $props();

  type Section = "today" | "spots" | "path" | "own" | "trainer" | "wissen";
  const SECTIONS: { id: Section; label: string; sub?: string }[] = [
    { id: "today", label: "Heute" },
    {
      id: "spots",
      label: "Wichtige Spots",
      sub: "Die Entscheidungen, die in kleinen Turnieren am meisten kosten – vorgerechnet. Falsch beantwortete kommen in der nächsten Runde zuerst.",
    },
    { id: "path", label: "Lernpfad", sub: "Elf Einheiten, aufeinander aufgebaut. Spring ruhig dorthin, wo du stehst – nichts ist gesperrt." },
    {
      id: "own",
      label: "Deine Hände",
      sub: "Deine All-ins und Push/Fold-Spots. Der Replayer hält vor deiner Entscheidung an, das Ergebnis bleibt verdeckt, bis du geantwortet hast.",
    },
    { id: "trainer", label: "Trainer" },
    { id: "wissen", label: "Wissen", sub: "Kurze Seiten mit Bild statt Textwand. Eine Idee pro Seite, in einer Minute gelesen." },
  ];
  const STORE = "ringside.learn.section";

  function initial(): Section {
    const p = preview("learn");
    if (SECTIONS.some((x) => x.id === p)) return p as Section;
    try {
      const s = localStorage.getItem(STORE);
      if (SECTIONS.some((x) => x.id === s)) return s as Section;
    } catch {
      // Storage may be unavailable.
    }
    return "today";
  }

  let section = $state<Section>(initial());
  let progress = $state<LearnProgress | null>(null);
  /** Coach concepts from stored reports; empty without reports. */
  let concepts = $state<ConceptCount[]>([]);
  /** Kinds of spot from the analysed hands; null while loading or locked. */
  let profile = $state<PlayProfile | null>(null);
  /** „Was als Nächstes?“ over all hands; empty while it loads or while locked. */
  let steps = $state<NextStep[]>([]);
  /** A kind of spot "Heute" practises right away. */
  let focus = $state<string | null>(preview("focus"));
  let loadError = $state<string | null>(null);
  let article = $state<string | null>(preview("article"));
  /** Skill and level the trainer opens with when coming from the path. */
  let trainerPick = $state<{ kind: LearnKind; level: number } | null>(
    preview("skill") ? { kind: preview("skill") as LearnKind, level: Number(preview("level") ?? 1) } : null,
  );
  let top = $state<HTMLElement>();

  $effect(() => {
    const s = section;
    try {
      localStorage.setItem(STORE, s);
    } catch {
    }
  });

  async function reload() {
    try {
      progress = await api.learnProgress();
      loadError = null;
    } catch (e) {
      loadError = errorText(e);
    }
    api
      .learnProfile({ game: null, since: null, tournament_id: null })
      .then((p) => (profile = p))
      .catch(() => {
        // Locked or not analysed yet: the plan then works without the own game.
      });
    api
      .nextSteps({ ...filter, tournament_id: null })
      .then((s) => (steps = s))
      .catch(() => {
        // Locked: no advice while PokerStars runs.
      });
  }

  onMount(() => {
    reload();
    coachApi
      .concepts()
      .then((c) => (concepts = c))
      .catch(() => {
        // Without reports, or while locked, the learning area simply shows no weaknesses.
      });
  });

  function go(s: Section) {
    section = s;
    top?.scrollIntoView({ block: "start" });
  }

  function train(kind: LearnKind, level: number) {
    trainerPick = { kind, level };
    go("trainer");
  }

  function practice(key: string) {
    focus = key;
    go("today");
  }

  function read(id: string) {
    article = id;
    go("wissen");
  }

  function learn(target: LearnTarget) {
    if ("article" in target) read(target.article);
    else if ("spot" in target) practice(target.spot);
    else train(target.kind, target.level);
  }

  $effect(() => {
    if (!open) return;
    learn(open);
    onOpened();
  });

  usePageHead(() => ({
    sub: SECTIONS.find((x) => x.id === section)?.sub,
    label: "Lernbereich",
    value: section,
    tabs: SECTIONS.map(({ id, label }) => ({ id, label })),
    onchange: (id) => go(id as Section),
  }));
</script>

<div class="learn page" bind:this={top}>

  {#if loadError}
    <p class="error" role="alert">{loadError}</p>
  {/if}

  <div role="tabpanel" aria-label={SECTIONS.find((s) => s.id === section)?.label}>
    {#if section === "today"}
      <Today
        {progress}
        {concepts}
        {profile}
        {steps}
        {focus}
        onFocusDone={() => (focus = null)}
        onProgress={reload}
        onPath={() => go("path")}
        onOwn={() => go("own")}
        onTrainer={() => go("trainer")}
        onSpots={() => go("spots")}
        onLearn={learn}
        {onHands}
      />
    {:else if section === "spots"}
      <KeySpots onProgress={reload} />
    {:else if section === "path"}
      <Path {progress} {steps} onTrain={train} onRead={read} onLearn={learn} {onHands} />
    {:else if section === "own"}
      <OwnHands onProgress={reload} onRead={read} />
    {:else if section === "trainer"}
      <Trainer {progress} pick={trainerPick} onProgress={reload} onRead={read} />
    {:else}
      <Wissen {hud} openId={article} onOpenChange={(id) => (article = id)} />
    {/if}
  </div>
</div>

<style>
  .learn {
    display: grid;
    gap: 22px;
    scroll-margin-top: 0;
  }
  .error {
    margin: 0;
    padding: 10px 14px;
    border-radius: var(--r-sm);
    background: rgba(229, 72, 59, 0.12);
    color: #ff9d93;
  }
  @container (max-width: 560px) {
    .learn {
      padding: 18px 16px 40px;
    }
  }
</style>
