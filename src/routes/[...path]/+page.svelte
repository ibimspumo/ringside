<script lang="ts">
  import "$lib/theme.css";
  import Overlay from "$lib/hud/Overlay.svelte";
  import Main from "$lib/app/Main.svelte";

  const view = new URLSearchParams(window.location.search).get("view");
</script>

{#if view === "overlay"}
  <Overlay />
{:else}
  <Main />
{/if}
