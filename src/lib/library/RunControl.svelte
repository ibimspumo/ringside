<script lang="ts">
  import type { Snippet } from "svelte";
  import Button from "../ui/Button.svelte";
  import CardText from "../ui/CardText.svelte";
  import MiniCard from "../ui/MiniCard.svelte";
  import ProgressBar from "../ui/ProgressBar.svelte";
  import { libraryApi } from "./api";
  import { cards, titleOf } from "./labels";
  import type { LibraryOverview, WorkerStatus } from "./types";

  /** What the worker does, in words, and the one button that starts or stops it. */
  let {
    data,
    worker,
    primary = false,
    onChange,
    after,
  }: { data: LibraryOverview; worker: WorkerStatus; primary?: boolean; onChange: () => void; after?: Snippet } = $props();

  let busy = $state(false);
  let error = $state<string | null>(null);

  const running = $derived(worker.manual || worker.state === "solving" || worker.state === "cloud");
  const done = $derived(worker.state === "done");
  /** The library is complete; with sharing on, a run solves what other users need. */
  const forOthers = $derived(done && data.share);
  const scenario = $derived(data.scenarios.find((x) => x.id === worker.scenario) ?? null);
  const pct = $derived(worker.state === "solving" && worker.progress !== null ? Math.round(worker.progress * 100) : null);
  const tone = $derived(running ? "live" : worker.reason === "client" ? "warn" : done ? "done" : "idle");

  /** Headline and the sentence under it. */
  const words = $derived.by((): [string, string] => {
    switch (worker.state) {
      case "solving":
        return worker.community
          ? ["Rechnet für andere", "Deine Bibliothek ist komplett. Jetzt löst dein PC Spots, die andere noch brauchen."]
          : [
              "Rechnet gerade",
              worker.manual ? "Läuft, bis du anhältst, PokerStars startet oder alles fertig ist." : "Die Automatik rechnet, solange du die App nicht benutzt.",
            ];
      case "cloud":
        return ["Schaut in der Cloud nach", "Hat schon jemand diesen Spot gerechnet? Dann lädt Ringside ihn, statt selbst zu rechnen."];
      case "packs":
        return ["Lädt fertige Spots herunter", "Das kostet deinen PC keine Rechenzeit."];
      case "done":
        return forOthers
          ? [
              "Bibliothek komplett",
              `Willst du helfen, rechnet dein PC Spots, die andere noch brauchen${worker.community_done ? ` – ${worker.community_done} hat er seit dem Start schon beigetragen` : ""}. Danke!`,
            ]
          : ["Bibliothek komplett", "Alle Spots sind gelöst. Danke fürs Mitrechnen!"];
      case "off":
        return ["Automatik aus", "Dein PC rechnet nur, wenn du hier startest."];
    }
    switch (worker.reason) {
      case "client":
        return ["Pause: PokerStars läuft", "Solange PokerStars offen ist, rechnet Ringside nie."];
      case "halted":
        return ["Angehalten", "Die Automatik ruht, bis du wieder startest."];
      case "idle":
        return ["Wartet auf eine Pause", "Die Automatik legt los, wenn fünf Minuten niemand Maus oder Tastatur benutzt."];
      case "idle_unknown":
        return ["Wartet", "Ob der PC gerade frei ist, erkennt Ringside nur unter Windows. Starte hier selbst."];
      case "in_use":
        return ["Wartet, bis du Ringside nicht benutzt", "So bleibt die App flott. Mit dem Knopf geht es sofort los."];
    }
    return ["Startet …", ""];
  });

  async function toggle() {
    busy = true;
    try {
      await libraryApi.run(!running);
      error = null;
      onChange();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<div class="run">
  <div class="say" aria-live="polite">
    <p class="head"><span class="dot {tone}" aria-hidden="true"></span>{words[0]}</p>
    {#if worker.state === "solving" && worker.community}
      {@const [where, flop] = worker.community.split(/,\s*Flop\s+/)}
      <p class="spot"><span>{where}</span>{#if flop}<span class="flop"><CardText text={flop} /></span>{/if}</p>
    {:else if scenario && worker.flop && (worker.state === "solving" || worker.state === "cloud")}
      <p class="spot">
        <span>{titleOf(scenario)}</span>
        <span class="flop">{#each cards(worker.flop) as [r, s], k (k)}<MiniCard card={r + s} />{/each}</span>
      </p>
    {/if}
    {#if worker.state === "solving"}
      <div class="track">
        <ProgressBar value={worker.progress} tone="good" label="Fortschritt des aktuellen Spots" />
        {#if pct !== null}<span class="pct num">{pct} %</span>{/if}
      </div>
    {/if}
    {#if words[1]}<p class="sub">{words[1]}</p>{/if}
    {#if error}<p class="err" role="alert">{error}</p>{/if}
  </div>

  <div class="acts">
  <Button
    class="go"
    variant={primary && !running ? "primary" : "secondary"}
    loading={busy}
    disabled={done && !running && !forOthers}
    aria-pressed={running}
    onclick={toggle}
  >
    {#snippet icon()}
      <svg viewBox="0 0 24 24" width="14" height="14">
        {#if running}
          <rect x="6" y="5" width="4" height="14" rx="1" fill="currentColor" />
          <rect x="14" y="5" width="4" height="14" rx="1" fill="currentColor" />
        {:else}
          <path d="M8 5.2v13.6a.8.8 0 0 0 1.2.7l10.6-6.8a.8.8 0 0 0 0-1.4L9.2 4.5A.8.8 0 0 0 8 5.2z" fill="currentColor" />
        {/if}
      </svg>
    {/snippet}
    {running ? "Anhalten" : forOthers ? "Für andere mitrechnen" : done ? "Alles fertig" : "Jetzt mitrechnen"}
  </Button>
  {@render after?.()}
  </div>
</div>

<style>
  /* Text and button side by side when there is room, the button under the text when not. */
  .run {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-3) var(--sp-6);
    min-width: 0;
  }
  .acts {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2);
  }
  .say {
    flex: 1 1 22em;
    display: flex;
    flex-direction: column;
    gap: var(--sp-1);
    min-width: 0;
  }
  p {
    margin: 0;
  }
  .head {
    display: flex;
    align-items: baseline;
    gap: var(--sp-2);
    color: var(--text);
    font-size: var(--fs-base);
    font-weight: var(--fw-semibold);
    line-height: var(--lh-tight);
  }
  .dot {
    flex: none;
    transform: translateY(-1px);
    width: 8px;
    height: 8px;
    border-radius: var(--r-pill);
    background: var(--text-3);
  }
  .dot.live {
    background: var(--pos);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--pos) 25%, transparent);
  }
  .dot.warn {
    background: var(--amber);
  }
  .dot.done {
    background: var(--info);
  }
  .spot {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-1) var(--sp-2);
    color: var(--text);
    font-size: var(--fs-md);
  }
  .flop {
    display: inline-flex;
    gap: 2px;
    white-space: nowrap;
  }
  .track {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    gap: var(--sp-2);
    margin: var(--sp-1) 0;
  }
  .pct {
    color: var(--pos);
    font-size: var(--fs-sm);
    font-weight: var(--fw-semibold);
  }
  .sub {
    color: var(--text-2);
    font-size: var(--fs-md);
    line-height: var(--lh-base);
  }
  .err {
    color: var(--neg);
    font-size: var(--fs-md);
  }
</style>
