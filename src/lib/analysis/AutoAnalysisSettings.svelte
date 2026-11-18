<script lang="ts">
  import { onMount } from "svelte";
  import { api, on } from "../api";
  import SettingRow from "../ui/SettingRow.svelte";
  import Switch from "../ui/Switch.svelte";

  /** `onChange` hears every known state, for summaries elsewhere on the page. */
  let { onChange }: { onChange?: (auto: boolean) => void } = $props();

  let auto = $state<boolean | null>(null);
  $effect(() => {
    if (auto !== null) onChange?.(auto);
  });

  onMount(() => {
    api.getSettings().then((s) => (auto = s.analysis.auto));
    const sub = on<{ analysis?: { auto: boolean } }>("settings-changed", (s) => {
      if (s.analysis) auto = s.analysis.auto;
    });
    return () => {
      sub.then((f) => f());
    };
  });

  function change(next: boolean) {
    auto = next;
    api.analysisSetAuto(next);
  }
</script>

<SettingRow
  title="Neue Hände automatisch analysieren"
  sub="Sobald du PokerStars schließt, bewertet Ringside deine neuen Hände. Solange PokerStars läuft, rechnet Ringside nie."
>
  {#snippet control()}
    {#if auto !== null}<Switch label="Neue Hände automatisch analysieren" checked={auto} onchange={change} />{/if}
  {/snippet}
</SettingRow>
