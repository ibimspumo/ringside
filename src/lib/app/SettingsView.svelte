<script lang="ts">
  import { onMount } from "svelte";
  import { api, inTauri } from "../api";
  import type { ImportStatus, Settings } from "../types";

  let { status }: { status: ImportStatus | null } = $props();

  const REPO = "https://github.com/ibimspumo/ringside";

  let settings = $state<Settings | null>(null);
  let detected = $state<string[]>([]);
  let version = $state("dev");
  let update = $state<{ state: "idle" | "checking" | "none" | "available" | "installing" | "error"; text?: string }>({
    state: "idle",
  });
  let pending: import("@tauri-apps/plugin-updater").Update | null = null;

  onMount(async () => {
    settings = await api.getSettings();
    detected = await api.detectedDirs();
    if (inTauri) version = await (await import("@tauri-apps/api/app")).getVersion();
  });

  function save() {
    if (settings) api.saveSettings($state.snapshot(settings));
  }

  async function addDir() {
    if (!settings || !inTauri) return;
    const { open } = await import("@tauri-apps/plugin-dialog");
    const dir = await open({ directory: true, title: "Hand-History-Ordner wählen" });
    if (typeof dir === "string" && !settings.hand_history_dirs.includes(dir)) {
      settings.hand_history_dirs = [...settings.hand_history_dirs, dir];
      save();
    }
  }

  function removeDir(dir: string) {
    if (!settings) return;
    settings.hand_history_dirs = settings.hand_history_dirs.filter((d) => d !== dir);
    save();
  }

  async function checkUpdate() {
    if (!inTauri) return;
    update = { state: "checking" };
    try {
      const { check } = await import("@tauri-apps/plugin-updater");
      pending = await check();
      update = pending ? { state: "available", text: pending.version } : { state: "none" };
    } catch (e) {
      update = { state: "error", text: String(e) };
    }
  }

  async function install() {
    if (!pending) return;
    update = { state: "installing" };
    try {
      await pending.downloadAndInstall();
      const { relaunch } = await import("@tauri-apps/plugin-process");
      await relaunch();
    } catch (e) {
      update = { state: "error", text: String(e) };
    }
  }

  async function openRepo() {
    if (!inTauri) return window.open(REPO);
    const { openUrl } = await import("@tauri-apps/plugin-opener");
    openUrl(REPO);
  }
</script>

<div class="page">
  <div class="page-head">
    <div>
      <h1>Einstellungen</h1>
      <p>Ringside arbeitet komplett lokal. Ins Internet geht es nur, wenn du nach Updates suchst.</p>
    </div>
  </div>

  {#if settings}
    <div class="grid">
      <section class="panel">
        <h2 class="panel-title">Hand-History-Ordner</h2>
        <div class="body">
          <p class="hint">
            In PokerStars unter <b>Einstellungen → Spielhistorie</b> das Speichern der Hand-Histories einschalten.
            {settings.hand_history_dirs.length === 0 ? "Ringside sucht die Ordner automatisch:" : ""}
          </p>
          <ul class="dirs">
            {#each settings.hand_history_dirs.length ? settings.hand_history_dirs : detected as dir (dir)}
              <li>
                <code>{dir}</code>
                {#if settings.hand_history_dirs.length}
                  <button class="x" onclick={() => removeDir(dir)} title="Entfernen">✕</button>
                {:else}
                  <span class="tag">automatisch</span>
                {/if}
              </li>
            {:else}
              <li class="missing">Kein Ordner gefunden – bitte manuell hinzufügen.</li>
            {/each}
          </ul>
          <div class="row">
            <button class="btn" onclick={addDir}>Ordner hinzufügen</button>
          </div>
          {#if status}
            <p class="stat num">
              {status.hands_imported.toLocaleString("de-DE")} Hände importiert · {status.files_read} Dateien gelesen
              {#if status.malformed}· {status.malformed} nicht lesbar{/if}
            </p>
            {#if status.last_error}<p class="error">{status.last_error}</p>{/if}
          {/if}
        </div>
      </section>

      <section class="panel">
        <h2 class="panel-title">Tische</h2>
        <div class="body">
          <label class="check">
            <input type="checkbox" bind:checked={settings.preferred_seating} onchange={save} />
            <span>
              Bevorzugter Sitzplatz ist in PokerStars aktiv
              <small>Dann sitzt du immer unten in der Mitte und das HUD dreht sich mit.</small>
            </span>
          </label>
          <label class="stack">
            <span>Fensterklasse der Tische <small>(nur ändern, wenn keine Tische erkannt werden)</small></span>
            <input
              class="field"
              value={settings.table_window_classes.join(", ")}
              onchange={(e) => {
                settings!.table_window_classes = e.currentTarget.value
                  .split(",")
                  .map((s) => s.trim())
                  .filter(Boolean);
                save();
              }}
            />
          </label>
        </div>
      </section>

      <section class="panel">
        <h2 class="panel-title">Updates</h2>
        <div class="body">
          <p class="version">Version <b class="num">{version}</b></p>
          <div class="row">
            {#if update.state === "available"}
              <button class="btn primary" onclick={install}>Version {update.text} installieren</button>
            {:else}
              <button class="btn" onclick={checkUpdate} disabled={update.state === "checking" || update.state === "installing"}>
                {update.state === "checking" ? "Suche …" : "Nach Updates suchen"}
              </button>
            {/if}
            {#if update.state === "none"}<span class="ok">Du bist auf dem neuesten Stand.</span>{/if}
            {#if update.state === "installing"}<span>Wird installiert, Ringside startet gleich neu …</span>{/if}
          </div>
          {#if update.state === "error"}<p class="error">{update.text}</p>{/if}
        </div>
      </section>

      <section class="panel">
        <h2 class="panel-title">Über Ringside</h2>
        <div class="body">
          <p class="hint">
            Open Source unter MIT-Lizenz. Ringside liest nur deine eigenen Hand-Histories und zeigt einfache
            Zahlen-Statistiken an – kein Data-Mining, keine Echtzeit-Ratschläge.
          </p>
          <div class="row"><button class="btn" onclick={openRepo}>Projekt auf GitHub</button></div>
        </div>
      </section>
    </div>
  {/if}
</div>

<style>
  .grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 14px;
    align-items: start;
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 12px 18px 18px;
  }
  .hint {
    margin: 0;
    color: var(--text-dim);
    line-height: 1.5;
  }
  .dirs {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .dirs li {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    border-radius: 8px;
    background: var(--bg-raised);
    border: 1px solid var(--line);
  }
  code {
    flex: 1;
    font-size: 12.5px;
    overflow-wrap: anywhere;
  }
  .tag {
    font-size: 11px;
    color: var(--text-faint);
  }
  .x {
    border: 0;
    background: transparent;
    color: var(--text-faint);
    cursor: pointer;
  }
  .missing {
    color: #ffb19f;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .stat {
    margin: 0;
    font-size: 13px;
    color: var(--text-faint);
  }
  .error {
    margin: 0;
    color: #ff9f8f;
    font-size: 12.5px;
    overflow-wrap: anywhere;
  }
  .check {
    display: flex;
    gap: 10px;
    align-items: flex-start;
  }
  .check input {
    margin-top: 3px;
    accent-color: var(--brand);
  }
  small {
    display: block;
    color: var(--text-faint);
    font-size: 12px;
    margin-top: 2px;
  }
  .stack {
    display: flex;
    flex-direction: column;
    gap: 6px;
    color: var(--text-dim);
  }
  .version {
    margin: 0;
    color: var(--text-dim);
  }
  .ok {
    color: var(--win);
  }
</style>
