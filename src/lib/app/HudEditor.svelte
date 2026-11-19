<script lang="ts">
  import { onMount } from "svelte";
  import { api, inTauri, on } from "../api";
  import { demoTable } from "../demo";
  import Felt from "../Felt.svelte";
  import ClassicBox from "../hud/ClassicBox.svelte";
  import HudLayer from "../hud/HudLayer.svelte";
  import SeatAnatomy, { type Slot } from "../hud/SeatAnatomy.svelte";
  import ThresholdScale from "../hud/ThresholdScale.svelte";
  import { REFERENCE_ASPECT } from "../hud/geometry";
  import { CLASSIC_PRESETS, PALETTES, RATED_STATS, STAT_META, fmtUnit, stat, tone, toneColor } from "../stats";
  import type { ClassicPreset, HudConfig, Palette, Preset, SeatView, Settings, StatId, TableView } from "../types";

  const SIZES = [2, 3, 4, 5, 6, 7, 8, 9, 10];
  const SHAPES = [
    { id: "standard", label: "Standard", aspect: REFERENCE_ASPECT },
    { id: "wide", label: "Breit 16:9", aspect: 16 / 9 },
  ];
  const PANES = [
    { id: "look", label: "Aussehen" },
    { id: "colors", label: "Farben" },
    { id: "saved", label: "Vorlagen" },
  ] as const;
  type Pane = (typeof PANES)[number]["id"];

  /** What a low and a high value says about a player, for the colour scales. */
  const MEANING: Partial<Record<StatId, [string, string]>> = {
    vpip: ["spielt wenige Hände", "spielt sehr viele Hände"],
    pfr: ["erhöht selten", "erhöht sehr oft"],
    three_bet: ["re-raist kaum", "re-raist oft"],
    fold_to_three_bet: ["hält Re-Raises stand", "gibt auf Re-Raises auf"],
    steal: ["klaut selten", "klaut oft die Blinds"],
    fold_to_steal: ["verteidigt die Blinds", "gibt die Blinds leicht her"],
    cbet: ["setzt selten nach", "setzt fast immer nach"],
    fold_to_cbet: ["bleibt am Flop dabei", "gibt am Flop schnell auf"],
    wtsd: ["steigt früh aus", "geht oft bis zum Showdown"],
    wsd: ["verliert am Showdown", "gewinnt am Showdown"],
    af: ["callt lieber", "setzt und erhöht viel"],
    afq: ["passiv nach dem Flop", "aggressiv nach dem Flop"],
  };
  const QUARTER = ["oben links", "oben rechts", "unten rechts", "unten links"];
  /** Picker names where the stat's own name is only the abbreviation. */
  const PICK_NAME: Partial<Record<StatId, string>> = { vpip: "Freiwillig gespielt" };

  const params = new URLSearchParams(window.location.search);
  const askedPane = inTauri ? null : PANES.find((p) => p.id === params.get("pane"))?.id;
  let pane = $state<Pane>(askedPane ?? "look");

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

  /** Ring segment or plate number that the stat tiles below fill. */
  let slot = $state<Slot>({ kind: "ring", i: 0 });
  let platePage = $state(0);
  let exact = $state(false);

  /** A table screenshot to calibrate against; `crop` removes the window title bar. */
  let shot = $state<{ url: string; w: number; h: number; crop: number } | null>(null);

  const aspect = $derived(
    shape === "screenshot" && shot ? shot.w / Math.max(shot.h - shot.crop, 1) : (SHAPES.find((x) => x.id === shape)?.aspect ?? REFERENCE_ASPECT),
  );
  const customised = $derived(!!view && !!settings?.layouts[view.layout_key]);

  /** A typical opponent for the close-up and the preset previews. */
  const sample = $derived.by(() => {
    if (!view) return null;
    const hands = (s: SeatView) => stat(s.stats, "hands")?.value ?? 0;
    const pick = view.seats.filter((s) => !s.is_hero && !s.note?.label).sort((a, b) => hands(b) - hands(a));
    return pick[0] ?? view.seats[0] ?? null;
  });

  let presetName = $state("");

  onMount(() => {
    // Positions dragged at a table (or anything saved elsewhere) show up here live.
    const sub = on<Settings>("settings-changed", (s) => {
      if (!saveTimer) settings = s;
    });
    (async () => {
      const s = await api.getSettings();
      // Browser preview: ?tab=hud&mode=classic
      const mode = inTauri ? null : params.get("mode");
      if (mode === "classic" || mode === "graphic") s.hud.mode = mode;
      settings = s;
      base = await api.previewView(REFERENCE_ASPECT);
      if (base) maxSeats = base.max_seats;
    })();
    return () => sub.then((f) => f());
  });

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
    const name = presetName.trim() || `Vorlage ${settings.presets.length + 1}`;
    const preset: Preset = $state.snapshot({
      name,
      preferred_seating: settings.preferred_seating,
      hud: settings.hud,
      layouts: settings.layouts,
    }) as Preset;
    settings.presets = [...settings.presets.filter((p) => p.name !== name), preset];
    presetName = "";
    await api.saveSettings($state.snapshot(settings) as Settings);
    message = { ok: true, text: `Vorlage „${name}“ gespeichert.` };
  }

  async function applyPreset(p: Preset) {
    if (!settings) return;
    settings.hud = structuredClone(p.hud);
    settings.layouts = structuredClone(p.layouts);
    settings.preferred_seating = p.preferred_seating;
    await api.saveSettings($state.snapshot(settings) as Settings);
    message = { ok: true, text: `Vorlage „${p.name}“ gilt jetzt an allen Tischen.` };
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

  /** The stat in the selected ring segment or plate slot. */
  const slotStat = $derived.by((): StatId | null => {
    if (!settings) return null;
    const g = settings.hud.graphic;
    return slot.kind === "ring" ? (g.segments[slot.i] ?? null) : (g.plate_pages[platePage]?.[slot.i] ?? null);
  });
  const slotName = $derived(slot.kind === "ring" ? `Ring, Segment ${QUARTER[slot.i]}` : `Zahlenfeld, Seite ${platePage + 1}, Wert ${slot.i + 1}`);

  function assign(id: StatId) {
    const g = settings!.hud.graphic;
    if (slot.kind === "ring") g.segments[slot.i] = id;
    else g.plate_pages[platePage][slot.i] = id;
    changed();
  }

  function addPage() {
    const g = settings!.hud.graphic;
    g.plate_pages = [...g.plate_pages, ["vpip", "pfr", "three_bet"]];
    platePage = g.plate_pages.length - 1;
    slot = { kind: "plate", i: 0 };
    changed();
  }

  function removePage() {
    const g = settings!.hud.graphic;
    g.plate_pages = g.plate_pages.filter((_, j) => j !== platePage);
    platePage = Math.min(platePage, g.plate_pages.length - 1);
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

  const appOnly = () => (message = { ok: false, text: "Export und Import gehen nur in der App, nicht in der Browser-Vorschau." });

  async function exportProfile() {
    if (!inTauri) return appOnly();
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
    if (!inTauri) return appOnly();
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

  const marks = (id: StatId) =>
    (view?.seats ?? []).map((s) => stat(s.stats, id)?.value).filter((v): v is number => typeof v === "number");
</script>

<div class="page">
  {#if settings && view}
    {@const hud = settings.hud}
    {@const pal = PALETTES[hud.palette]}
    <div class="layout">
      <section class="stage-col">
        <div class="toolbar">
          <div class="group">
            <span class="lbl">Plätze</span>
            <div class="seg seats" role="radiogroup" aria-label="Plätze am Tisch">
              {#each SIZES as n (n)}
                <button class="num" class:on={maxSeats === n} role="radio" aria-checked={maxSeats === n} onclick={() => (maxSeats = n)}>{n}</button>
              {/each}
            </div>
          </div>
          <div class="group">
            <span class="lbl">Fenster</span>
            <div class="seg">
              {#each SHAPES as sh (sh.id)}
                <button class:on={shape === sh.id} onclick={() => (shape = sh.id)}>{sh.label}</button>
              {/each}
              {#if shot}
                <button class:on={shape === "screenshot"} onclick={() => (shape = "screenshot")}>Wie Screenshot</button>
              {/if}
            </div>
          </div>
          <label class="check toggle" title="Zeigt hier die Karten der letzten Hand und das Board, damit du sie verschieben kannst. Wie lange der Rückblick am Tisch steht, stellst du unter „Aussehen“ ein.">
            <input type="checkbox" bind:checked={showRecap} />
            <span>Rückblick in der Vorschau</span>
          </label>
        </div>

        <div class="stage-wrap">
          <div class="stage" style="aspect-ratio:{aspect}; --aspect:{aspect}; width:min(100%, calc((100vh - 250px) * {aspect}))" bind:clientWidth={stageW} bind:clientHeight={stageH}>
            {#if shot && shape === "screenshot"}
              <img class="shot" src={shot.url} alt="Tisch-Screenshot" style="top:{(-shot.crop / Math.max(shot.h - shot.crop, 1)) * 100}%" />
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
        </div>

        <div class="stage-foot">
          <ul class="legend">
            <li><b class="gesture">Ziehen</b>{hud.mode === "graphic" ? "verschiebt Ring und Zahlenfeld" : "verschiebt die Box"}</li>
            {#if hud.mode === "graphic"}
              <li><b class="gesture">Klick aufs Zahlenfeld</b>blättert zur nächsten Seite</li>
            {:else}
              <li><b class="gesture">Doppelklick</b>legt die Box zurück an den Avatar</li>
            {/if}
            <li><b class="gesture">Klick auf den Spieler</b>zeigt alle seine Werte</li>
            {#if hud.mode === "graphic"}
              <li><b class="gesture num">233</b>im Ring: gemeinsame Hände – je mehr, desto verlässlicher die Farben</li>
            {/if}
            {#if view.seats.some((s) => s.note?.label)}
              <li><b class="gesture frame">Farbiger Rahmen</b>deine eigene Spieler-Markierung aus PokerStars</li>
            {/if}
          </ul>
          <div class="row actions">
            <span class="state" class:custom={customised} title="Positionen gelten je Tischgröße, für jede Fenstergröße – am Tisch wie hier.">
              {customised ? `Eigene Positionen für ${maxSeats} Plätze` : `Standard-Positionen für ${maxSeats} Plätze`}
            </span>
            {#if customised}<button class="btn ghost" onclick={resetLayout}>Zurücksetzen</button>{/if}
            <label class="btn" title="Screenshot eines PokerStars-Tischs als Hintergrund, um Ring, Zahlenfeld oder Box genau auf die Avatare zu ziehen.">
              <svg width="15" height="15" viewBox="0 0 24 24" aria-hidden="true"><path d="M4 7.5h3l1.5-2h7L17 7.5h3v11H4zM12 16a3 3 0 1 0 0-6 3 3 0 0 0 0 6" /></svg>
              {shot ? "Anderen Screenshot" : "Tisch-Screenshot laden"}
              <input type="file" accept="image/*" onchange={loadShot} hidden />
            </label>
          </div>
        </div>
        {#if shot}
          <div class="shot-bar">
            <label class="crop">
              <span>Titelleiste abschneiden</span>
              <input type="range" min="0" max={Math.round(shot.h * 0.12)} bind:value={shot.crop} />
              <em class="num">{shot.crop} px</em>
            </label>
            <button class="btn ghost" onclick={dropShot}>Screenshot entfernen</button>
          </div>
        {/if}
      </section>

      <aside class="inspector panel">
        <div class="tabs" role="tablist">
          {#each PANES as p (p.id)}
            <button role="tab" aria-selected={pane === p.id} class:on={pane === p.id} onclick={() => (pane = p.id)}>{p.label}</button>
          {/each}
        </div>

        {#if pane === "look"}
          <section class="sec">
            <div class="modes">
              <button class="mode" class:on={hud.mode === "graphic"} onclick={() => setMode("graphic")}>
                <svg viewBox="0 0 120 44" aria-hidden="true">
                  <rect x="6" y="8" width="88" height="28" rx="6" class="m-plate" />
                  <rect x="14" y="24" width="12" height="5" rx="1.5" fill={pal.high} /><rect x="32" y="24" width="12" height="5" rx="1.5" fill={pal.mid} /><rect
                    x="50"
                    y="24"
                    width="12"
                    height="5"
                    rx="1.5"
                    fill={pal.low}
                  />
                  <rect x="14" y="14" width="34" height="4" rx="1.5" class="m-line" />
                  <circle cx="96" cy="22" r="16" class="m-ring" />
                  <path d="M80 22a16 16 0 0 1 16-16" stroke={pal.high} class="m-seg" /><path d="M96 6a16 16 0 0 1 16 16" stroke={pal.mid} class="m-seg" />
                  <path d="M112 22a16 16 0 0 1-16 16" stroke={pal.low} class="m-seg" />
                </svg>
                <b>Grafisch</b>
                <span>Farbring um den Avatar, Zahlen daneben</span>
              </button>
              <button class="mode" class:on={hud.mode === "classic"} onclick={() => setMode("classic")}>
                <svg viewBox="0 0 120 44" aria-hidden="true">
                  <rect x="22" y="5" width="76" height="34" rx="5" class="m-plate" />
                  <rect x="29" y="11" width="22" height="4" rx="1.5" class="m-line" />
                  <rect x="29" y="20" width="14" height="5" rx="1.5" fill={pal.high} /><rect x="47" y="20" width="14" height="5" rx="1.5" fill={pal.mid} /><rect
                    x="65"
                    y="20"
                    width="14"
                    height="5"
                    rx="1.5"
                    fill={pal.low}
                  />
                  <rect x="29" y="29" width="40" height="4" rx="1.5" class="m-line" />
                </svg>
                <b>Klassisch</b>
                <span>Eine Textbox je Spieler</span>
              </button>
            </div>
          </section>

          {#if hud.mode === "graphic" && sample}
            <section class="sec">
              <header class="sec-head">
                <h3>Was der Ring und das Zahlenfeld zeigen</h3>
                <p>Tippe ein Segment oder eine Zahl an und wähle darunter den Wert.</p>
              </header>
              <div class="pages">
                <span class="lbl">Seiten</span>
                <div class="seg">
                  {#each hud.graphic.plate_pages as _, p (p)}
                    <button class="num" class:on={platePage === p} onclick={() => ((platePage = p), slot.kind === "plate" || (slot = { kind: "plate", i: 0 }))}>{p + 1}</button>
                  {/each}
                </div>
                {#if hud.graphic.plate_pages.length < 5}
                  <button class="icon-btn" onclick={addPage} title="Seite hinzufügen" aria-label="Seite hinzufügen">
                    <svg width="16" height="16" viewBox="0 0 24 24"><path d="M12 5v14M5 12h14" /></svg>
                  </button>
                {/if}
                {#if hud.graphic.plate_pages.length > 1}
                  <button class="btn ghost small danger-quiet" onclick={removePage} title="Seite {platePage + 1} entfernen">
                    <svg width="14" height="14" viewBox="0 0 24 24" aria-hidden="true"><path d="M5 7h14M10 7V5h4v2M7 7l1 12h8l1-12" /></svg>
                    Seite {platePage + 1} entfernen
                  </button>
                {/if}
              </div>
              <div class="closeup">
                <SeatAnatomy
                  seat={sample}
                  {hud}
                  game={view.game}
                  currency={view.currency}
                  bountyCurrency={view.bounty_currency}
                  page={platePage}
                  selected={slot}
                  onSelect={(s) => (slot = s)}
                />
              </div>
              {#if slotStat}
                {@const m = STAT_META[slotStat]}
                {@const v = stat(sample.stats, slotStat)}
                {@const t = hud.thresholds[slotStat]}
                {@const k = tone(hud, v)}
                <div class="explain">
                  <p><b>{m.name}:</b> {m.help}</p>
                  <p class="example">
                    Beispiel {sample.name}: {m.plain}
                    <b class="num" style="color:{toneColor(hud, v, 'var(--text-2)')}">{fmtUnit(v)}</b>{m.after ? ` ${m.after}` : ""}
                    {#if k === "none"}
                      – noch keine Daten.
                    {:else}
                      → <b>{k === "mid" ? "normal" : MEANING[slotStat]?.[k === "low" ? 0 : 1]}</b>{#if t}{" "}<span class="faint">(normal bei dir: {t.low.toLocaleString("de-DE")}–{t.high.toLocaleString("de-DE")}{m.ratio ? "×" : " %"})</span>{/if}
                    {/if}
                  </p>
                </div>
              {/if}
              <div class="picker-head">
                <span>Anderer Wert für: {slotName}</span>
              </div>
              <div class="picker">
                {#each RATED_STATS as id (id)}
                  <button class="stat" class:on={slotStat === id} title={STAT_META[id].help} onclick={() => assign(id)}>
                    <b>{PICK_NAME[id] ?? STAT_META[id].name}</b>
                    {#if STAT_META[id].short !== (PICK_NAME[id] ?? STAT_META[id].name)}<i class="num">{STAT_META[id].short}</i>{/if}
                  </button>
                {/each}
              </div>
            </section>

            <section class="sec">
              <header class="sec-head"><h3>Namensschild von PokerStars</h3></header>
              <div class="covers">
                <button class="cover" class:on={hud.graphic.cover_plate} onclick={() => ((settings!.hud.graphic.cover_plate = true), changed())}>
                  <b>Überdecken</b>
                  <span>Name, Stack und Zahlen in einem Feld. Verdeckt aber „Check“, „Fold“ und den Zeitbalken.</span>
                </button>
                <button class="cover" class:on={!hud.graphic.cover_plate} onclick={() => ((settings!.hud.graphic.cover_plate = false), changed())}>
                  <b>Frei lassen</b>
                  <span>Schmale Zahlenleiste darunter; alles von PokerStars bleibt sichtbar.</span>
                </button>
              </div>
              {#if hud.graphic.cover_plate}
                <label class="slider">
                  <span>Breite</span>
                  <input type="range" min="0.8" max="1.5" step="0.02" bind:value={settings.hud.graphic.plate_width} oninput={changed} />
                  <em class="num">{Math.round(hud.graphic.plate_width * 100)} %</em>
                </label>
              {/if}
            </section>
          {:else if sample}
            <section class="sec">
              <header class="sec-head">
                <h3>Was in der Box steht</h3>
                <p>Für den Anfang ist „Klartext“ am leichtesten: Es schreibt jeden Wert als Satz aus.</p>
              </header>
              <div class="presets">
                {#each Object.entries(CLASSIC_PRESETS) as [id, p] (id)}
                  <button class="preset" class:on={hud.classic.preset === id} onclick={() => setPreset(id as ClassicPreset)}>
                    <span class="p-text"><b>{p.name}</b><span>{p.description}</span></span>
                    <span class="mini" aria-hidden="true">
                      <ClassicBox
                        seat={sample}
                        hud={{ ...hud, scale: 1, opacity: 1, classic: { ...hud.classic, preset: id as ClassicPreset } }}
                        ring={{ x: 0, y: 0 }}
                        box={{ x: 0, y: 0 }}
                        scale={0.92}
                        width={400}
                        height={300}
                        game={view.game}
                        currency={view.currency}
                        bountyCurrency={view.bounty_currency}
                        onMove={() => {}}
                        onReset={() => {}}
                        onTap={() => {}}
                      />
                    </span>
                  </button>
                {/each}
                <button class="preset" class:on={hud.classic.preset === "custom"} onclick={() => setPreset("custom")}>
                  <span class="p-text"><b>Eigene</b><span>Werte selbst wählen, vier pro Zeile</span></span>
                </button>
              </div>
              {#if hud.classic.preset === "custom"}
                <div class="picker">
                  {#each [...RATED_STATS, "hands" as StatId] as id (id)}
                    {@const n = hud.classic.custom.indexOf(id)}
                    <button class="stat" class:on={n >= 0} title={STAT_META[id].help} onclick={() => toggleCustom(id)}>
                      <b>{STAT_META[id].name}</b>
                      {#if n >= 0}<i class="num order">{n + 1}</i>{:else if STAT_META[id].short !== STAT_META[id].name}<i class="num">{STAT_META[id].short}</i>{/if}
                    </button>
                  {/each}
                </div>
                <p class="note">Reihenfolge wie angeklickt, höchstens acht Werte.</p>
              {/if}
            </section>
          {/if}

          <section class="sec">
            <header class="sec-head"><h3>Größe und Zusatzangaben</h3></header>
            <label class="slider">
              <span>Größe</span>
              <input type="range" min="0.6" max="1.6" step="0.05" bind:value={settings.hud.scale} oninput={changed} />
              <em class="num">{Math.round(hud.scale * 100)} %</em>
            </label>
            {#if hud.mode === "classic"}
              <label class="slider">
                <span>Deckkraft</span>
                <input type="range" min="0.5" max="1" step="0.01" bind:value={settings.hud.opacity} oninput={changed} />
                <em class="num">{Math.round(hud.opacity * 100)} %</em>
              </label>
            {/if}
            <label class="slider" title="Nach jeder Hand kurz die Karten aller Spieler und das Board zeigen.">
              <span>Rückblick</span>
              <input type="range" min="0" max="10" step="1" bind:value={settings.hud.recap_seconds} oninput={changed} />
              <em class="num">{hud.recap_seconds ? `${hud.recap_seconds} s` : "aus"}</em>
            </label>
            <div class="toggles">
              <label class="check">
                <input type="checkbox" bind:checked={settings.hud.show_stack} onchange={changed} />
                <span>Stack <small>Chips zu Beginn der letzten Hand</small></span>
              </label>
              <label class="check">
                <input type="checkbox" bind:checked={settings.hud.show_bounty} onchange={changed} />
                <span>Bounty <small>Kopfgeld bei Knockout-Turnieren</small></span>
              </label>
            </div>
            <details class="more">
              <summary>Zählweise</summary>
              <div class="toggles">
                <label class="check">
                  <input type="checkbox" bind:checked={settings.hud.hero_session_only} onchange={changed} />
                  <span>Eigene Werte nur aus diesem Turnier <small>Bei Cash Games: dieser Tisch heute</small></span>
                </label>
                <label class="check">
                  <input type="checkbox" bind:checked={settings.hud.split_by_game} onchange={changed} />
                  <span>Cash Games und Turniere getrennt zählen <small>Gegner spielen dort oft anders</small></span>
                </label>
              </div>
            </details>
          </section>
        {:else if pane === "colors"}
          <section class="sec">
            <header class="sec-head">
              <h3>Farben</h3>
              <p>Die Farbe zeigt auf einen Blick, ob ein Spieler etwas selten, normal oft oder oft tut – nicht, ob das gut oder schlecht ist.</p>
            </header>
            <div class="palettes">
              {#each Object.entries(PALETTES) as [id, p] (id)}
                <button class="palette" class:on={hud.palette === id} onclick={() => ((settings!.hud.palette = id as Palette), changed())}>
                  <span class="bar"><i style="background:{p.low}"></i><i style="background:{p.mid}"></i><i style="background:{p.high}"></i></span>
                  <b>{p.name}</b>
                  <span class="faint">niedrig · normal · hoch</span>
                </button>
              {/each}
            </div>
          </section>
          <section class="sec">
            <header class="sec-head">
              <h3>Ab wann die Farbe wechselt</h3>
              <p>Griffe ziehen. Die dunklen Striche sind die Spieler am Vorschautisch.</p>
            </header>
            <div class="scales">
              {#each RATED_STATS as id (id)}
                {@const t = settings.hud.thresholds[id]}
                {#if t}
                  <ThresholdScale {id} bind:t={settings.hud.thresholds[id]} name={PICK_NAME[id]} colors={pal} marks={marks(id)} {exact} meaning={MEANING[id]} onchange={changed} />
                {/if}
              {/each}
            </div>
            <label class="check exact">
              <input type="checkbox" bind:checked={exact} />
              <span>Genaue Zahlen eintippen</span>
            </label>
            <p class="note">Je Wert höchstens drei Farben – so verlangt es PokerStars für HUDs.</p>
          </section>
        {:else}
          <section class="sec">
            <header class="sec-head">
              <h3>Eigene Vorlagen</h3>
              <p>Eine Vorlage merkt sich Darstellung, Farben und alle Positionen – etwa eine für Turniere und eine für Cash Games.</p>
            </header>
            {#if settings.presets.length}
              <div class="saved">
                {#each settings.presets as p (p.name)}
                  {@const pp = PALETTES[p.hud.palette] ?? PALETTES.traffic}
                  <div class="saved-row">
                    <span class="swatch"><i style="background:{pp.low}"></i><i style="background:{pp.mid}"></i><i style="background:{pp.high}"></i></span>
                    <span class="saved-name">
                      <b>{p.name}</b>
                      <small>{p.hud.mode === "graphic" ? "Grafisch" : "Klassisch"} · {Math.round(p.hud.scale * 100)} %</small>
                    </span>
                    <button class="btn" onclick={() => applyPreset(p)}>Anwenden</button>
                    <button class="icon-btn danger" onclick={() => deletePreset(p)} title="Vorlage löschen" aria-label="Vorlage {p.name} löschen">
                      <svg width="15" height="15" viewBox="0 0 24 24"><path d="M5 7h14M10 7V5h4v2M7 7l1 12h8l1-12" /></svg>
                    </button>
                  </div>
                {/each}
              </div>
            {:else}
              <p class="empty-note">Noch keine Vorlage gespeichert.</p>
            {/if}
            <div class="save-row">
              <input class="field grow" placeholder="Name, z. B. Turniere" bind:value={presetName} />
              <button class="btn primary" onclick={savePreset}>Aktuelles HUD speichern</button>
            </div>
          </section>
          <section class="sec">
            <header class="sec-head">
              <h3>Als Datei</h3>
              <p>Sichern oder auf einen anderen Rechner mitnehmen.</p>
            </header>
            <div class="row">
              <button class="btn" onclick={exportProfile}>Exportieren</button>
              <button class="btn" onclick={importProfile}>Importieren</button>
              <button class="btn ghost danger push" onclick={resetProfile}>Alles auf Standard</button>
            </div>
          </section>
        {/if}
        {#if message}<p class="msg" class:bad={!message.ok}>{message.text}</p>{/if}
      </aside>
    </div>
  {/if}
</div>

<style>
  .page {
    padding-top: 20px;
  }
  .layout {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 384px;
    gap: 20px;
    align-items: start;
  }
  .stage-col {
    position: sticky;
    top: 16px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-width: 0;
  }
  .toolbar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px 20px;
  }
  .group,
  .pages {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .lbl {
    font-size: 12px;
    color: var(--text-3);
  }
  .seats button {
    min-width: 28px;
    padding: 0 6px;
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
  .stage-wrap {
    display: flex;
    justify-content: center;
  }
  .stage {
    position: relative;
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
    flex-direction: column;
    align-items: flex-start;
    gap: 10px;
  }
  .legend {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 16px;
    margin: 0;
    padding: 0;
    list-style: none;
    font-size: 12.5px;
    color: var(--text-2);
  }
  .legend li {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .gesture {
    font-weight: 600;
    color: var(--text);
  }
  .gesture.frame {
    padding: 0 5px;
    border: 1.5px solid var(--sd);
    border-radius: 4px;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
  .state {
    font-size: 12.5px;
    color: var(--text-2);
  }
  .state.custom {
    color: var(--text);
  }
  .shot-bar {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .crop {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 12.5px;
    color: var(--text-2);
  }
  .crop em {
    font-style: normal;
    min-width: 40px;
  }

  .inspector {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .tabs {
    display: flex;
    gap: 2px;
    padding: 0 12px;
    border-bottom: 1px solid var(--line);
  }
  .tabs button {
    position: relative;
    height: 44px;
    padding: 0 12px;
    border: 0;
    background: transparent;
    color: var(--text-2);
    font-size: 13.5px;
    font-weight: 500;
    cursor: pointer;
  }
  .tabs button:hover {
    color: var(--text);
  }
  .tabs button.on {
    color: var(--text);
  }
  .tabs button.on::after {
    content: "";
    position: absolute;
    left: 10px;
    right: 10px;
    bottom: -1px;
    height: 2px;
    border-radius: 2px;
    background: var(--text);
  }
  .sec {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 16px 18px;
  }
  .sec + .sec {
    border-top: 1px solid var(--line);
  }
  .sec-head h3 {
    margin: 0;
    font-size: 13px;
    font-weight: 600;
  }
  .sec-head p {
    margin: 3px 0 0;
    font-size: 13px;
    line-height: 1.45;
    color: var(--text-2);
  }

  .modes,
  .covers,
  .palettes {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }
  .mode,
  .cover,
  .palette,
  .preset {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 3px;
    padding: 10px 12px;
    border-radius: var(--r-sm);
    border: 1px solid var(--line-2);
    background: var(--bg);
    text-align: left;
    cursor: pointer;
    transition:
      border-color 0.12s,
      background 0.12s;
  }
  .mode:hover,
  .cover:hover,
  .palette:hover,
  .preset:hover {
    border-color: var(--text-3);
  }
  .mode.on,
  .cover.on,
  .palette.on,
  .preset.on {
    border-color: var(--text);
    background: var(--surface-2);
  }
  .mode svg {
    width: 100%;
    height: 44px;
    margin-bottom: 6px;
  }
  .m-plate {
    fill: var(--plate);
    stroke: rgba(255, 255, 255, 0.08);
  }
  .m-line {
    fill: #4a4f57;
  }
  .m-ring {
    fill: #1a1d22;
    stroke: #33373e;
    stroke-width: 4;
  }
  .m-seg {
    fill: none;
    stroke-width: 4;
  }
  .mode b,
  .cover b,
  .palette b,
  .preset b {
    font-size: 13px;
    font-weight: 600;
  }
  .mode span,
  .cover span,
  .preset .p-text span {
    font-size: 12px;
    line-height: 1.4;
    color: var(--text-2);
  }
  .mode.on span,
  .cover.on span,
  .preset.on .p-text span {
    color: var(--text-2);
  }

  .pages {
    flex-wrap: wrap;
  }
  .pages .seg button {
    min-width: 30px;
  }
  .danger-quiet:hover {
    color: #ff9d93;
  }
  .btn.small {
    height: 28px;
    padding: 0 9px;
    font-size: 12.5px;
  }
  .closeup {
    padding: 4px 0 2px;
    border-radius: var(--r-sm);
    background: radial-gradient(120% 140% at 70% 50%, rgba(46, 77, 58, 0.35), transparent 70%), var(--bg);
    border: 1px solid var(--line);
  }
  .picker-head {
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-3);
  }
  .picker {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 5px;
  }
  .stat {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
    min-height: 34px;
    padding: 5px 10px;
    border-radius: var(--r-sm);
    border: 1px solid var(--line);
    background: transparent;
    color: var(--text-2);
    text-align: left;
    cursor: pointer;
  }
  .stat b {
    font-size: 13px;
    line-height: 1.25;
    font-weight: 500;
  }
  .stat i {
    flex-shrink: 0;
    font-style: normal;
    font-size: 11.5px;
    color: var(--text-2);
  }
  .stat:hover {
    border-color: var(--line-2);
    color: var(--text);
  }
  .stat.on {
    border-color: var(--text);
    background: var(--surface-2);
    color: var(--text);
  }
  .stat .order {
    display: grid;
    place-items: center;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: var(--text);
    color: var(--bg);
    font-weight: 700;
  }
  .explain {
    margin: 0;
    padding: 9px 11px;
    border-radius: var(--r-sm);
    background: var(--surface-2);
    font-size: 13px;
    line-height: 1.5;
    color: var(--text-2);
  }
  .explain b {
    color: var(--text);
    font-weight: 600;
  }
  .explain p {
    margin: 0;
  }
  .example {
    margin-top: 6px !important;
    padding-top: 6px;
    border-top: 1px solid var(--line);
  }

  .presets {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .preset {
    gap: 8px;
  }
  .p-text {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .mini {
    display: block;
    max-width: 100%;
    overflow: hidden;
    pointer-events: none;
  }
  .mini :global(.box) {
    position: relative !important;
    display: inline-block;
    transform: none !important;
    box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.08) inset !important;
  }

  .slider {
    display: grid;
    grid-template-columns: 74px 1fr 46px;
    align-items: center;
    gap: 10px;
    font-size: 13px;
    color: var(--text-2);
  }
  .slider em {
    font-style: normal;
    text-align: right;
    color: var(--text);
  }
  .toggles {
    display: flex;
    flex-direction: column;
    gap: 10px;
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
  .more summary {
    cursor: pointer;
    font-size: 12.5px;
    color: var(--text-2);
  }
  .more summary:hover {
    color: var(--text);
  }
  .more[open] summary {
    margin-bottom: 10px;
  }

  .palette .bar {
    display: flex;
    width: 100%;
    height: 8px;
    margin-bottom: 6px;
    border-radius: 4px;
    overflow: hidden;
  }
  .palette .bar i {
    flex: 1;
  }
  .palette .faint {
    font-size: 12px;
  }
  .scales {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .note {
    margin: 0;
    font-size: 12px;
    line-height: 1.5;
    color: var(--text-3);
  }

  .saved {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .saved-row {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto auto;
    align-items: center;
    gap: 10px;
    padding: 8px 8px 8px 12px;
    border-radius: var(--r-sm);
    border: 1px solid var(--line);
    background: var(--bg);
  }
  .swatch {
    display: flex;
    gap: 2px;
  }
  .swatch i {
    width: 6px;
    height: 18px;
    border-radius: 2px;
  }
  .saved-name {
    display: flex;
    flex-direction: column;
    min-width: 0;
    font-size: 13px;
  }
  .saved-name b {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 600;
  }
  .empty-note {
    margin: 0;
    padding: 14px;
    border: 1px dashed var(--line-2);
    border-radius: var(--r-sm);
    text-align: center;
    font-size: 12.5px;
    color: var(--text-3);
  }
  .save-row {
    display: flex;
    gap: 8px;
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .push {
    margin-left: auto;
  }
  .danger {
    color: #ff9d93;
  }
  .icon-btn.danger {
    color: var(--text-3);
  }
  .icon-btn.danger:hover {
    color: #ff9d93;
  }
  .msg {
    margin: 0;
    padding: 0 18px 16px;
    font-size: 12.5px;
    color: var(--pos);
    overflow-wrap: anywhere;
  }
  .msg.bad {
    color: #ff9d93;
  }
  @media (min-width: 1900px) {
    .layout {
      grid-template-columns: minmax(0, 1fr) 460px;
    }
  }
  @media (max-width: 1240px) {
    .layout {
      grid-template-columns: 1fr;
    }
    /* Table, then the settings, then the how-to: the settings follow the table directly. */
    .stage-col {
      display: contents;
    }
    .inspector {
      order: 1;
    }
    .stage-foot,
    .shot-bar {
      order: 2;
    }
    .mode {
      display: grid;
      grid-template-columns: 110px 1fr;
      column-gap: 12px;
      align-items: center;
    }
    .mode svg {
      grid-row: span 2;
      margin: 0;
    }
    .stage {
      width: 100% !important;
    }
  }
</style>
