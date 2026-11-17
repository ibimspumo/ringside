<script lang="ts">
  import { onMount } from "svelte";
  import { on } from "../api";
  import { libraryApi } from "./api";
  import BackgroundMode from "./BackgroundMode.svelte";
  import type { LibraryMode } from "./types";

  let mode = $state<LibraryMode | null>(null);

  onMount(() => {
    libraryApi
      .overview()
      .then((o) => (mode = o.mode))
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
</script>

<section class="panel">
  <div class="panel-head"><h2 class="panel-title">Bibliothek im Hintergrund erweitern</h2></div>
  <div class="body">
    {#if mode}
      <BackgroundMode {mode} onChange={change} />
    {:else}
      <p class="faint">Solange PokerStars läuft, ist die Bibliothek gesperrt.</p>
    {/if}
  </div>
</section>

<style>
  .body {
    padding: 12px 18px 18px;
  }
  p {
    margin: 0;
    font-size: 12.5px;
  }
</style>
