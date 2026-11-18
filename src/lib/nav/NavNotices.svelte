<script lang="ts">
  import type { ImportStatus } from "../types";
  import Button from "../ui/Button.svelte";
  import { int } from "../ui/format";
  import ProgressBar from "../ui/ProgressBar.svelte";
  import Term from "../ui/Term.svelte";

  /** Database upgrade and the missing hand history: the two notes the whole app needs to see. */
  let {
    status,
    settingsOpen,
    layout = "side",
    onChooseFolder,
  }: {
    status: ImportStatus | null;
    /** The settings page says the same in its folder row. */
    settingsOpen: boolean;
    layout?: "side" | "bar";
    onChooseFolder: () => void;
  } = $props();

  const upgrade = $derived(status?.upgrade ?? null);
  const share = $derived(upgrade?.total ? upgrade.done / upgrade.total : 0);
</script>

{#if upgrade}
  <div class="note {layout}" role="status">
    <div class="row">
      <span>Datenbank wird aktualisiert</span>
      <span class="pct">{Math.floor(share * 100)} %</span>
    </div>
    <ProgressBar value={share} label="Datenbank wird aktualisiert" />
    <small>{int(upgrade.done)} von {int(upgrade.total)} Händen</small>
  </div>
{:else if status && status.dirs.length === 0 && !settingsOpen}
  <div class="note warn {layout}">
    <strong
      ><Term tip="In PokerStars unter Einstellungen → Spielhistorie das Speichern der Hand-Historys einschalten. Ringside findet den Ordner dann von selbst."
        >PokerStars speichert noch keine Hände</Term
      ></strong
    >
    <span class="how">In PokerStars unter Einstellungen → Spielhistorie das Speichern der Hand-Historys einschalten. Ringside findet den Ordner dann von selbst.</span>
    <Button size="sm" onclick={onChooseFolder}>Ordner selbst wählen</Button>
  </div>
{/if}

<style>
  .note {
    display: grid;
    gap: 5px;
    font-size: var(--fs-sm);
    line-height: var(--lh-base);
    color: var(--text-2);
  }
  .row {
    display: flex;
    justify-content: space-between;
    color: var(--text);
  }
  .pct {
    font-family: var(--font-num);
  }
  small {
    color: var(--text-3);
    font-size: var(--fs-xs);
  }
  .warn {
    padding: var(--sp-2) 9px;
    border: 1px solid var(--line-2);
    border-radius: var(--r-sm);
    background: var(--surface-2);
  }
  .warn strong {
    color: var(--text);
    font-weight: var(--fw-semibold);
  }
  .warn :global(.ui-btn) {
    justify-self: start;
  }
  /* In the top bar the note is a strip across the window. */
  .bar {
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: var(--sp-3);
    padding: var(--sp-2) var(--sp-6);
    border-width: 0 0 1px;
    border-radius: 0;
  }
  .bar .how {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  @media (max-height: 760px) {
    .side .how {
      display: none;
    }
  }
</style>
