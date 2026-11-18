<script lang="ts">
  import { onMount } from "svelte";
  import { cloudApi, onCloudChanged } from "./api";
  import type { CloudSettings } from "./types";

  let cloud = $state<CloudSettings | null>(null);

  onMount(() => {
    cloudApi.settings().then((c) => (cloud = c));
    const sub = onCloudChanged((c) => (cloud = c));
    return () => {
      sub.then((f) => f());
    };
  });

  function save() {
    if (cloud) cloudApi.saveSettings($state.snapshot(cloud));
  }
</script>

<section class="panel">
  <div class="panel-head"><h2 class="panel-title">Ringside Cloud</h2></div>
  {#if cloud}
    <div class="body">
      <label class="check">
        <input type="checkbox" bind:checked={cloud.fetch} onchange={save} />
        <span>
          Fertige Lösungen abrufen
          <small>
            Statt selbst zu rechnen, lädt Ringside Lösungen, die schon jemand gerechnet hat. Push/Fold-Lösungen werden vor der
            Nutzung hier mit der eigenen Engine nachgeprüft. Spots der Bibliothek misst vor der Freigabe ein Prüfer mit dem
            Solver; hier wird nur ihre Form geprüft, und Grundstock-Pakete müssen die Signatur von Ringside tragen.
          </small>
        </span>
      </label>
      <label class="check">
        <input type="checkbox" bind:checked={cloud.share} onchange={save} />
        <span>
          Eigene Lösungen teilen
          <small>
            Selbst gerechnete Lösungen gehen im Hintergrund an die Cloud: die Rechen-Eingabe (Stacks, Blinds, Antes,
            Auszahlungen; bei Bibliothek-Spots Board, Ranges und Größen) und das Ergebnis. Flop-Spots aus der Handanalyse und aus
            „Hände spielen“ werden nur geteilt, wenn ihre Ranges aus dem Preflop-Modell stammen: dann gehen Flop, Pot, Stack,
            Größen und die Modell-Linie, aus der die Ranges folgen, hoch – nie deine Karten, die Hand oder wann du sie gespielt
            hast. Ein Prüfer misst jede Lösung, bevor andere sie bekommen. Mit „Mitrechnen“ hilft dein PC außerdem bei Spots,
            die andere noch brauchen.
          </small>
        </span>
      </label>
      <p class="hint">
        Nicht geteilt werden Hände, Spielernamen, Statistiken und Notizen. Die Cloud läuft bei Bunny.net; deine IP-Adresse sieht der
        Anbieter bei jeder Anfrage technisch, gespeichert wird sie nur als Hash für Missbrauchsgrenzen. Solange PokerStars läuft,
        spricht Ringside nicht mit der Cloud. Hände, die Ringside nicht lesen kann, sendest du nur einzeln und nach Bestätigung.
      </p>
    </div>
  {/if}
</section>

<style>
  .body {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 12px 18px 18px;
  }
  .check {
    display: flex;
    gap: 10px;
    align-items: flex-start;
  }
  .check input {
    margin-top: 3px;
    accent-color: var(--sd);
  }
  small {
    display: block;
    margin-top: 2px;
    color: var(--text-3);
    font-size: 12px;
    line-height: 1.45;
  }
  .hint {
    margin: 0;
    color: var(--text-2);
    font-size: 12.5px;
    line-height: 1.5;
  }
</style>
