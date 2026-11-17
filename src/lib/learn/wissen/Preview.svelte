<script lang="ts">
  import { onMount } from "svelte";
  import Wissen from "./Wissen.svelte";

  // Mimics the app shell (200 px sidebar, scrolling page) so layouts can be checked in a browser.
  const params = new URLSearchParams(window.location.search);
  let openId = $state<string | null>(params.get("a"));

  onMount(() => {
    document.body.className = "app";
  });
</script>

<div class="shell">
  <aside></aside>
  <main>
    <div class="page">
      <Wissen {openId} onOpenChange={(id) => (openId = id)} />
    </div>
  </main>
</div>

<style>
  .shell {
    display: grid;
    grid-template-columns: 200px minmax(0, 1fr);
    height: 100vh;
  }
  aside {
    background: #0f1114;
    border-right: 1px solid var(--line);
  }
  main {
    overflow: auto;
    min-width: 0;
  }
</style>
