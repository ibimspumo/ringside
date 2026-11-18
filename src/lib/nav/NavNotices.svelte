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
    <b><Term tip={HOW}>PokerStars speichert noch keine Hände</Term></b>
    <span class="how long">{HOW}</span>
    <span class="how short" title={HOW}>In PokerStars: Einstellungen → Spielhistorie → speichern.</span>
    <Button size="sm" onclick={onChooseFolder}>Ordner selbst wählen</Button>
  </div>
{/if}

<style>
  .strip {
    container-type: inline-size;
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: var(--sp-4);
    min-height: 44px;
    padding: var(--sp-2) var(--frame-right, var(--frame-auto)) var(--sp-2) var(--frame-left, var(--frame-auto));
    border-bottom: 1px solid var(--line);
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
    white-space: nowrap;
  }
  .long {
    display: none;
  }
  @container (min-width: 1150px) {
    .long {
      display: block;
    }
    .short {
      display: none;
    }
  }
  .warn {
    background: var(--surface-2);
  }
  .warn :global(.ui-btn) {
    grid-column: 3;
  }
</style>
