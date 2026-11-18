<script lang="ts">
  import { onMount } from "svelte";
  import { api, on } from "../api";
  import { libraryApi } from "./api";
  import { COST, MODES } from "./BackgroundMode.svelte";
  import SegmentedControl from "../ui/SegmentedControl.svelte";
  import SettingRow from "../ui/SettingRow.svelte";
  import type { LibraryMode } from "./types";

  let mode = $state<LibraryMode | null>(null);

  // The choice lives in the settings, so it stays readable and changeable while PokerStars locks
  // the library itself; it only decides what happens once PokerStars is closed.
  onMount(() => {
    api
      .getSettings()
      .then((s) => s.library?.background ?? libraryApi.overview().then((o) => o.mode))
      .then((m) => (mode = m))
      .catch(() => (mode = null));
    const sub = on<{ library?: { background: LibraryMode } }>("settings-changed", (s) => {
      if (s.library) mode = s.library.background;
    });
    return () => {
      sub.then((f) => f());
    };
  });

  function change(m: LibraryMode) {
    mode = m;
    libraryApi.setMode(m);
  }

  const current = $derived(MODES.find((m) => m.value === mode) ?? null);
</script>

<SettingRow
  title="Bibliothek automatisch erweitern"
  sub={current?.text ?? "Dein PC rechnet Spots für die Bibliothek vor, damit du sie später sofort nachschlagen kannst."}
>
  {#snippet control()}
    <SegmentedControl label="Bibliothek automatisch erweitern" value={mode ?? ("" as LibraryMode)} options={MODES} disabled={!mode} onchange={change} />
  {/snippet}
  {#snippet more()}<p>{COST}</p>{/snippet}
</SettingRow>
