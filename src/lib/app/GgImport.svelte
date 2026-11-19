<script lang="ts">
  import { onMount } from "svelte";
  import { api, inTauri } from "../api";
  import Button from "../ui/Button.svelte";
  import IconButton from "../ui/IconButton.svelte";
  import SettingRow from "../ui/SettingRow.svelte";
  import SiteMark from "../ui/SiteMark.svelte";
  import SourceNote from "../ui/SourceNote.svelte";
  import type { ImportReport, ImportStatus, Settings } from "../types";
  import { int } from "./format";

  /** GGPoker keeps no hand history folder: hands come from PokerCraft downloads, dropped, picked or watched. */
  let { settings, status, onsave }: { settings: Settings; status: ImportStatus | null; onsave: () => void } = $props();

  let over = $state(false);
  let busy = $state(false);
  let report = $state<ImportReport | null>(null);
  let failed = $state<string | null>(null);
  const stored = $derived(status?.sites?.ggpoker ?? 0);

  async function run(paths: string[]) {
    if (!paths.length || busy) return;
    busy = true;
    failed = null;
    try {
      report = await api.importHandFiles(paths);
    } catch (e) {
      failed = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }

  async function pick() {
    if (!inTauri) return run(["PokerCraft.zip"]);
    const { open } = await import("@tauri-apps/plugin-dialog");
    const picked = await open({
      multiple: true,
      title: "GGPoker-Hände importieren",
      filters: [{ name: "PokerCraft-Download", extensions: ["zip", "txt"] }],
    });
    if (picked) run(Array.isArray(picked) ? picked : [picked]);
  }

  async function watchDir() {
    if (!inTauri) return;
    const { open } = await import("@tauri-apps/plugin-dialog");
    const dir = await open({ directory: true, title: "Ordner für GGPoker-Downloads wählen" });
    if (typeof dir === "string" && !settings.gg_dirs.includes(dir)) {
      settings.gg_dirs = [...settings.gg_dirs, dir];
      onsave();
    }
  }

  function unwatch(dir: string) {
    settings.gg_dirs = settings.gg_dirs.filter((d) => d !== dir);
    onsave();
  }

  // In the app the window hands over dropped files with their paths; the browser preview only plays it.
  onMount(() => {
    if (!inTauri) return;
    let stop: (() => void) | undefined;
    let live = true;
    import("@tauri-apps/api/webview").then(({ getCurrentWebview }) =>
      getCurrentWebview()
        .onDragDropEvent((e) => {
          if (e.payload.type === "over" || e.payload.type === "enter") over = true;
          else if (e.payload.type === "leave") over = false;
          else if (e.payload.type === "drop") {
            over = false;
            run(e.payload.paths);
          }
        })
        .then((f) => (live ? (stop = f) : f())),
    );
    return () => {
      live = false;
      stop?.();
    };
  });

  function line(r: ImportReport): string {
    const parts = [r.new_hands === 1 ? "1 neue Hand" : `${int(r.new_hands)} neue Hände`];
    if (r.duplicates) parts.push(r.duplicates === 1 ? "1 war schon da" : `${int(r.duplicates)} waren schon da`);
    if (r.tournaments) parts.push(r.tournaments === 1 ? "1 Turnier-Ergebnis" : `${int(r.tournaments)} Turnier-Ergebnisse`);
    if (r.malformed) parts.push(`${int(r.malformed)} unlesbar`);
    return parts.join(" · ");
  }
</script>

{#snippet more()}
  <p>
    In PokerCraft (im GGPoker-Client) lädst du deine Handverläufe der letzten 90 Tage als ZIP herunter. Ringside liest Hände und
    Turnier-Ergebnisse daraus; doppelte Hände zählt es nur einmal, du kannst also jeden Download ganz importieren.
  </p>
  <p>
    Gegner heißen bei GGPoker nur Kennungen, die von Session zu Session wechseln. Deshalb gibt es für sie keine Spielerseite und kein
    HUD – deine eigenen Hände wertet Ringside voll aus.
  </p>
{/snippet}

<SettingRow
  title="GGPoker-Hände"
  sub="GGPoker legt keine Hände auf deinem PC ab. Lade sie in PokerCraft herunter und zieh die Datei hierher."
  {more}
>
  <div
    class="drop"
    class:over
    role="region"
    aria-label="PokerCraft-Download hier ablegen"
    ondragover={(e) => (e.preventDefault(), (over = true))}
    ondragleave={() => (over = false)}
    ondrop={(e) => {
      e.preventDefault();
      over = false;
      if (!inTauri) run([...(e.dataTransfer?.files ?? [])].map((f) => f.name));
    }}
  >
    <SiteMark site="ggpoker" size="md" />
    <span class="drop-text">
      <b>{busy ? "Liest die Hände …" : over ? "Loslassen zum Importieren" : "PokerCraft-Download hier ablegen"}</b>
      <span>ZIP oder .txt – Hände und Turnier-Ergebnisse</span>
    </span>
    <Button size="sm" disabled={busy} onclick={pick}>oder Datei wählen</Button>
  </div>
  {#if failed}
    <p class="result bad"><span class="dot bad"></span>Import fehlgeschlagen: {failed}</p>
  {:else if report}
    <p class="result"><span class="dot on"></span>{line(report)}</p>
  {/if}
  {#if settings.gg_dirs.length}
    <ul class="files">
      {#each settings.gg_dirs as dir (dir)}
        <li>
          <svg class="ico" viewBox="0 0 16 16" aria-hidden="true"><path d="M2 4.5h4l1.5 1.5H14v6.5H2z" /></svg>
          <code title={dir}>{dir}</code>
          <span class="watch">wird beobachtet</span>
          <IconButton label="Ordner nicht mehr beobachten" size="sm" onclick={() => unwatch(dir)}
            ><svg viewBox="0 0 16 16" aria-hidden="true"><path d="M4 4l8 8M12 4l-8 8" /></svg></IconButton
          >
        </li>
      {/each}
    </ul>
  {/if}
  <div class="foot">
    <span class="count"><span class="dot" class:off={!stored}></span><span><span class="num">{int(stored)}</span> GGPoker-Hände gespeichert</span></span>
    <Button size="sm" onclick={watchDir}>
      {settings.gg_dirs.length ? "Weiteren Ordner beobachten" : "Download-Ordner beobachten"}
    </Button>
  </div>
  <div class="rule">
    <SourceNote kind="locked" variant="block">
      GGPoker erlaubt seit 15.10.2026 keine Poker-Programme neben dem Client. Schließ Ringside ganz, bevor du GGPoker startest, und importier
      deine Hände danach.
    </SourceNote>
  </div>
</SettingRow>

<style>
  .drop {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding: var(--sp-4) var(--sp-5);
    border: 1.5px dashed color-mix(in srgb, var(--site-gg) 45%, var(--line-2));
    border-radius: var(--r-md);
    background: var(--surface-sunken);
    transition:
      border-color var(--dur-fast) var(--ease-standard),
      background var(--dur-fast) var(--ease-standard);
  }
  .drop.over {
    border-color: var(--site-gg);
    background: color-mix(in srgb, var(--site-gg) 8%, var(--surface-sunken));
  }
  .drop-text {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: var(--fs-md);
  }
  .drop-text span {
    color: var(--text-2);
    font-size: var(--fs-sm);
  }
  .result,
  .count {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    margin: var(--sp-2) 0 0;
    color: var(--text-2);
    font-size: var(--fs-sm);
  }
  .result.bad {
    color: var(--danger-text);
  }
  .count {
    margin: 0;
    font-size: var(--fs-base);
  }
  .dot.off {
    background: var(--text-3);
  }
  .dot {
    width: 8px;
    height: 8px;
    flex: none;
    border-radius: 50%;
    background: var(--pos);
  }
  .dot.bad {
    background: var(--neg);
  }
  .files {
    list-style: none;
    margin: var(--sp-3) 0 0;
    padding: 0;
    border: 1px solid var(--line);
    border-radius: var(--r-sm);
    background: var(--surface-sunken);
  }
  .files li {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    min-height: 40px;
    padding: var(--sp-2) var(--sp-3);
  }
  .files li + li {
    border-top: 1px solid var(--line);
  }
  code {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--fs-md);
  }
  .watch {
    flex: none;
    color: var(--text-2);
    font-size: var(--fs-sm);
  }
  .ico {
    flex: none;
    width: 16px;
    height: 16px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.3;
    stroke-linejoin: round;
    stroke-linecap: round;
    color: var(--text-3);
  }
  .foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-3);
    margin-top: var(--sp-3);
  }
  .rule {
    margin-top: var(--sp-3);
  }
</style>
