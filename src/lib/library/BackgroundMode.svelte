<script lang="ts">
  import type { LibraryMode } from "./types";

  /** Choice of when the library grows in the background, with what it costs. */
  let { mode, onChange }: { mode: LibraryMode; onChange: (m: LibraryMode) => void } = $props();

  const MODES: { id: LibraryMode; label: string; text: string }[] = [
    { id: "off", label: "Aus", text: "Dein PC rechnet nicht mit. Fertige Spots werden trotzdem heruntergeladen." },
    {
      id: "idle",
      label: "Nur wenn der PC frei ist",
      text: "Empfohlen. Gerechnet wird erst, wenn fünf Minuten lang niemand Maus oder Tastatur benutzt hat, zum Beispiel nachts. Sobald du zurück bist, hört Ringside sofort auf.",
    },
    {
      id: "always",
      label: "Immer, wenn PokerStars zu ist",
      text: "Auch während du anderes am PC machst, aber nie, während du Ringside benutzt.",
    },
  ];
  const current = $derived(MODES.find((m) => m.id === mode) ?? MODES[1]);
</script>

<div class="mode">
  <div class="seg" role="radiogroup" aria-label="Bibliothek im Hintergrund erweitern">
    {#each MODES as m (m.id)}
      <button role="radio" aria-checked={mode === m.id} class:on={mode === m.id} onclick={() => onChange(m.id)}>{m.label}</button>
    {/each}
  </div>
  <p>{current.text}</p>
  <p class="cost">
    Mitrechnen kostet etwas Strom, und der Lüfter kann lauter werden. Ringside nimmt sich dafür höchstens die Hälfte der
    Rechenleistung und ein Viertel des Arbeitsspeichers. Solange PokerStars läuft, wird nie gerechnet.
  </p>
</div>

<style>
  .mode {
    display: flex;
    flex-direction: column;
    gap: 8px;
    align-items: flex-start;
  }
  p {
    margin: 0;
    color: var(--text-2);
    font-size: 12.5px;
    line-height: 1.5;
    max-width: 68ch;
  }
  .cost {
    color: var(--text-3);
  }
</style>
