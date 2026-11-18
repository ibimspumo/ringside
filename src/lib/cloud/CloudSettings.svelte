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
    <p>
      Push/Fold-Lösungen werden vor der Nutzung hier mit der eigenen Engine nachgeprüft. Spots der Bibliothek misst vor der
      Freigabe ein Prüfer mit dem Solver; hier wird nur ihre Form geprüft, und Grundstock-Pakete müssen die Signatur von
      Ringside tragen.
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
    <p>
      Selbst gerechnete Lösungen gehen im Hintergrund an die Cloud: die Rechen-Eingabe (Stacks, Blinds, Antes, Auszahlungen;
      bei Bibliothek-Spots Board, Ranges und Größen) und das Ergebnis.
    </p>
    <p>
      Flop-Spots aus der Handanalyse und aus „Hände spielen“ werden nur geteilt, wenn ihre Ranges aus dem Preflop-Modell
      stammen: dann gehen Flop, Pot, Stack, Größen und die Modell-Linie, aus der die Ranges folgen, hoch – nie deine Karten,
      die Hand oder wann du sie gespielt hast.
    </p>
    <p>
      Ein Prüfer misst jede Lösung, bevor andere sie bekommen. Mit „Mitrechnen“ hilft dein PC außerdem bei Spots, die andere
      noch brauchen.
    </p>
  {/snippet}
</SettingRow>

<SettingRow title="Was nie geteilt wird" sub="Hände, Spielernamen, Statistiken und Notizen bleiben auf deinem PC.">
  {#snippet more()}
    <p>
      Die Cloud läuft bei Bunny.net; deine IP-Adresse sieht der Anbieter bei jeder Anfrage technisch, gespeichert wird sie nur
      als Hash für Missbrauchsgrenzen.
    </p>
    <p>
      Solange PokerStars läuft, spricht Ringside nicht mit der Cloud. Hände, die Ringside nicht lesen kann, sendest du nur
      einzeln und nach Bestätigung.
    </p>
  {/snippet}
</SettingRow>
