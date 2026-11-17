<script lang="ts">
  import { onMount, tick } from "svelte";
  import { api, inTauri, on } from "../api";
  import Felt from "../Felt.svelte";
  import type { TableView } from "../types";
  import HudLayer from "./HudLayer.svelte";

  const params = new URLSearchParams(window.location.search);
  /** Browser preview only: a screenshot of a real table to check placement and contrast. */
  const backdrop = inTauri ? null : params.get("bg");

  let view = $state<TableView | null>(null);
  /** PokerStars saves no hand histories until the player turns it on, so the table stays empty. */
  let noHistory = $state(false);
  let width = $state(window.innerWidth);
  let height = $state(window.innerHeight);
  let recapUntil = $state(0);
  let now = $state(Date.now());
  let dragging = false;
  let label = "demo";
  let shapeKey = "";

  const shape = () => (Math.round((width / Math.max(height, 1)) * 20) / 20).toFixed(2);

  async function load() {
    shapeKey = shape();
    const next = await api.overlayView(label, width / Math.max(height, 1));
    if (!inTauri && next) {
      // Browser preview: ?mode=classic&preset=standard
      const mode = params.get("mode");
      if (mode === "classic" || mode === "graphic") next.hud.mode = mode;
      const preset = params.get("preset");
      if (preset) next.hud.classic.preset = preset as typeof next.hud.classic.preset;
      if (params.has("nocover")) next.hud.graphic.cover_plate = false;
    }
    // A new hand at this table: show the one that just ended for a moment.
    if (view && next && next.hand_id !== view.hand_id && next.hud.recap_seconds > 0) {
      recapUntil = Date.now() + next.hud.recap_seconds * 1000;
    }
    view = next;
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
    if (!inTauri && params.has("recap")) recapUntil = Number.MAX_SAFE_INTEGER;
    const unlisten: Promise<() => void>[] = [];
    (async () => {
      if (inTauri) {
        const { getCurrentWebviewWindow } = await import("@tauri-apps/api/webviewWindow");
        label = getCurrentWebviewWindow().label;
      }
      if (inTauri) {
        api.importStatus().then((st) => (noHistory = st.dirs.length === 0));
        unlisten.push(on<{ dirs: string[] }>("import-status", (st) => (noHistory = st.dirs.length === 0)));
      }
      unlisten.push(on("overlay-table", load));
      unlisten.push(on<string>("table-updated", (key) => (!view || view.table_key === key) && load()));
      unlisten.push(on("settings-changed", load));
      unlisten.push(on("stats-rebuilt", load));
      unlisten.push(on("notes-changed", load));
      load();
    })();

    const resize = () => {
      width = window.innerWidth;
      height = window.innerHeight;
      // Another window shape has its own saved layout.
      if (shape() !== shapeKey) load();
    };
    window.addEventListener("resize", resize);
    const clock = setInterval(() => (now = Date.now()), 250);
    const observer = new MutationObserver(() => requestAnimationFrame(reportRegions));
    observer.observe(document.body, { subtree: true, childList: true, attributes: true, attributeFilter: ["style"] });
    return () => {
      window.removeEventListener("resize", resize);
      clearInterval(clock);
      observer.disconnect();
      unlisten.forEach((u) => u.then((f) => f()));
    };
  });
</script>

<div class="root">
  {#if backdrop}
    <img class="backdrop" src={backdrop} alt="" />
  {:else if !inTauri}
    <Felt />
  {/if}
  {#if !view && inTauri}
    {#if noHistory}
      <div class="waiting">
        Ringside sieht keine Hände: PokerStars speichert noch keine Hand-Historys. In PokerStars unter
        Einstellungen → Spielhistorie das Speichern der Hand-Historys einschalten.
      </div>
    {:else}
      <div class="waiting">Ringside · wartet auf die erste Hand an diesem Tisch</div>
    {/if}
  {/if}
  {#if view}
    <HudLayer
      {view}
      {width}
      {height}
      showRecap={now < recapUntil}
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
  .waiting {
    position: absolute;
    left: 10px;
    bottom: 10px;
    padding: 4px 9px;
    border-radius: 6px;
    background: #14161a;
    color: #a4a7ad;
    max-width: min(560px, calc(100% - 20px));
    font-size: 11.5px;
    line-height: 1.4;
    pointer-events: none;
  }
  .backdrop {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
  }
</style>
