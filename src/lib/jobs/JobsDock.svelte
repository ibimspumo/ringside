<script lang="ts">
  import { onMount } from "svelte";
  import ChipRing from "./ChipRing.svelte";
  import { KIND_LABEL, PAGE_LABEL, elapsedText, etaText, isActive, statusText } from "./kinds";
  import { cancelJob, clearJobs, jobs, startJobs } from "./store.svelte";
  import type { Job, JobTarget } from "./types";

  /** `top`: the dock sits in a top bar, so the panel opens below it at the right. */
  let {
    onView,
    placement = "side",
    compact = false,
  }: { onView: (target: JobTarget) => void; placement?: "side" | "top"; /** The ring and the state in one short line, without the word "Aufgaben". */ compact?: boolean } = $props();

  onMount(startJobs);

  const active = $derived(jobs.list.filter(isActive));
  const finished = $derived(jobs.list.filter((j) => !isActive(j)));
  /** The ring shows the average of the tasks that know how far they are. */
  const share = $derived.by(() => {
    const known = active.filter((j) => j.progress !== null);
    if (!active.length) return 0;
    if (!known.length) return null;
    return known.reduce((s, j) => s + (j.progress ?? 0), 0) / known.length;
  });
  const lead = $derived(active[0] ?? null);
  const summary = $derived.by(() => {
    if (jobs.locked && active.length) return "pausiert";
    if (active.length === 0) return finished.length ? `${finished.length} erledigt` : "nichts zu rechnen";
    const pct = share === null ? "" : ` · ${Math.floor(share * 100)} %`;
    return `${active.length === 1 ? "1 läuft" : `${active.length} laufen`}${pct}`;
  });
  /** Beside the ring in a narrow bar: what runs and how far ("Analyse 96 %"), or the state. */
  const brief = $derived(
    jobs.locked || share === null || !lead ? summary : `${active.length === 1 ? (KIND_LABEL[lead.kind] ?? "Aufgabe") : `${active.length} Aufgaben`} ${Math.floor(share * 100)} %`,
  );

  let panel = $state<HTMLElement | null>(null);
  let button = $state<HTMLElement | null>(null);

  function outside(e: PointerEvent) {
    const t = e.target as Node;
    if (jobs.open && !panel?.contains(t) && !button?.contains(t)) jobs.open = false;
  }
  function key(e: KeyboardEvent) {
    if (e.key === "Escape" && jobs.open) {
      jobs.open = false;
      button?.focus();
    }
  }

  function view(j: Job) {
    jobs.open = false;
    onView(j.target);
  }

  function ago(ms: number | null): string {
    if (!ms) return "";
    const min = Math.round((Date.now() - ms) / 60000);
    return min < 1 ? "gerade eben" : min === 1 ? "vor 1 Min." : min < 60 ? `vor ${min} Min.` : `vor ${Math.round(min / 60)} Std.`;
  }
</script>

<svelte:window onpointerdown={outside} onkeydown={key} />

<button
  class="dock"
  class:busy={active.length > 0}
  class:compact
  aria-label={compact ? `Aufgaben: ${summary}` : undefined}
  title={compact ? `Aufgaben: ${summary}` : undefined}
  bind:this={button}
  aria-expanded={jobs.open}
  aria-controls="jobs-panel"
  onclick={() => (jobs.open = !jobs.open)}
>
  <ChipRing {share} count={active.length} dim={jobs.locked} size={30} />
  {#if !compact}
    <span class="txt">
      <b>Aufgaben</b>
      <small>{summary}</small>
    </span>
  {:else if active.length}
    <span class="txt"><small>{brief}</small></span>
  {/if}
</button>

{#if jobs.open}
  <div class="sheet" class:top={placement === "top"} id="jobs-panel" role="dialog" aria-label="Aufgaben" bind:this={panel}>
    <header>
      <div>
        <h2>Aufgaben</h2>
        <p>Alles, was länger rechnet. Es läuft weiter, egal welche Seite du öffnest.</p>
      </div>
      <button class="close" aria-label="Schließen" onclick={() => (jobs.open = false)}>
        <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true"><path d="M4 4l8 8M12 4l-8 8" /></svg>
      </button>
    </header>

    {#if jobs.locked}
      <p class="locked">
        <b>PokerStars läuft.</b> Solange du spielst, rechnet Ringside nicht: laufende Lösungen stoppen, die Bibliothek wartet
        und rechnet weiter, sobald der Client zu ist.
      </p>
    {/if}

    <div class="list">
      {#if !jobs.list.length}
        <p class="empty">
          Gerade rechnet nichts. Hier erscheint alles, was länger dauert: Solver, Live-Charts, die Bibliothek, Downloads und der
          Coach.
        </p>
      {/if}

      {#each active as j (j.id)}
        {@const eta = etaText(j.eta_secs)}
        <article class="job">
          <div class="top">
            <span class="kind">{KIND_LABEL[j.kind] ?? j.kind}</span>
            <span class="state num" class:paused={j.status === "paused"}>{statusText(j, jobs.locked)}</span>
          </div>
          <h3>{j.title}</h3>
          {#if j.detail}<p class="detail">{j.detail}</p>{/if}
          <div
            class="bar"
            class:unknown={j.progress === null}
            role="progressbar"
            aria-label="Fortschritt"
            aria-valuemin="0"
            aria-valuemax="100"
            aria-valuenow={j.progress === null ? undefined : Math.round(j.progress * 100)}
          >
            <i style="width:{(j.progress ?? 0.3) * 100}%"></i>
          </div>
          <div class="foot">
            <span class="time num">läuft seit {elapsedText(j.elapsed_ms)}{eta ? ` · ${eta}` : ""}</span>
            <span class="acts">
              <button class="btn ghost sm" onclick={() => view(j)}>Ansehen</button>
              {#if j.cancellable}
                <button class="btn sm" disabled={j.cancelling} onclick={() => cancelJob(j.id)}>
                  {j.cancelling ? "Bricht ab …" : "Abbrechen"}
                </button>
              {/if}
            </span>
          </div>
        </article>
      {/each}

      {#if finished.length}
        <div class="done-head">
          <span>Erledigt</span>
          <button class="link" onclick={clearJobs}>Liste leeren</button>
        </div>
        {#each finished as j (j.id)}
          <article class="job done" class:failed={j.status === "failed"}>
            <span class="mark" aria-hidden="true">
              {#if j.status === "done"}
                <svg viewBox="0 0 16 16" width="14" height="14"><path d="M3.5 8.5l3 3 6-7" /></svg>
              {:else}
                <svg viewBox="0 0 16 16" width="14" height="14"><path d="M4.5 4.5l7 7M11.5 4.5l-7 7" /></svg>
              {/if}
            </span>
            <div class="body">
              <h3>{j.title}</h3>
              <p class="detail">
                {statusText(j, jobs.locked)} · {ago(j.finished_at)}{j.error ? ` · ${j.error}` : ""}
              </p>
            </div>
            {#if j.status === "done"}
              <button class="btn ghost sm" onclick={() => view(j)} title="Öffnet {PAGE_LABEL[j.target.page] ?? 'die Seite'}">
                Ansehen
              </button>
            {/if}
          </article>
        {/each}
      {/if}
    </div>
  </div>
{/if}

<style>
  .dock {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    margin: 0 0 4px;
    padding: 6px 8px 6px 4px;
    border: 1px solid transparent;
    border-radius: var(--r-sm);
    background: transparent;
    text-align: left;
    cursor: pointer;
  }
  .dock:hover,
  .dock[aria-expanded="true"] {
    background: var(--surface);
    border-color: var(--line);
  }
  .txt {
    display: grid;
    min-width: 0;
    line-height: 1.25;
  }
  .txt b {
    font-size: 13px;
    font-weight: 600;
    color: var(--text-2);
  }
  .busy .txt b {
    color: var(--text);
  }
  .txt small {
    font-size: 12px;
    color: var(--text-3);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .busy .txt small {
    color: var(--text-2);
  }

  .sheet {
    position: fixed;
    left: 206px;
    bottom: 12px;
    z-index: 60;
    display: flex;
    flex-direction: column;
    width: min(420px, calc(100vw - 230px));
    max-height: min(640px, calc(100vh - 24px));
    border: 1px solid var(--line-2);
    border-radius: var(--r);
    background: var(--surface);
    box-shadow: 0 18px 50px rgba(0, 0, 0, 0.55);
    animation: rise 0.16s ease-out;
  }
  .sheet.top {
    left: auto;
    right: 12px;
    top: 64px;
    bottom: auto;
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .sheet {
      animation: none;
    }
  }
  header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 12px;
    padding: 14px 16px 12px;
    border-bottom: 1px solid var(--line);
  }
  h2 {
    margin: 0;
    font-family: var(--font-num);
    font-size: 17px;
    font-weight: 600;
  }
  header p {
    margin: 2px 0 0;
    font-size: 12.5px;
    color: var(--text-2);
  }
  .close {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border: 0;
    border-radius: var(--r-sm);
    background: transparent;
    color: var(--text-2);
    cursor: pointer;
  }
  .close:hover {
    background: var(--surface-2);
    color: var(--text);
  }
  svg path {
    fill: none;
    stroke: currentColor;
    stroke-width: 1.7;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .locked {
    margin: 12px 16px 0;
    padding: 9px 11px;
    border: 1px solid var(--line-2);
    border-radius: var(--r-sm);
    background: var(--surface-2);
    font-size: 12.5px;
    color: var(--text-2);
  }
  .locked b {
    color: var(--text);
  }
  .list {
    overflow-y: auto;
    padding: 6px 8px 10px;
  }
  .empty {
    margin: 0;
    padding: 22px 12px;
    color: var(--text-2);
    font-size: 13px;
    text-align: center;
  }
  .job {
    padding: 11px 8px 10px;
    border-bottom: 1px solid var(--line);
  }
  .job:last-child {
    border-bottom: 0;
  }
  .top {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
  }
  .kind {
    font-size: 11.5px;
    font-weight: 600;
    letter-spacing: 0.02em;
    color: var(--text-3);
  }
  .state {
    font-size: 13px;
    color: var(--text);
  }
  .state.paused {
    color: var(--text-3);
  }
  h3 {
    margin: 2px 0 0;
    font-size: 13.5px;
    font-weight: 600;
    line-height: 1.35;
  }
  .detail {
    margin: 2px 0 0;
    font-size: 12.5px;
    color: var(--text-2);
  }
  .bar {
    height: 4px;
    margin: 9px 0 7px;
    border-radius: 2px;
    background: var(--surface-3);
    overflow: hidden;
  }
  .bar i {
    display: block;
    height: 100%;
    background: var(--text);
    transition: width 0.4s;
  }
  .bar.unknown i {
    animation: slide 1.4s ease-in-out infinite;
  }
  @keyframes slide {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(340%);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .bar.unknown i {
      animation: none;
    }
  }
  .foot {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 8px;
  }
  .time {
    font-size: 12px;
    color: var(--text-3);
  }
  .acts {
    display: flex;
    gap: 4px;
  }
  .btn.sm {
    height: 26px;
    padding: 0 10px;
    font-size: 12.5px;
  }
  .done-head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    padding: 12px 8px 2px;
    font-size: 11.5px;
    font-weight: 600;
    letter-spacing: 0.02em;
    color: var(--text-3);
  }
  .link {
    border: 0;
    background: none;
    padding: 0;
    color: var(--text-2);
    font-size: 12px;
    font-weight: 400;
    cursor: pointer;
  }
  .link:hover {
    color: var(--text);
    text-decoration: underline;
  }
  .job.done {
    display: grid;
    grid-template-columns: 18px minmax(0, 1fr) auto;
    align-items: center;
    gap: 8px;
    padding: 8px;
  }
  .mark {
    color: var(--text-2);
  }
  .job.failed .mark {
    color: var(--text-3);
  }
  .job.done h3 {
    margin: 0;
    font-weight: 500;
    color: var(--text-2);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .job.done .detail {
    font-size: 12px;
    color: var(--text-3);
  }
</style>
