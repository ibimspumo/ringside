<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "../api";
  import { coachApi } from "../coach/api";
  import type { ConceptCount } from "../coach/types";
  import type { LearnTarget } from "../app/Leaks.svelte";
  import type { HudConfig, LearnKind, LearnProgress } from "../types";
  import { errorText } from "./catalog";
  import OwnHands from "./OwnHands.svelte";
  import Path from "./Path.svelte";
  import Today from "./Today.svelte";
  import Trainer from "./Trainer.svelte";
  import Wissen from "./wissen/Wissen.svelte";
  import { preview } from "./preview";
  import { usePageHead } from "../ui/page.svelte";

  let {
    hud = null,
    open = null,
    onOpened = () => {},
  }: {
    hud?: HudConfig | null;
    /** An article or exercise to show first, e.g. from the leak finder. */
    open?: LearnTarget | null;
    onOpened?: () => void;
  } = $props();

  type Section = "today" | "path" | "own" | "trainer" | "wissen";
  const SECTIONS: { id: Section; label: string; sub?: string }[] = [
    { id: "today", label: "Heute" },
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
  let loadError = $state<string | null>(null);
  let article = $state<string | null>(preview("article"));
  /** Skill and level the trainer opens with when coming from the path. */
  let trainerPick = $state<{ kind: LearnKind; level: number } | null>(
    preview("skill") ? { kind: preview("skill") as LearnKind, level: 1 } : null,
  );
  let top = $state<HTMLElement>();

  $effect(() => {
    const s = section;
    try {
      localStorage.setItem(STORE, s);
    } catch {
      // ignore
    }
  });

  async function reload() {
    try {
      progress = await api.learnProgress();
      loadError = null;
    } catch (e) {
      loadError = errorText(e);
    }
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

  function read(id: string) {
    article = id;
    go("wissen");
  }

  $effect(() => {
    if (!open) return;
    if ("article" in open) read(open.article);
    else train(open.kind, open.level);
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
        onProgress={reload}
        onPath={() => go("path")}
        onOwn={() => go("own")}
        onTrainer={() => go("trainer")}
      />
    {:else if section === "path"}
      <Path {progress} {concepts} onTrain={train} onRead={read} />
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
  /* Very wide windows: the area may grow beyond the usual page width; reading text keeps its own measure. */
  @container (min-width: 1800px) {
    .learn {
      max-width: 1640px;
    }
  }
  @container (max-width: 560px) {
    .learn {
      padding: 18px 16px 40px;
    }
  }
</style>
