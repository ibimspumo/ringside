<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "../api";
  import { demoTable } from "../demo";
  import Felt from "../Felt.svelte";
  import HudLayer from "../hud/HudLayer.svelte";
  import { ALL_STATS, STAT_META } from "../stats";
  import type { Settings, StatId, TableView } from "../types";

  let settings = $state<Settings | null>(null);
  let view = $state<TableView | null>(null);
  let stageW = $state(0);
  let stageH = $state(0);
  let saveTimer: ReturnType<typeof setTimeout> | undefined;

  const RING_STATS = ALL_STATS.filter((id) => id !== "hands");

  onMount(async () => {
    settings = await api.getSettings();
    view = (await api.previewView()) ?? demoTable();
  });

  function changed() {
    if (!settings || !view) return;
    view.hud = settings.hud;
    clearTimeout(saveTimer);
    const snapshot = $state.snapshot(settings);
    saveTimer = setTimeout(() => api.saveSettings(snapshot), 250);
  }

  function toggleRow(id: StatId) {
    if (!settings) return;
    const rows = settings.hud.rows;
    settings.hud.rows = rows.includes(id) ? rows.filter((r) => r !== id) : rows.length < 4 ? [...rows, id] : rows;
    changed();
  }

  async function resetLayout() {
    if (!view) return;
    await api.resetLayout(view.max_seats);
    view = (await api.previewView()) ?? demoTable();
    if (settings) settings.layouts = (await api.getSettings()).layouts;
  }
</script>

<div class="page wide">
  <div class="page-head">
    <div>
      <h1>HUD anpassen</h1>
      <p>Änderungen erscheinen sofort an allen offenen Tischen. Panels lassen sich hier und am Tisch verschieben.</p>
    </div>
  </div>

  {#if settings && view}
    <div class="layout">
      <div class="stage panel" bind:clientWidth={stageW} bind:clientHeight={stageH}>
        <Felt>
          {#each view.layout as p, i (i)}
            <div class="avatar" style="left:{p.x * 100}%; top:{p.y * 100}%; --s:{Math.min(Math.max(stageW / 960, 0.5), 1.7) * settings.hud.scale}">
              {view.seats.find((s) => s.visual === i)?.name.slice(0, 1) ?? ""}
            </div>
          {/each}
          {#if stageW > 0}
            <HudLayer {view} width={stageW} height={stageH} />
          {/if}
        </Felt>
      </div>

      <div class="controls">
        <section class="panel">
          <h2 class="panel-title">Ring um den Avatar</h2>
          <div class="body">
            <label>
              <span>Außen</span>
              <select class="field" bind:value={settings.hud.ring_outer} onchange={changed}>
                {#each RING_STATS as id (id)}<option value={id}>{STAT_META[id].name}</option>{/each}
              </select>
            </label>
            <label>
              <span>Innen</span>
              <select class="field" bind:value={settings.hud.ring_inner} onchange={changed}>
                {#each RING_STATS as id (id)}<option value={id}>{STAT_META[id].name}</option>{/each}
              </select>
            </label>
          </div>
        </section>

        <section class="panel">
          <h2 class="panel-title">Werte in der Karte <small>(max. 4)</small></h2>
          <div class="body chips">
            {#each RING_STATS as id (id)}
              <button
                class="chip"
                class:on={settings.hud.rows.includes(id)}
                title={STAT_META[id].help}
                onclick={() => toggleRow(id)}
              >
                {STAT_META[id].short}
                {#if settings.hud.rows.includes(id)}<i>{settings.hud.rows.indexOf(id) + 1}</i>{/if}
              </button>
            {/each}
          </div>
        </section>

        <section class="panel">
          <h2 class="panel-title">Darstellung</h2>
          <div class="body">
            <label>
              <span>Größe</span>
              <input type="range" min="0.6" max="1.6" step="0.05" bind:value={settings.hud.scale} oninput={changed} />
              <em class="num">{Math.round(settings.hud.scale * 100)} %</em>
            </label>
            <label>
              <span>Deckkraft</span>
              <input type="range" min="0.4" max="1" step="0.02" bind:value={settings.hud.opacity} oninput={changed} />
              <em class="num">{Math.round(settings.hud.opacity * 100)} %</em>
            </label>
            <label class="check">
              <input type="checkbox" bind:checked={settings.hud.show_stack} onchange={changed} />
              Stack anzeigen (Turniere in BB)
            </label>
            <label class="check">
              <input type="checkbox" bind:checked={settings.hud.split_by_game} onchange={changed} />
              Cash und Turniere getrennt zählen
            </label>
            <button class="btn" onclick={resetLayout}>Sitzpositionen für {view.max_seats}-max zurücksetzen</button>
          </div>
        </section>

        <section class="panel">
          <h2 class="panel-title">Farbschwellen</h2>
          <div class="legend">
            <span><i style="background:var(--ice)"></i>darunter</span>
            <span><i style="background:var(--neutral)"></i>dazwischen</span>
            <span><i style="background:var(--fire)"></i>darüber</span>
          </div>
          <div class="body thresholds">
            {#each RING_STATS as id (id)}
              {@const t = settings.hud.thresholds[id]}
              {#if t}
                <div class="trow" title={STAT_META[id].help}>
                  <span>{STAT_META[id].name}</span>
                  <input class="field num" type="number" step="0.5" bind:value={t.low} onchange={changed} />
                  <input class="field num" type="number" step="0.5" bind:value={t.high} onchange={changed} />
                </div>
              {/if}
            {/each}
          </div>
          <p class="note">
            Höchstens drei Farben je Wert, keine Spieler-Etiketten und keine situationsabhängigen Werte – so bleibt
            das HUD innerhalb der PokerStars-Regeln für Hilfsprogramme.
          </p>
        </section>
      </div>
    </div>
  {/if}
</div>

<style>
  .wide {
    max-width: 1500px;
  }
  .layout {
    display: grid;
    grid-template-columns: 1fr 340px;
    gap: 16px;
    align-items: start;
  }
  .stage {
    position: sticky;
    top: 20px;
    aspect-ratio: 4 / 3;
    overflow: hidden;
  }
  .avatar {
    position: absolute;
    width: calc(56px * var(--s));
    height: calc(56px * var(--s));
    transform: translate(-50%, -50%);
    border-radius: 50%;
    background: radial-gradient(circle at 35% 30%, #3a4152, #1a1e28);
    display: grid;
    place-items: center;
    font-family: var(--font-num);
    font-size: calc(20px * var(--s));
    color: rgba(255, 255, 255, 0.35);
  }
  .controls {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .panel-title small {
    text-transform: none;
    letter-spacing: 0;
    font-weight: 400;
    color: var(--text-faint);
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px 18px 16px;
  }
  label {
    display: grid;
    grid-template-columns: 76px 1fr auto;
    align-items: center;
    gap: 10px;
    color: var(--text-dim);
  }
  label select {
    grid-column: 2 / 4;
  }
  label em {
    font-style: normal;
    color: var(--text);
    min-width: 42px;
    text-align: right;
  }
  input[type="range"] {
    accent-color: var(--brand);
  }
  .check {
    display: flex;
    gap: 9px;
    color: var(--text);
  }
  .check input {
    accent-color: var(--brand);
  }
  .chips {
    flex-direction: row;
    flex-wrap: wrap;
    gap: 6px;
  }
  .chip {
    position: relative;
    padding: 6px 11px;
    border-radius: 99px;
    border: 1px solid var(--line-strong);
    background: transparent;
    color: var(--text-dim);
    cursor: pointer;
    font-family: var(--font-num);
    letter-spacing: 0.04em;
  }
  .chip.on {
    background: rgba(255, 138, 61, 0.14);
    border-color: rgba(255, 138, 61, 0.55);
    color: var(--text);
  }
  .chip i {
    font-style: normal;
    font-size: 10px;
    margin-left: 4px;
    color: var(--brand);
  }
  .legend {
    display: flex;
    gap: 14px;
    padding: 10px 18px 0;
    font-size: 12px;
    color: var(--text-dim);
  }
  .legend i {
    display: inline-block;
    width: 9px;
    height: 9px;
    border-radius: 50%;
    margin-right: 6px;
  }
  .thresholds {
    gap: 6px;
  }
  .trow {
    display: grid;
    grid-template-columns: 1fr 70px 70px;
    gap: 8px;
    align-items: center;
    color: var(--text-dim);
  }
  .trow input {
    height: 30px;
    text-align: right;
  }
  .trow input:first-of-type {
    border-color: rgba(76, 198, 255, 0.35);
  }
  .trow input:last-of-type {
    border-color: rgba(255, 91, 58, 0.35);
  }
  .note {
    margin: 0;
    padding: 0 18px 16px;
    font-size: 12px;
    line-height: 1.5;
    color: var(--text-faint);
  }
</style>
