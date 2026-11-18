<script lang="ts">
  import { onMount } from "svelte";
  import { cloudApi, onCloudChanged } from "./api";
  import SettingRow from "../ui/SettingRow.svelte";
  import Switch from "../ui/Switch.svelte";
  import type { CloudSettings } from "./types";

  let cloud = $state<CloudSettings | null>(null);

  onMount(() => {
    cloudApi.settings().then((c) => (cloud = c));
    const sub = onCloudChanged((c) => (cloud = c));
    return () => {
      sub.then((f) => f());
    };
  });

  function set(key: "fetch" | "share", on: boolean) {
    if (!cloud) return;
    cloud[key] = on;
    cloudApi.saveSettings($state.snapshot(cloud));
  }
</script>

<SettingRow title="Fertige Lösungen abrufen" sub="Statt selbst zu rechnen, lädt Ringside Lösungen, die schon jemand gerechnet hat.">
  {#snippet control()}
    {#if cloud}<Switch label="Fertige Lösungen abrufen" checked={cloud.fetch} onchange={(on) => set("fetch", on)} />{/if}
  {/snippet}
  {#snippet more()}
    <h4>Wie Ringside fremden Lösungen traut</h4>
    <p>
      Push/Fold-Lösungen rechnet Ringside vor der Nutzung mit der eigenen Engine nach. Spots der Bibliothek misst vor der
      Freigabe ein Prüfer mit dem Solver; auf deinem PC prüft Ringside dann nur noch, ob die Lösung richtig aufgebaut ist. Grundstock-Pakete
      müssen die Signatur von Ringside tragen.
    </p>
  {/snippet}
</SettingRow>

<SettingRow
  title="Eigene Lösungen teilen"
  sub="Was dein PC selbst gerechnet hat, hilft anderen – ohne deine Karten, Hände oder Namen."
>
  {#snippet control()}
    {#if cloud}<Switch label="Eigene Lösungen teilen" checked={cloud.share} onchange={(on) => set("share", on)} />{/if}
  {/snippet}
  {#snippet more()}
    <h4>Was übertragen wird</h4>
    <p>
      Die Rechen-Eingabe und das Ergebnis: bei Push/Fold Stacks, Blinds, Antes und Auszahlungen, bei Bibliothek-Spots Board,
      Ranges und Einsatzgrößen. Das geschieht im Hintergrund.
    </p>
    <p>
      Flop-Spots aus der Handanalyse und aus „Hände spielen“ teilt Ringside nur, wenn ihre Ranges aus dem Preflop-Modell
      stammen. Dann gehen Flop, Pot, Stack, Einsatzgrößen und die Modell-Linie hoch, aus der die Ranges folgen – nie deine
      Karten, die Hand oder wann du sie gespielt hast.
    </p>
    <h4>Wie geprüft wird</h4>
    <p>
      Ein Prüfer misst jede Lösung, bevor andere sie bekommen. Mit „Mitrechnen“ hilft dein PC außerdem bei Spots, die andere
      noch brauchen.
    </p>
  {/snippet}
</SettingRow>

<SettingRow title="Was nie geteilt wird" sub="Hände, Spielernamen, Statistiken und Notizen bleiben auf deinem PC.">
  {#snippet more()}
    <h4>Wo die Cloud läuft</h4>
    <p>
      Bei Bunny.net. Deine IP-Adresse sieht der Anbieter bei jeder Anfrage technisch; gespeichert wird sie nur als Hash, um
      Missbrauch zu begrenzen.
    </p>
    <p>
      Solange PokerStars läuft, spricht Ringside nicht mit der Cloud. Hände, die Ringside nicht lesen kann, sendest du nur
      einzeln und nach Bestätigung.
    </p>
  {/snippet}
</SettingRow>
