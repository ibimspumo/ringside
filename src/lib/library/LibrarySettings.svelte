<script lang="ts">
  import { onMount } from "svelte";
  import { on } from "../api";
  import { libraryApi } from "./api";
  import { COST, MODES } from "./BackgroundMode.svelte";
  import SegmentedControl from "../ui/SegmentedControl.svelte";
  import SettingRow from "../ui/SettingRow.svelte";
  import type { LibraryMode } from "./types";

  /** `locked`: PokerStars runs, so the library cannot be read and its choice waits. */
  let { locked = false }: { locked?: boolean } = $props();

  let mode = $state<LibraryMode | null>(null);

  const load = () =>
    libraryApi
      .overview()
      .then((o) => (mode = o.mode))
      .catch(() => (mode = null));

  onMount(() => {
    const sub = on<{ library?: { background: LibraryMode } }>("settings-changed", (s) => {
      if (s.library) mode = s.library.background;
    });
    return () => {
      sub.then((f) => f());
    };
  });

  $effect(() => {
    if (!locked) load();
  });

  function change(m: LibraryMode) {
    mode = m;
    libraryApi.setMode(m);
  }

  const shown = $derived(locked ? null : mode);
  const current = $derived(MODES.find((m) => m.value === shown) ?? null);
</script>

<SettingRow
  title="Bibliothek automatisch erweitern"
  sub={current?.text ?? "Dein PC rechnet Spots für die Bibliothek vor, damit du sie später sofort nachschlagen kannst."}
  locked={locked || mode === null}
>
  {#snippet control()}
    <SegmentedControl label="Bibliothek automatisch erweitern" value={shown ?? ("" as LibraryMode)} options={MODES} disabled={!shown} onchange={change} />
  {/snippet}
  {#snippet more()}<p>{COST}</p>{/snippet}
</SettingRow>
