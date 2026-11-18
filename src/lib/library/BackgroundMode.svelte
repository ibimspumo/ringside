<script lang="ts" module>
  import type { LibraryMode } from "./types";

  /** When the library grows in the background, with what each choice means. */
  export const MODES: { value: LibraryMode; label: string; hint: string; text: string }[] = [
    {
      value: "off",
      label: "Aus",
      hint: "Nur rechnen, wenn du es startest",
      text: "Dein PC rechnet nur, wenn du es auf der Seite Bibliothek startest. Fertige Spots werden trotzdem heruntergeladen.",
    },
    {
      value: "idle",
      label: "Wenn der PC frei ist",
      hint: "Empfohlen",
      text: "Empfohlen. Gerechnet wird erst, wenn fünf Minuten lang niemand Maus oder Tastatur benutzt hat, zum Beispiel nachts. Sobald du zurück bist, hört Ringside sofort auf.",
    },
    {
      value: "always",
      label: "Immer",
      hint: "Immer, wenn PokerStars zu ist",
      text: "Immer, wenn PokerStars zu ist – auch während du anderes am PC machst. Während du Ringside benutzt, wartet die Automatik; sofort rechnen kannst du mit „Jetzt mitrechnen“ auf der Seite Bibliothek.",
    },
  ];
  export const COST =
    "Mitrechnen kostet etwas Strom, und der Lüfter kann lauter werden; wie viel Kraft Ringside nimmt, stellst du auf der Seite Bibliothek ein. Solange PokerStars läuft, wird nie gerechnet.";
</script>

<script lang="ts">
  import SegmentedControl from "../ui/SegmentedControl.svelte";

  let { mode, onChange }: { mode: LibraryMode; onChange: (m: LibraryMode) => void } = $props();
  const current = $derived(MODES.find((m) => m.value === mode) ?? MODES[1]);
</script>

<div class="mode">
  <SegmentedControl label="Automatik" value={mode} options={MODES} onchange={onChange} />
  <p>{current.text}</p>
  <p>{COST}</p>
</div>

<style>
  .mode {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    align-items: flex-start;
  }
  p {
    margin: 0;
    color: var(--text-2);
    font-size: var(--fs-md);
    line-height: 1.5;
    max-width: 68ch;
  }
</style>
