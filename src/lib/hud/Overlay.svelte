<script lang="ts">
  import { onMount, tick } from "svelte";
  import { api, inTauri, on } from "../api";
  import type { TableView } from "../types";
  import Felt from "../Felt.svelte";
  import HudLayer from "./HudLayer.svelte";

  let view = $state<TableView | null>(null);
  let width = $state(window.innerWidth);
  let height = $state(window.innerHeight);
  let dragging = false;
  let label = "demo";

  async function load() {
    view = await api.overlayView(label);
    await tick();
    reportRegions();
  }

  /** Tells the tracker where the panels are, so clicks elsewhere reach the poker table. */
  function reportRegions() {
    const dpr = window.devicePixelRatio;
    const regions = dragging
      ? [{ x: 0, y: 0, w: width * dpr, h: height * dpr }]
      : [...document.querySelectorAll(".hit")].map((el) => {
          const r = el.getBoundingClientRect();
          return { x: r.left * dpr, y: r.top * dpr, w: r.width * dpr, h: r.height * dpr };
        });
    api.setHitRegions(label, regions);
  }

  onMount(() => {
    document.body.className = inTauri ? "overlay" : "app";
    const unlisten: Promise<() => void>[] = [];
    (async () => {
      if (inTauri) {
        const { getCurrentWebviewWindow } = await import("@tauri-apps/api/webviewWindow");
        label = getCurrentWebviewWindow().label;
      }
      unlisten.push(on("overlay-table", load));
      unlisten.push(on<string>("table-updated", (key) => (!view || view.table_key === key) && load()));
      unlisten.push(on("settings-changed", load));
      load();
    })();

    const resize = () => {
      width = window.innerWidth;
      height = window.innerHeight;
    };
    window.addEventListener("resize", resize);
    const observer = new MutationObserver(() => requestAnimationFrame(reportRegions));
    observer.observe(document.body, { subtree: true, childList: true, attributes: true, attributeFilter: ["style"] });
    return () => {
      window.removeEventListener("resize", resize);
      observer.disconnect();
      unlisten.forEach((u) => u.then((f) => f()));
    };
  });
</script>

<div class="root" class:demo={!inTauri}>
  {#if !inTauri}
    <Felt />
  {/if}
  {#if view}
    <HudLayer
      {view}
      {width}
      {height}
      onDragging={(d) => {
        dragging = d;
        reportRegions();
      }}
    />
  {/if}
</div>

<style>
  .root {
    position: fixed;
    inset: 0;
  }
</style>
