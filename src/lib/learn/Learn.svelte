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
  const SECTIONS: { id: Section; label: string; icon: string }[] = [
    { id: "today", label: "Heute", icon: "M12 4v2M12 18v2M4 12h2M18 12h2M6.3 6.3l1.4 1.4M16.3 16.3l1.4 1.4M6.3 17.7l1.4-1.4M16.3 7.7l1.4-1.4M12 9a3 3 0 1 0 0 6 3 3 0 0 0 0-6z" },
    { id: "path", label: "Lernpfad", icon: "M6 19a2 2 0 1 0 0-4 2 2 0 0 0 0 4zM18 9a2 2 0 1 0 0-4 2 2 0 0 0 0 4zM8 17h6a3 3 0 0 0 0-6h-4a3 3 0 0 1 0-6h6" },
    { id: "own", label: "Deine Hände", icon: "M10 4h8a1 1 0 0 1 1 1v11a1 1 0 0 1-1 1h-8a1 1 0 0 1-1-1V5a1 1 0 0 1 1-1zM6 7.5V19a1 1 0 0 0 1 1h8" },
    { id: "trainer", label: "Trainer", icon: "M4 12h3l2-6 4 12 2-6h5" },
    { id: "wissen", label: "Wissen", icon: "M5 5.5A1.5 1.5 0 0 1 6.5 4H19v14H6.5A1.5 1.5 0 0 0 5 19.5zM5 19.5A1.5 1.5 0 0 0 6.5 21H19" },
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

  function tabKey(e: KeyboardEvent) {
    const i = SECTIONS.findIndex((s) => s.id === section);
    const d = e.key === "ArrowRight" ? 1 : e.key === "ArrowLeft" ? -1 : 0;
    if (!d) return;
    e.preventDefault();
    const next = SECTIONS[(i + d + SECTIONS.length) % SECTIONS.length];
    go(next.id);
    (e.currentTarget as HTMLElement).querySelector<HTMLElement>(`[data-id="${next.id}"]`)?.focus();
  }
</script>

<div class="learn page" bind:this={top}>
  <div class="tabs" role="tablist" aria-label="Lernbereich" tabindex="-1" onkeydown={tabKey}>
    {#each SECTIONS as s (s.id)}
      <button
        role="tab"
        data-id={s.id}
        aria-selected={section === s.id}
        tabindex={section === s.id ? 0 : -1}
        class:on={section === s.id}
        onclick={() => go(s.id)}
      >
        <svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true"><path d={s.icon} /></svg>
        {s.label}
      </button>
    {/each}
  </div>

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
  .tabs {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    padding: 3px;
    border-radius: 10px;
    background: var(--surface);
    border: 1px solid var(--line);
    justify-self: start;
    max-width: 100%;
  }
  .tabs button {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 34px;
    padding: 0 14px;
    border: 0;
    border-radius: 7px;
    background: transparent;
    color: var(--text-2);
    font-size: 13.5px;
    white-space: nowrap;
    cursor: pointer;
    transition:
      background 0.12s,
      color 0.12s;
  }
  .tabs button:hover {
    color: var(--text);
  }
  .tabs button.on {
    background: var(--surface-3);
    color: var(--text);
  }
  .tabs svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 1.7;
    stroke-linecap: round;
    stroke-linejoin: round;
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
    .tabs button {
      padding: 0 10px;
    }
  }
</style>
