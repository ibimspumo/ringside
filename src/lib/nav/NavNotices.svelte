<script lang="ts">
  import type { ImportStatus } from "../types";
  import Button from "../ui/Button.svelte";
  import { int } from "../ui/format";
  import ProgressBar from "../ui/ProgressBar.svelte";
  import Term from "../ui/Term.svelte";

  /**
   * Database upgrade and the missing hand history: the two notes the whole app needs to see, as a
   * strip under the top bar that shares the page's edges.
   */
  let {
    status,
    settingsOpen,
    onChooseFolder,
  }: {
    status: ImportStatus | null;
    /** The settings page says the same in its folder row. */
    settingsOpen: boolean;
    onChooseFolder: () => void;
  } = $props();

  const upgrade = $derived(status?.upgrade ?? null);
  const share = $derived(upgrade?.total ? upgrade.done / upgrade.total : 0);
  const HOW = "In PokerStars unter Einstellungen → Spielhistorie das Speichern der Hand-Historys einschalten. Ringside findet den Ordner dann von selbst.";
</script>

{#if upgrade}
  <div class="strip" role="status">
    <b>Datenbank wird aktualisiert</b>
    <span class="bar"><ProgressBar value={share} label="Datenbank wird aktualisiert" /></span>
    <span class="num">{Math.floor(share * 100)} % · {int(upgrade.done)} von {int(upgrade.total)} Händen</span>
  </div>
{:else if status && status.dirs.length === 0 && !settingsOpen}
  <div class="strip warn">
    <b class="title"
      ><svg viewBox="0 0 16 16" width="15" height="15" aria-hidden="true"><path d="M8 2.5 14.5 13.5h-13zM8 6.5v3.5M8 11.8v.2" /></svg><Term tip={HOW}
        >PokerStars speichert keine neuen Hände</Term
      ></b
    >
    <span class="how long">{HOW}</span>
    <span class="how short" title={HOW}>In PokerStars: Einstellungen → Spielhistorie → Hände speichern.</span>
    <Button size="sm" onclick={onChooseFolder}>Ordner selbst wählen</Button>
  </div>
{/if}

<style>
  /* A card in the page column, like the cloud note below it; urgency is the amber edge alone. */
  .strip {
    container-type: inline-size;
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: var(--sp-4);
    min-height: 44px;
    margin: var(--sp-3) var(--frame-right, var(--frame-auto)) 0 var(--frame-left, var(--frame-auto));
    padding: 6px 6px 6px var(--sp-4);
    border: 1px solid var(--line-2);
    border-radius: var(--r);
    background: var(--surface);
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  b {
    color: var(--text);
    font-weight: var(--fw-semibold);
    white-space: nowrap;
  }
  .num {
    font-family: var(--font-num);
    white-space: nowrap;
  }
  /* The how-to in one line: in full where it fits, else in short; the tooltip has it in full. */
  .how {
    overflow: hidden;
    white-space: nowrap;
  }
  .long {
    display: none;
  }
  @container (min-width: 1340px) {
    .long {
      display: block;
    }
    .short {
      display: none;
    }
  }
  /* Something to do: amber edge and sign, louder than the notes that only inform. */
  .warn {
    background: var(--surface-2);
    box-shadow: inset 3px 0 0 var(--amber);
  }
  .title {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-2);
  }
  .title svg {
    flex-shrink: 0;
    fill: none;
    stroke: var(--amber);
    stroke-width: 1.6;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .warn :global(.ui-btn) {
    grid-column: 3;
  }
</style>
