<script lang="ts">
  import "$lib/theme.css";
  import Overlay from "$lib/hud/Overlay.svelte";
  import Main from "$lib/app/Main.svelte";

  const view = new URLSearchParams(window.location.search).get("view");
</script>

{#if view === "overlay"}
  <Overlay />
{:else if view === "kit"}
  {#await import("$lib/ui/Kit.svelte") then kit}<kit.default />{/await}
{:else if view === "table-lab" && import.meta.env.DEV}
  {#await import("$lib/table/TableLab.svelte") then lab}<lab.default />{/await}
{:else}
  <Main />
{/if}
