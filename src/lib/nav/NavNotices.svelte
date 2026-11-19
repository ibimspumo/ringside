<script lang="ts" module>
  import type { ImportStatus } from "../types";

  /** No hand-history folder: the one warning above every page. */
  export const historyMissing = (status: ImportStatus | null, settingsOpen: boolean) =>
    !!status && !status.upgrade && status.dirs.length === 0 && !settingsOpen;
</script>

<script lang="ts">
  import Button from "../ui/Button.svelte";
  import { rectOf, type Anchor } from "../ui/floating";
  import { int } from "../ui/format";
  import Popover from "../ui/Popover.svelte";
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
  const HOW = "In PokerStars unter Einstellungen → Spielhistorie „Handverläufe speichern“ einschalten. Ringside findet den Ordner dann von selbst.";

  let guide = $state<Anchor | null>(null);
</script>

{#if upgrade}
  <div class="strip" role="status">
    <b>Datenbank wird aktualisiert</b>
    <span class="bar"><ProgressBar value={share} label="Datenbank wird aktualisiert" /></span>
    <span class="num">{Math.floor(share * 100)} % · {int(upgrade.done)} von {int(upgrade.total)} Händen</span>
  </div>
{:else if historyMissing(status, settingsOpen)}
  <div class="strip warn">
    <b class="title"
      ><svg viewBox="0 0 16 16" width="15" height="15" aria-hidden="true"><path d="M8 2.5 14.5 13.5h-13zM8 6.5v3.5M8 11.8v.2" /></svg><Term tip={HOW}
        >PokerStars speichert keine neuen Hände</Term
      ></b
    >
    <span class="how long">{HOW}</span>
    <span class="how short" title={HOW}>„Handverläufe speichern“ einschalten: Einstellungen → Spielhistorie.</span>
    <span class="acts">
      <Button size="sm" variant="quiet" onclick={onChooseFolder}>Ordner selbst wählen</Button>
      <Button size="sm" variant="primary" onclick={(e) => (guide = guide ? null : rectOf(e.currentTarget as Element))}>Anleitung anzeigen</Button>
    </span>
  </div>
  {#if guide}
    <Popover anchor={guide} pinned side="bottom" width={340} label="Handverläufe in PokerStars speichern" onclose={() => (guide = null)}>
      <p class="g-title">So speichert PokerStars deine Hände</p>
      <ol class="steps">
        <li><span class="n">1</span><span>In PokerStars oben auf <b>Einstellungen</b>.</span></li>
        <li><span class="n">2</span><span>Links <b>Spielhistorie</b> wählen.</span></li>
        <li>
          <span class="n">3</span>
          <span>
            <b>Handverläufe speichern</b> einschalten.
            <span class="mock" aria-hidden="true"><span>Handverläufe speichern</span><i></i></span>
          </span>
        </li>
      </ol>
      <p class="g-foot">Ringside findet den Ordner dann von selbst, du musst nichts weiter tun.</p>
    </Popover>
  {/if}
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
  /* The how-to in one line: in full where it fits, else in short, and in narrow windows only behind "Anleitung anzeigen". */
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
  @container (max-width: 1099px) {
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
  .acts {
    grid-column: 3;
    display: flex;
    gap: var(--sp-2);
  }
  .g-title {
    margin: 0 var(--sp-6) var(--sp-3) 0;
    color: var(--text);
    font-weight: var(--fw-semibold);
  }
  .steps {
    display: grid;
    gap: var(--sp-3);
    margin: 0;
    padding: 0;
    list-style: none;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .steps li {
    display: grid;
    grid-template-columns: 22px minmax(0, 1fr);
    gap: var(--sp-2);
    align-items: start;
  }
  .n {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border-radius: var(--r-pill);
    background: var(--surface-3);
    color: var(--text);
    font-family: var(--font-num);
    font-size: var(--fs-xs);
  }
  /* A picture of the PokerStars switch, switched on: where to look, not a control. */
  .mock {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-top: var(--sp-2);
    padding: 6px var(--sp-3);
    border: 1px solid var(--line-2);
    border-radius: var(--r-sm);
    background: var(--surface-2);
    color: var(--text);
  }
  .mock i {
    position: relative;
    width: 30px;
    height: 16px;
    border-radius: var(--r-pill);
    background: var(--pos);
  }
  .mock i::after {
    content: "";
    position: absolute;
    top: 2px;
    right: 2px;
    width: 12px;
    height: 12px;
    border-radius: var(--r-pill);
    background: var(--surface);
  }
  .g-foot {
    margin: var(--sp-3) 0 0;
    font-size: var(--fs-sm);
    color: var(--text-3);
  }
</style>
