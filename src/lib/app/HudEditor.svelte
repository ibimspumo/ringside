<script lang="ts">
  import { onMount } from "svelte";
  import { api, inTauri, on } from "../api";
  import { demoTable } from "../demo";
  import Felt from "../Felt.svelte";
  import HudLayer from "../hud/HudLayer.svelte";
  import { REFERENCE_ASPECT } from "../hud/geometry";
  import { CLASSIC_PRESETS, PALETTES, RATED_STATS, STAT_META } from "../stats";
  import type { ClassicPreset, HudConfig, Palette, Preset, Settings, StatId, TableView } from "../types";

  const SIZES = [2, 3, 4, 5, 6, 7, 8, 9, 10];
  const SHAPES = [
    { id: "standard", label: "Standard", aspect: REFERENCE_ASPECT },
    { id: "wide", label: "Breit 16:9", aspect: 16 / 9 },
  ];

  let settings = $state<Settings | null>(null);
  let base = $state<TableView | null>(null);
  let view = $state<TableView | null>(null);
  let maxSeats = $state(8);
  let shape = $state("standard");
  let showRecap = $state(true);
  let stageW = $state(0);
  let stageH = $state(0);
  let message = $state<{ ok: boolean; text: string } | null>(null);
  let saveTimer: ReturnType<typeof setTimeout> | undefined;

  /** A table screenshot to calibrate against; `crop` removes the window title bar. */
  let shot = $state<{ url: string; w: number; h: number; crop: number } | null>(null);

  const aspect = $derived(
    shape === "screenshot" && shot ? shot.w / Math.max(shot.h - shot.crop, 1) : (SHAPES.find((x) => x.id === shape)?.aspect ?? REFERENCE_ASPECT),
  );
  const customised = $derived(!!view && !!settings?.layouts[view.layout_key]);

  let presetName = $state("");

  onMount(() => {
    // Positions dragged at a table (or anything saved elsewhere) show up here live.
    const sub = on<Settings>("settings-changed", (s) => {
      if (!saveTimer) settings = s;
    });
    (async () => {
      settings = await api.getSettings();
      base = await api.previewView(REFERENCE_ASPECT);
      if (base) maxSeats = base.max_seats;
    })();
    return () => sub.then((f) => f());
  });

  // Rebuild the preview whenever the table size or window shape changes.
  $effect(() => {
    const s = settings;
    const n = maxSeats;
    const a = aspect;
    if (!s) return;
    api.layoutFor(n, a).then(({ key, layout }) => {
      const seats = base && base.max_seats === n ? base : demoTable(n);
      view = { ...seats, hud: $state.snapshot(s.hud) as HudConfig, layout_key: key, layout };
    });
  });

  function changed() {
    if (!settings) return;
    if (view) view.hud = $state.snapshot(settings.hud) as HudConfig;
    clearTimeout(saveTimer);
    const snapshot = $state.snapshot(settings) as Settings;
    saveTimer = setTimeout(() => {
      saveTimer = undefined;
      api.saveSettings(snapshot);
    }, 250);
  }

  async function savePreset() {
    if (!settings) return;
    const name = presetName.trim() || `Preset ${settings.presets.length + 1}`;
    const preset: Preset = $state.snapshot({
      name,
      preferred_seating: settings.preferred_seating,
      hud: settings.hud,
      layouts: settings.layouts,
    }) as Preset;
    settings.presets = [...settings.presets.filter((p) => p.name !== name), preset];
    presetName = "";
    await api.saveSettings($state.snapshot(settings) as Settings);
    message = { ok: true, text: `Preset „${name}“ gespeichert.` };
  }

  async function applyPreset(p: Preset) {
    if (!settings) return;
    settings.hud = structuredClone(p.hud);
    settings.layouts = structuredClone(p.layouts);
    settings.preferred_seating = p.preferred_seating;
    await api.saveSettings($state.snapshot(settings) as Settings);
    message = { ok: true, text: `Preset „${p.name}“ gilt jetzt an allen Tischen.` };
  }

  async function deletePreset(p: Preset) {
    if (!settings) return;
    settings.presets = settings.presets.filter((x) => x.name !== p.name);
    await api.saveSettings($state.snapshot(settings) as Settings);
  }

  function setMode(mode: HudConfig["mode"]) {
    settings!.hud.mode = mode;
    changed();
  }

  function setPreset(id: ClassicPreset) {
    settings!.hud.classic.preset = id;
    changed();
  }

  function toggleCustom(id: StatId) {
    const list = settings!.hud.classic.custom;
    settings!.hud.classic.custom = list.includes(id) ? list.filter((x) => x !== id) : list.length < 8 ? [...list, id] : list;
    changed();
  }

  async function resetLayout() {
    if (!view) return;
    await api.resetLayout(view.layout_key);
    settings = await api.getSettings();
  }

  function loadShot(e: Event) {
    const file = (e.currentTarget as HTMLInputElement).files?.[0];
    if (!file) return;
    const url = URL.createObjectURL(file);
    const img = new Image();
    img.onload = () => {
      if (shot) URL.revokeObjectURL(shot.url);
      shot = { url, w: img.naturalWidth, h: img.naturalHeight, crop: 0 };
      shape = "screenshot";
    };
    img.src = url;
  }

  function dropShot() {
    if (shot) URL.revokeObjectURL(shot.url);
    shot = null;
    shape = "standard";
  }

  async function exportProfile() {
    const { save } = await import("@tauri-apps/plugin-dialog");
    const path = await save({ defaultPath: "ringside-profil.json", filters: [{ name: "Ringside-Profil", extensions: ["json"] }] });
    if (!path) return;
    try {
      await api.exportProfile(path);
      message = { ok: true, text: `Gespeichert: ${path}` };
    } catch (e) {
      message = { ok: false, text: String(e) };
    }
  }

  async function importProfile() {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const path = await open({ multiple: false, filters: [{ name: "Ringside-Profil", extensions: ["json"] }] });
    if (typeof path !== "string") return;
    try {
      await api.importProfile(path);
      settings = await api.getSettings();
      message = { ok: true, text: "Profil übernommen." };
    } catch (e) {
      message = { ok: false, text: String(e) };
    }
  }

  async function resetProfile() {
    await api.resetProfile();
    settings = await api.getSettings();
    message = { ok: true, text: "Darstellung und Positionen auf Standard zurückgesetzt." };
  }
</script>

<div class="page wide">

  {#if settings && view}
    <div class="layout">
      <section class="stage-col">
        <div class="toolbar">
          <label class="group">
            <span>Tisch</span>
            <select class="field" bind:value={maxSeats}>
              {#each SIZES as n (n)}<option value={n}>{n} Plätze</option>{/each}
            </select>
          </label>
          <div class="group">
            <span>Fenster</span>
            <div class="seg">
              {#each SHAPES as sh (sh.id)}
                <button class:on={shape === sh.id} onclick={() => (shape = sh.id)}>{sh.label}</button>
              {/each}
              {#if shot}
                <button class:on={shape === "screenshot"} onclick={() => (shape = "screenshot")}>Wie Screenshot</button>
              {/if}
            </div>
          </div>
          <label class="check toggle">
            <input type="checkbox" bind:checked={showRecap} />
            <span>Rückblick zeigen</span>
          </label>
        </div>

        <div class="stage" style="aspect-ratio:{aspect}" bind:clientWidth={stageW} bind:clientHeight={stageH}>
          {#if shot && shape === "screenshot"}
            <img
              class="shot"
              src={shot.url}
              alt="Tisch-Screenshot"
              style="top:{(-shot.crop / Math.max(shot.h - shot.crop, 1)) * 100}%"
            />
          {:else}
            <Felt />
          {/if}
          {#if stageW > 0}
            <HudLayer
              {view}
              width={stageW}
              height={stageH}
              {showRecap}
              editRecap
              preview={!(shot && shape === "screenshot")}
              onLayoutChange={() => api.getSettings().then((s) => (settings = s))}
            />
          {/if}
        </div>

        <div class="stage-foot">
          <span class="faint">
            {maxSeats} Plätze · {shape === "wide" ? "Breitbild" : shape === "screenshot" ? "Fenster wie Screenshot" : "Standardfenster"} ·
            {customised ? "eigene Positionen" : "Standard-Positionen"}
          </span>
          <div class="row">
            <label class="btn">
              Tisch-Screenshot laden
              <input type="file" accept="image/*" onchange={loadShot} hidden />
            </label>
            {#if shot}
              <label class="crop">
                <span>Titelleiste weg</span>
                <input type="range" min="0" max={Math.round(shot.h * 0.12)} bind:value={shot.crop} />
                <em class="num">{shot.crop} px</em>
              </label>
              <button class="btn ghost" onclick={dropShot}>Screenshot entfernen</button>
            {/if}
            <button class="btn ghost" onclick={resetLayout} disabled={!customised}>Positionen zurücksetzen</button>
          </div>
        </div>
        <p class="hint">
          Tipp: Screenshot eines PokerStars-Tischs laden, die Titelleiste wegschneiden und Ring, Platte oder Box auf die
          Avatare ziehen. Gespeichert wird einmal je Tischgröße und gilt für jede Fenstergröße, am Tisch wie hier. Mit „Rückblick zeigen“ lassen sich auch die Karten jedes Spielers und das Board verschieben. Im grafischen Modus bewegen sich Ring und Platte gemeinsam; im klassischen Modus legt ein Doppelklick die Box wieder an
          den Avatar.
        </p>
      </section>

      <aside class="inspector">
        <section class="panel">
          <div class="panel-head"><h2 class="panel-title">Darstellung</h2></div>
          <div class="body">
            <div class="modes">
              <button class="mode" class:on={settings.hud.mode === "graphic"} onclick={() => setMode("graphic")}>
                <b>Grafisch</b><span>Ring um den Avatar, Platte über dem Namensschild</span>
              </button>
              <button class="mode" class:on={settings.hud.mode === "classic"} onclick={() => setMode("classic")}>
                <b>Klassisch</b><span>Eine Textbox pro Spieler</span>
              </button>
            </div>

            {#if settings.hud.mode === "graphic"}
              <div class="sub">Ring – vier Segmente im Uhrzeigersinn</div>
              <div class="grid2">
                {#each settings.hud.graphic.segments as _, i (i)}
                  <select class="field" bind:value={settings.hud.graphic.segments[i]} onchange={changed}>
                    {#each RATED_STATS as id (id)}<option value={id}>{STAT_META[id].name}</option>{/each}
                  </select>
                {/each}
              </div>
              <div class="sub">Platte – drei Zahlen je Seite, ein Klick auf die Platte blättert</div>
              {#each settings.hud.graphic.plate_pages as page, p (p)}
                <div class="page-row">
                  <span class="num faint">{p + 1}</span>
                  {#each page as _, i (i)}
                    <select class="field" bind:value={settings.hud.graphic.plate_pages[p][i]} onchange={changed}>
                      {#each RATED_STATS as id (id)}<option value={id}>{STAT_META[id].short}</option>{/each}
                    </select>
                  {/each}
                  <button
                    class="btn ghost"
                    disabled={settings.hud.graphic.plate_pages.length < 2}
                    onclick={() => ((settings!.hud.graphic.plate_pages = settings!.hud.graphic.plate_pages.filter((_, j) => j !== p)), changed())}
                    aria-label="Seite {p + 1} entfernen">✕</button
                  >
                </div>
              {/each}
              {#if settings.hud.graphic.plate_pages.length < 5}
                <button
                  class="btn ghost add"
                  onclick={() => ((settings!.hud.graphic.plate_pages = [...settings!.hud.graphic.plate_pages, ["vpip", "pfr", "three_bet"]]), changed())}
                  >Seite hinzufügen</button
                >
              {/if}
              <label class="slider">
                <span>Breite</span>
                <input type="range" min="0.8" max="1.5" step="0.02" bind:value={settings.hud.graphic.plate_width} oninput={changed} />
                <em class="num">{Math.round(settings.hud.graphic.plate_width * 100)} %</em>
              </label>
              <label class="check">
                <input type="checkbox" bind:checked={settings.hud.graphic.cover_plate} onchange={changed} />
                <span>
                  Namensschild von PokerStars überdecken
                  <small>Sieht aus wie früher Jivaro. Verdeckt aber auch „Check“, „Fold“ und den Zeitbalken.</small>
                </span>
              </label>
            {:else}
              <div class="presets">
                {#each Object.entries(CLASSIC_PRESETS) as [id, p] (id)}
                  <button class="preset" class:on={settings.hud.classic.preset === id} onclick={() => setPreset(id as ClassicPreset)}>
                    <b>{p.name}</b><span>{p.description}</span>
                  </button>
                {/each}
                <button class="preset" class:on={settings.hud.classic.preset === "custom"} onclick={() => setPreset("custom")}>
                  <b>Eigene</b><span>Werte selbst wählen, vier pro Zeile</span>
                </button>
              </div>
              {#if settings.hud.classic.preset === "custom"}
                <div class="chips">
                  {#each [...RATED_STATS, "hands" as StatId] as id (id)}
                    {@const n = settings.hud.classic.custom.indexOf(id)}
                    <button class="chip" class:on={n >= 0} title={STAT_META[id].help} onclick={() => toggleCustom(id)}>
                      {STAT_META[id].short}{#if n >= 0}<i>{n + 1}</i>{/if}
                    </button>
                  {/each}
                </div>
              {/if}
            {/if}
          </div>
        </section>

        <section class="panel">
          <div class="panel-head"><h2 class="panel-title">Anzeige</h2></div>
          <div class="body">
            <label class="slider">
              <span>Größe</span>
              <input type="range" min="0.6" max="1.6" step="0.05" bind:value={settings.hud.scale} oninput={changed} />
              <em class="num">{Math.round(settings.hud.scale * 100)} %</em>
            </label>
            {#if settings.hud.mode === "classic"}
              <label class="slider">
                <span>Deckkraft</span>
                <input type="range" min="0.5" max="1" step="0.01" bind:value={settings.hud.opacity} oninput={changed} />
                <em class="num">{Math.round(settings.hud.opacity * 100)} %</em>
              </label>
            {/if}
            <label class="slider">
              <span>Rückblick</span>
              <input type="range" min="0" max="10" step="1" bind:value={settings.hud.recap_seconds} oninput={changed} />
              <em class="num">{settings.hud.recap_seconds ? `${settings.hud.recap_seconds} s` : "aus"}</em>
            </label>
            <label class="check">
              <input type="checkbox" bind:checked={settings.hud.show_stack} onchange={changed} />
              <span>Stack zeigen <small>Stand zu Beginn der letzten Hand</small></span>
            </label>
            <label class="check">
              <input type="checkbox" bind:checked={settings.hud.show_bounty} onchange={changed} />
              <span>Bounty zeigen <small>bei Knockout-Turnieren</small></span>
            </label>
            <label class="check">
              <input type="checkbox" bind:checked={settings.hud.hero_session_only} onchange={changed} />
              <span>Eigene Werte nur aus diesem Turnier <small>bei Cash Games: dieser Tisch heute</small></span>
            </label>
            <label class="check">
              <input type="checkbox" bind:checked={settings.hud.split_by_game} onchange={changed} />
              <span>Cash Games und Turniere getrennt zählen</span>
            </label>
          </div>
        </section>

        <section class="panel">
          <div class="panel-head"><h2 class="panel-title">Farben</h2></div>
          <div class="body">
            <div class="seg palettes">
              {#each Object.entries(PALETTES) as [id, p] (id)}
                <button class:on={settings.hud.palette === id} onclick={() => ((settings!.hud.palette = id as Palette), changed())}>
                  <i style="background:{p.low}"></i><i style="background:{p.mid}"></i><i style="background:{p.high}"></i>
                  {p.name}
                </button>
              {/each}
            </div>
            <div class="thresholds">
              <div class="trow head"><span>Wert</span><span>unter</span><span>über</span></div>
              {#each RATED_STATS as id (id)}
                {@const t = settings.hud.thresholds[id]}
                {#if t}
                  <div class="trow" title={STAT_META[id].help}>
                    <span>{STAT_META[id].name}</span>
                    <input class="field num" type="number" step="0.5" bind:value={t.low} onchange={changed} style="--c:{PALETTES[settings.hud.palette].low}" />
                    <input class="field num" type="number" step="0.5" bind:value={t.high} onchange={changed} style="--c:{PALETTES[settings.hud.palette].high}" />
                  </div>
                {/if}
              {/each}
            </div>
            <p class="note">Je Wert höchstens drei Farben – so verlangt es PokerStars für HUDs.</p>
          </div>
        </section>

        <section class="panel">
          <div class="panel-head"><h2 class="panel-title">Presets</h2></div>
          <div class="body">
            <p class="note">
              Ein Preset merkt sich Darstellung, Farben und alle Positionen. Was du am Tisch oder hier verschiebst, gilt sofort
              für alle Tische gleicher Größe.
            </p>
            {#each settings.presets as p (p.name)}
              <div class="preset-row">
                <span>{p.name}</span>
                <button class="btn" onclick={() => applyPreset(p)}>Anwenden</button>
                <button class="btn ghost" onclick={() => deletePreset(p)} aria-label="Preset {p.name} löschen">Löschen</button>
              </div>
            {/each}
            <div class="row">
              <input class="field grow" placeholder="Name, z. B. Turniere" bind:value={presetName} />
              <button class="btn primary" onclick={savePreset}>Als Preset speichern</button>
            </div>
          </div>
        </section>

        <section class="panel">
          <div class="panel-head"><h2 class="panel-title">Datei</h2></div>
          <div class="body">
            <p class="note">Als Datei sichern oder weitergeben, etwa um sie als Standard in eine neue Version zu übernehmen.</p>
            <div class="row">
              <button class="btn" onclick={exportProfile} disabled={!inTauri}>Exportieren</button>
              <button class="btn" onclick={importProfile} disabled={!inTauri}>Importieren</button>
              <button class="btn ghost" onclick={resetProfile}>Auf Standard</button>
            </div>
            {#if message}<p class="msg" class:bad={!message.ok}>{message.text}</p>{/if}
          </div>
        </section>
      </aside>
    </div>
  {/if}
</div>

<style>
  .page {
    padding-top: 20px;
  }
  .wide {
    max-width: 1560px;
  }
  .layout {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 340px;
    gap: 18px;
    align-items: start;
  }
  .stage-col {
    position: sticky;
    top: 16px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .toolbar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 16px;
  }
  .group {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--text-2);
    font-size: 13px;
  }
  .toggle {
    margin-left: auto;
    align-items: center;
    font-size: 13px;
    color: var(--text-2);
  }
  .toggle input {
    margin: 0;
  }
  .stage {
    position: relative;
    width: 100%;
    max-height: calc(100vh - 230px);
    border-radius: var(--r);
    overflow: hidden;
    border: 1px solid var(--line-2);
    background: #0f1114;
  }
  .shot {
    position: absolute;
    left: 0;
    width: 100%;
    pointer-events: none;
  }
  .stage-foot {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
  .crop {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12.5px;
    color: var(--text-2);
  }
  .crop em {
    font-style: normal;
    min-width: 40px;
  }
  .hint {
    margin: 0;
    font-size: 12.5px;
    line-height: 1.55;
    color: var(--text-3);
    max-width: 760px;
  }

  .inspector {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px 16px 16px;
  }
  .sub {
    margin-top: 4px;
    font-size: 12px;
    color: var(--text-2);
  }
  .modes,
  .presets {
    display: grid;
    gap: 6px;
  }
  .modes {
    grid-template-columns: 1fr 1fr;
  }
  .mode,
  .preset {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 9px 11px;
    border-radius: var(--r-sm);
    border: 1px solid var(--line-2);
    background: var(--bg);
    text-align: left;
    cursor: pointer;
  }
  .mode span,
  .preset span {
    font-size: 12px;
    line-height: 1.35;
    color: var(--text-3);
  }
  .mode.on,
  .preset.on {
    border-color: var(--text);
    background: var(--surface-2);
  }
  .mode.on span,
  .preset.on span {
    color: var(--text-2);
  }
  .grid2 {
    display: grid;
    gap: 6px;
  }
  .grid2 {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
  .page-row {
    display: grid;
    grid-template-columns: 14px repeat(3, minmax(0, 1fr)) 30px;
    align-items: center;
    gap: 6px;
  }
  .page-row select {
    min-width: 0;
  }
  .page-row .btn {
    padding: 0;
  }
  .add {
    align-self: flex-start;
  }
  .grid2 select {
    min-width: 0;
    width: 100%;
  }
  .check {
    font-size: 13px;
  }
  small {
    display: block;
    margin-top: 1px;
    font-size: 12px;
    color: var(--text-3);
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
  }
  .chip {
    padding: 4px 9px;
    border-radius: 99px;
    border: 1px solid var(--line-2);
    background: transparent;
    color: var(--text-2);
    font-family: var(--font-num);
    font-size: 12.5px;
    cursor: pointer;
  }
  .chip.on {
    border-color: var(--text);
    color: var(--text);
  }
  .chip i {
    margin-left: 4px;
    font-style: normal;
    font-size: 10px;
    color: var(--text-3);
  }
  .slider {
    display: grid;
    grid-template-columns: 74px 1fr 46px;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    color: var(--text-2);
  }
  .slider em {
    font-style: normal;
    text-align: right;
    color: var(--text);
  }
  .palettes {
    align-self: flex-start;
  }
  .palettes button {
    display: inline-flex;
    align-items: center;
    gap: 3px;
  }
  .palettes i {
    width: 8px;
    height: 8px;
    border-radius: 50%;
  }
  .palettes i:last-of-type {
    margin-right: 5px;
  }
  .thresholds {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .trow {
    display: grid;
    grid-template-columns: 1fr 62px 62px;
    align-items: center;
    gap: 6px;
    font-size: 13px;
    color: var(--text-2);
  }
  .trow.head {
    font-size: 11px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-3);
  }
  .trow.head span:not(:first-child) {
    text-align: right;
    padding-right: 4px;
  }
  .trow input {
    height: 28px;
    text-align: right;
    border-left: 3px solid var(--c);
  }
  .note {
    margin: 0;
    font-size: 12px;
    line-height: 1.5;
    color: var(--text-3);
  }
  .preset-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto auto;
    align-items: center;
    gap: 6px;
    font-size: 13px;
  }
  .preset-row span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .msg {
    margin: 0;
    font-size: 12.5px;
    color: var(--pos);
    overflow-wrap: anywhere;
  }
  .msg.bad {
    color: #ff9d93;
  }
  @media (max-width: 1180px) {
    .layout {
      grid-template-columns: 1fr;
    }
    .stage-col {
      position: static;
    }
  }
</style>
