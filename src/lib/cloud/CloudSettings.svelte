<script lang="ts">
  import { onMount } from "svelte";
  import { cloudApi, onCloudChanged } from "./api";
  import SettingRow from "../ui/SettingRow.svelte";
  import Switch from "../ui/Switch.svelte";
  import type { CloudSettings } from "./types";

  /** `onChange` hears every known state, for summaries elsewhere on the page. */
  let { onChange }: { onChange?: (cloud: CloudSettings) => void } = $props();

  let cloud = $state<CloudSettings | null>(null);
  $effect(() => {
    if (cloud) onChange?.($state.snapshot(cloud));
  });

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
      Die Lösungen hat die Community mit Ringside gerechnet. Ringside nimmt nur, was richtig aufgebaut ist. Du musst dafür
      nichts tun.
    </p>
    <details>
      <summary>Technische Details</summary>
      <p>
        Push/Fold-Lösungen rechnet Ringside vor der Nutzung mit der eigenen Engine nach. Bei Spots der Bibliothek prüft
        Ringside auf deinem PC nur, ob die Lösung richtig aufgebaut ist und ob ihre gemeldete Genauigkeit reicht; nachgerechnet
        wird nur eine Stichprobe pro Stunde. Ist eine Stichprobe falsch, zieht Ringside Cloud alle Lösungen dieses
        Internetanschlusses zurück und nimmt von dort keine mehr an. Die Grundausstattung der Bibliothek (Grundstock-Pakete)
        muss die Signatur von Ringside tragen.
      </p>
    </details>
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
    <ul>
      <li>bei Push/Fold: Stacks, Blinds, Antes und Auszahlungen</li>
      <li>bei Spots der Bibliothek: Board, Ranges und Einsatzgrößen</li>
      <li>dazu die fertige Lösung</li>
    </ul>
    <h4>Was nie übertragen wird</h4>
    <p>Deine Karten, die Hand und wann du sie gespielt hast.</p>
    <details>
      <summary>Technische Details</summary>
      <p>
        Flop-Spots aus der Handanalyse und aus „Hände spielen“ teilt Ringside nur, wenn ihre Ranges aus Ringsides
        Preflop-Modell stammen. Dann gehen Flop, Pot, Stack, Einsatzgrößen und die Modell-Linie hoch, also der Weg durchs
        Modell, aus dem die Ranges folgen.
      </p>
      <p>
        Andere bekommen deine Lösung, sobald sie oben ist; Ringside Cloud prüft dafür nur ihren Aufbau und rechnet Stichproben
        nach. Mit „Mitrechnen“ hilft dein PC außerdem bei Spots, die andere noch brauchen. Das Teilen läuft im Hintergrund.
      </p>
    </details>
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
