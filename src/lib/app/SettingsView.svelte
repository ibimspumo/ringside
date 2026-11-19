<script lang="ts">
  import { onMount } from "svelte";
  import { api, inTauri, on } from "../api";
  import AutoAnalysisSettings from "../analysis/AutoAnalysisSettings.svelte";
  import CloudSettings from "../cloud/CloudSettings.svelte";
  import LibrarySettings from "../library/LibrarySettings.svelte";
  import MalformedReports from "../cloud/MalformedReports.svelte";
  import CoachSettings from "../coach/CoachSettings.svelte";
  import { COACH_INTRO } from "../coach/api";
  import { modelLabel } from "../coach/meaning";
  import type { CoachStatus } from "../coach/types";
  import type { CloudSettings as CloudChoice } from "../cloud/types";
  import Card from "../ui/Card.svelte";
  import Button from "../ui/Button.svelte";
  import EmptyState from "../ui/EmptyState.svelte";
  import IconButton from "../ui/IconButton.svelte";
  import Panel from "../ui/Panel.svelte";
  import ProgressBar from "../ui/ProgressBar.svelte";
  import SectionHead from "../ui/SectionHead.svelte";
  import SettingRow from "../ui/SettingRow.svelte";
  import Switch from "../ui/Switch.svelte";
  import Tooltip from "../ui/Tooltip.svelte";
  import { usePageHead } from "../ui/page.svelte";
  import type { ClientWindow, ImportStatus, NotesSource, Settings } from "../types";

  let { status }: { status: ImportStatus | null } = $props();

  const REPO = "https://github.com/ibimspumo/ringside";

  const GROUPS = [
    { id: "s-pokerstars", label: "Hände & Tische", sub: "Wo Ringside deine Hände findet und auf welchen Tischen das HUD liegt." },
    { id: "s-rechnen", label: "Analyse & Rechnen", sub: "Was dein PC für dich rechnet, und wann." },
    { id: "s-cloud", label: "Ringside Cloud", sub: "Fertige Lösungen holen und eigene teilen. Deine Hände bleiben hier." },
    { id: "s-coach", label: "KI-Coach", sub: COACH_INTRO },
    { id: "s-updates", label: "Updates & Info", sub: "Version, Updates und woher Ringside kommt." },
  ] as const;
  type GroupId = (typeof GROUPS)[number]["id"];

  let active = $state<GroupId>(GROUPS[0].id);
  let page = $state<HTMLElement>();
  /** While a jump scrolls the page, the scroll position says nothing about the chosen group. */
  let jumping = false;

  /** Wide windows show the groups in the overview column beside the page instead of as a tab row. */
  let wide = $state(false);

  usePageHead(() => ({
    tabs: wide ? [] : GROUPS.map((g) => ({ id: g.id, label: g.label })),
    value: active,
    label: "Gruppen der Einstellungen",
    jump: true,
    onchange: (id) => jump(id as GroupId),
  }));

  const scroller = () => page?.closest<HTMLElement>(".content") ?? null;

  function jump(id: GroupId) {
    active = id;
    const el = document.getElementById(id);
    const box = scroller();
    if (!el || !box) return;
    jumping = true;
    const smooth = !matchMedia("(prefers-reduced-motion: reduce)").matches;
    box.scrollTo({ top: el.offsetTop - (page?.offsetTop ?? 0), behavior: smooth ? "smooth" : "auto" });
    const done = () => (jumping = false);
    box.addEventListener("scrollend", done, { once: true });
    setTimeout(done, 900);
  }

  $effect(() => {
    const box = scroller();
    if (!box) return;
    const size = new ResizeObserver(() => (wide = box.clientWidth >= 1280));
    size.observe(box);
    return () => size.disconnect();
  });

  $effect(() => {
    const box = scroller();
    if (!box) return;
    const spy = () => {
      if (jumping) return;
      const top = box.getBoundingClientRect().top + 96;
      let id: GroupId = GROUPS[0].id;
      for (const g of GROUPS) {
        const el = document.getElementById(g.id);
        if (el && el.getBoundingClientRect().top <= top) id = g.id;
      }
      if (box.scrollTop > 0 && box.scrollTop + box.clientHeight >= box.scrollHeight - 2) id = GROUPS[GROUPS.length - 1].id;
      active = id;
    };
    box.addEventListener("scroll", spy, { passive: true });
    return () => box.removeEventListener("scroll", spy);
  });

  let settings = $state<Settings | null>(null);
  let detected = $state<string[]>([]);
  let windows = $state<ClientWindow[] | null>(null);
  let notes = $state<NotesSource[] | null>(null);
  let clientRunning = $state(false);
  let othersOpen = $state(false);

  async function loadWindows() {
    windows = await api.clientWindows();
  }
  let version = $state("dev");
  type UpdateState = { state: "idle" | "checking" | "none" | "available" | "installing" | "error"; text?: string };
  const previewParam = !inTauri ? new URLSearchParams(location.search).get("update") : null;
  let update = $state<UpdateState>(previewUpdate());
  /** Bytes of the update so far and in total (total unknown until the server says it). */
  let download = $state<{ done: number; total: number | null } | null>(
    previewParam === "download" ? { done: 12.4e6, total: 43.6e6 } : null,
  );

  /** Browser demo only: `?update=checking|none|available|download|error` shows that state. */
  function previewUpdate(): UpdateState {
    switch (previewParam) {
      case "download":
        return { state: "installing" };
      case "available":
        return { state: "available", text: "0.5.0" };
      case "checking":
      case "none":
        return { state: previewParam };
      case "error":
        return { state: "error", text: "Der Update-Server hat nicht geantwortet. Prüf deine Internetverbindung und versuch es gleich noch einmal." };
      default:
        return { state: "idle" };
    }
  }

  const mb = (bytes: number) => (bytes / 1e6).toLocaleString("de-DE", { minimumFractionDigits: 1, maximumFractionDigits: 1 });
  let pending: import("@tauri-apps/plugin-updater").Update | null = null;

  const loadNotes = () => api.notesSources().then((n) => (notes = n));

  onMount(() => {
    const subs = [on("notes-changed", loadNotes), on<boolean>("poker-client", (r) => (clientRunning = r))];
    return () => {
      for (const sub of subs) sub.then((f) => f());
    };
  });

  onMount(async () => {
    api.pokerClientRunning().then((r) => (clientRunning = r));
    settings = await api.getSettings();
    detected = await api.detectedDirs();
    loadWindows();
    loadNotes();
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
    download = { done: 0, total: null };
    try {
      await pending.downloadAndInstall((event) => {
        if (!download) return;
        if (event.event === "Started") download = { done: 0, total: event.data.contentLength ?? null };
        else if (event.event === "Progress") download = { ...download, done: download.done + event.data.chunkLength };
        else download = null;
      });
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

  const dirs = $derived(settings ? (settings.hand_history_dirs.length ? settings.hand_history_dirs : detected) : []);
  const manual = $derived(!!settings?.hand_history_dirs.length);
  const tables = $derived((windows ?? []).filter((w) => w.table_key));
  const others = $derived((windows ?? []).filter((w) => !w.table_key));

  /**
   * "$5.50 Mini Early Battle - 50/100 Ante 12 - Turnier 4046789300 Tisch 10 - Eingeloggt als Hero":
   * the name first, the rest as a quiet line, without the login part every title carries.
   */
  function tableName(title: string) {
    const parts = title
      .split(" - ")
      .map((p) => p.trim())
      .filter((p) => p && !/^(Eingeloggt als|Logged in as)\b/i.test(p));
    // "Turnier 4046789300 Tisch 10": the table number tells tables apart, the tournament number only in the tooltip.
    const short = parts.slice(1).map((p) => p.replace(/^(?:Turnier|Tournament)\s+\d+\s+/i, ""));
    return { name: parts[0] || "(ohne Titel)", rest: short.join(" · "), full: parts.join(" · ") };
  }
  const int = (n: number) => n.toLocaleString("de-DE");

  let auto = $state<boolean | null>(null);
  let cloud = $state<CloudChoice | null>(null);
  let coach = $state<CoachStatus | null>(null);

  type Tone = "ok" | "off" | "wait" | "bad";
  /** One line per group for the overview column: how it stands, without opening it. */
  const overview = $derived<Record<GroupId, { tone: Tone; line: string }>>({
    "s-pokerstars": dirs.length
      ? {
          tone: "ok",
          line: `${int(status?.total_hands ?? 0)} Hände · ${tables.length === 1 ? "1 Tisch" : `${tables.length} Tische`} erkannt`,
        }
      : { tone: "bad", line: "Kein Hand-History-Ordner gefunden" },
    "s-rechnen": clientRunning
      ? { tone: "wait", line: "Pausiert, solange PokerStars läuft" }
      : { tone: auto ? "ok" : "off", line: auto === null ? "…" : auto ? "Neue Hände werden automatisch analysiert" : "Analyse nur, wenn du sie startest" },
    "s-cloud": !cloud
      ? { tone: "off", line: "…" }
      : cloud.fetch || cloud.share
        ? { tone: "ok", line: cloud.fetch && cloud.share ? "Lösungen abrufen und teilen" : cloud.fetch ? "Lösungen nur abrufen" : "Lösungen nur teilen" }
        : { tone: "off", line: "Aus" },
    "s-coach": !coach
      ? { tone: "off", line: "…" }
      : coach.has_key
        ? { tone: "ok", line: `Bereit · ${modelLabel(coach.model)}` }
        : { tone: "off", line: "Noch kein Key – der Coach ist freiwillig" },
    "s-updates":
      update.state === "available"
        ? { tone: "wait", line: `Version ${update.text} ist da` }
        : update.state === "installing"
          ? { tone: "wait", line: download?.total ? `Update lädt · ${Math.round((download.done / download.total) * 100)} %` : "Update wird installiert" }
          : update.state === "error"
            ? { tone: "bad", line: "Suche nach Updates fehlgeschlagen" }
            : { tone: update.state === "none" ? "ok" : "off", line: update.state === "none" ? `Version ${version} – aktuell` : `Version ${version}` },
  });

  /** Which table names are cut off: only those get the full title as a tooltip. */
  let cut = $state<Record<number, boolean>>({});
  function clipped(node: HTMLElement, report: (cut: boolean) => void) {
    const check = () => report([node, ...node.children].some((e) => e.scrollWidth > e.clientWidth + 1));
    const watch = new ResizeObserver(check);
    watch.observe(node);
    check();
    return { destroy: () => watch.disconnect() };
  }
</script>

{#snippet dirsMore()}
  <p>
    Das Speichern schaltest du in PokerStars unter <b>Einstellungen → Spielhistorie</b> ein. Ringside findet die üblichen
    Ordner selbst; liegt deiner woanders, füg ihn hinzu.
  </p>
{/snippet}
{#snippet tableLabel(t: { name: string; rest: string }, i: number)}
  <span class="tname" use:clipped={(v) => (cut[i] = v)}><b>{t.name}</b>{#if t.rest}<span class="rest">{t.rest}</span>{/if}</span>
{/snippet}
{#snippet folderIcon()}
  <svg class="ico" viewBox="0 0 16 16" aria-hidden="true"><path d="M2 4.5h4l1.5 1.5H14v6.5H2z" /></svg>
{/snippet}
{#snippet fileIcon()}
  <svg class="ico" viewBox="0 0 16 16" aria-hidden="true"><path d="M4 2h5.5L12 4.5V14H4zM9.5 2v2.5H12M6 8h4M6 10.5h4" /></svg>
{/snippet}

<div class="page settings" class:wide bind:this={page}>
  {#if settings && wide}
    <aside class="overview" aria-label="Einstellungen auf einen Blick">
      <p class="ov-title">Auf einen Blick</p>
      {#each GROUPS as g (g.id)}
        {@const o = overview[g.id]}
        <Card pad="sm" tone={active === g.id ? "raised" : "default"} onclick={() => jump(g.id)} aria-current={active === g.id ? "location" : undefined}>
          <span class="ov-item">
            <b>{g.label}</b>
            <span class="ov-line"><span class="dot {o.tone}"></span>{o.line}</span>
          </span>
        </Card>
      {/each}
    </aside>
  {/if}
  {#if settings}
  <div class="groups">
    <section class="group" id="s-pokerstars" aria-labelledby="s-pokerstars-h">
      {@render head(0)}
      <Panel flush>
        <SettingRow
          title="Hand-History-Ordner"
          sub={dirs.length
            ? "Hier legt PokerStars jede gespielte Hand ab. Ringside liest neue Hände sofort mit."
            : "Hier legt PokerStars jede gespielte Hand ab, sobald das Speichern eingeschaltet ist."}
          more={dirs.length ? dirsMore : undefined}
        >
          {#snippet control()}
            {#if dirs.length}<Button size="sm" onclick={addDir}>Ordner hinzufügen</Button>{/if}
          {/snippet}
          {#if dirs.length}
            <ul class="files">
              {#each dirs as dir (dir)}
                <li>
                  {@render folderIcon()}
                  <code title={dir}>{dir}</code>
                  {#if manual}
                    <IconButton label="Ordner entfernen" size="sm" onclick={() => removeDir(dir)}
                      ><svg viewBox="0 0 16 16" aria-hidden="true"><path d="M4 4l8 8M12 4l-8 8" /></svg></IconButton
                    >
                  {:else}
                    <span class="found"
                      ><svg viewBox="0 0 16 16" aria-hidden="true"><path d="M3.5 8.5l3 3 6-7" /></svg>automatisch gefunden</span
                    >
                  {/if}
                </li>
              {/each}
            </ul>
            {#if status}
              <p class="meta">
                <span class="dot on"></span><span><span class="num">{int(status.total_hands)}</span> Hände gespeichert · <span
                    class="num">{int(status.hands_imported)}</span
                  > neu seit dem Start</span>
              </p>
            {/if}
          {:else}
            <EmptyState
              inline
              tone="error"
              title="Kein Ordner gefunden"
              text="Schalte in PokerStars unter Einstellungen → Spielhistorie das Speichern der Hände ein. Liegt der Ordner woanders, wähl ihn selbst."
            >
              {#snippet action()}<Button variant="primary" size="sm" onclick={addDir}>Ordner wählen</Button>{/snippet}
            </EmptyState>
          {/if}
          {#if status?.last_error}<p class="error">{status.last_error}</p>{/if}
        </SettingRow>

        {#if status}<MalformedReports count={status.malformed} />{/if}

        <SettingRow
          title="Erkannte Tische"
          sub={tables.length === 1 ? "Auf diesem PokerStars-Tisch liegt das HUD." : "Auf diesen PokerStars-Tischen liegt das HUD."}
        >
          {#snippet control()}
            <Button size="sm" variant="quiet" onclick={loadWindows}>
              {#snippet icon()}<svg class="ico" viewBox="0 0 16 16" aria-hidden="true"><path d="M13 8a5 5 0 1 1-1.5-3.5M13 2.5V5h-2.5" /></svg>{/snippet}
              Aktualisieren
            </Button>
          {/snippet}
          {#if windows === null}
            <p class="meta">Sucht PokerStars-Fenster …</p>
          {:else if windows.length === 0}
            <EmptyState inline title="Kein PokerStars-Fenster offen" text="Sobald du an einem Tisch sitzt, erscheint er hier." />
          {:else}
            <ul class="tables">
              {#each tables as w, i (i)}
                {@const t = tableName(w.title)}
                <li>
                  <span class="dot" class:on={w.has_hands} class:wait={!w.has_hands}></span>
                  {#if cut[i]}
                    <Tooltip text={t.full}>{@render tableLabel(t, i)}</Tooltip>
                  {:else}
                    {@render tableLabel(t, i)}
                  {/if}
                  <span class="tstate" class:wait={!w.has_hands}>{w.has_hands ? "HUD zeigt Werte" : "HUD füllt sich nach der ersten Hand"}</span>
                </li>
              {:else}
                <li class="none">Kein Tisch offen – PokerStars zeigt gerade nur Lobby-Fenster.</li>
              {/each}
            </ul>
            {#if others.length}
              <div class="others">
                <Button size="sm" variant="quiet" aria-expanded={othersOpen} onclick={() => (othersOpen = !othersOpen)}>
                  {#snippet icon()}<svg class="chev" class:open={othersOpen} viewBox="0 0 12 12" aria-hidden="true"><path d="M3 4.5l3 3 3-3" /></svg>{/snippet}
                  {others.length === 1 ? "1 weiteres PokerStars-Fenster" : `${others.length} weitere PokerStars-Fenster`} (Lobby, kein Tisch)
                </Button>
              </div>
              {#if othersOpen}
                <ul class="other-list">
                  {#each others as w, i (i)}<li title={w.title}>{w.title || "(ohne Titel)"}</li>{/each}
                </ul>
              {/if}
            {/if}
          {/if}
        </SettingRow>

        <SettingRow
          title="Bevorzugter Sitzplatz"
          sub="Schalt das ein, wenn er in PokerStars aktiv ist: Dann sitzt du immer unten in der Mitte und das HUD dreht sich mit."
        >
          {#snippet control()}
            <Switch label="Bevorzugter Sitzplatz ist in PokerStars aktiv" bind:checked={settings!.preferred_seating} onchange={save} />
          {/snippet}
        </SettingRow>

        <SettingRow
          title="Spieler-Notizen"
          sub={notes?.length
            ? "Deine Notizen und Farb-Labels aus PokerStars erscheinen im HUD."
            : "Keine Notizdatei gefunden. PokerStars legt sie an, sobald du einem Spieler eine Notiz oder ein Farb-Label gibst."}
        >
          {#if notes === null}
            <p class="meta">Lädt …</p>
          {:else if notes.length}
            <ul class="files">
              {#each notes as n (n.path)}
                <li>
                  {@render fileIcon()}
                  <code title={n.path}>{n.path}</code>
                  {#if n.error}<span class="error">nicht lesbar ({n.error})</span>{:else}<span class="count"
                      ><span class="num">{int(n.players)}</span> Spieler mit Notiz</span
                    >{/if}
                </li>
              {/each}
            </ul>
          {/if}
        </SettingRow>
      </Panel>
    </section>

    <section class="group" id="s-rechnen" aria-labelledby="s-rechnen-h">
      {@render head(1)}
      <Panel flush>
        <AutoAnalysisSettings onChange={(a) => (auto = a)} />
        <LibrarySettings />
      </Panel>
    </section>

    <section class="group" id="s-cloud" aria-labelledby="s-cloud-h">
      {@render head(2)}
      <Panel flush><CloudSettings onChange={(c) => (cloud = c)} /></Panel>
    </section>

    <section class="group" id="s-coach" aria-labelledby="s-coach-h">
      {@render head(3)}
      <Panel flush><CoachSettings onChange={(c) => (coach = c)} /></Panel>
    </section>

    <section class="group" id="s-updates" aria-labelledby="s-updates-h">
      {@render head(4)}
      <Panel flush>
        <SettingRow
          title="Updates"
          sub={update.state === "available"
            ? `Version ${update.text} ist da. Ringside lädt sie, installiert sie und startet danach neu.`
            : update.state === "none"
              ? `Du hast Version ${version} – das ist die neueste.`
              : update.state === "installing"
                ? "Das Update wird geladen. Danach installiert sich Ringside und startet neu."
                : `Installiert ist Version ${version}.`}
        >
          {#snippet control()}
            {#if update.state === "none"}<span class="ok"><span class="dot on"></span>Aktuell</span>{/if}
            {#if update.state === "available"}
              <Button variant="primary" size="sm" onclick={install}>Version {update.text} installieren</Button>
            {:else if update.state !== "installing"}
              <Button size="sm" onclick={checkUpdate} loading={update.state === "checking"} disabled={update.state === "checking"}>
                {update.state === "checking" ? "Sucht …" : "Nach Updates suchen"}
              </Button>
            {/if}
          {/snippet}
          {#if update.state === "installing"}
            <div class="download">
              <ProgressBar value={download?.total ? download.done / download.total : null} size="md" label="Download des Updates" />
              <p class="meta">
                {#if download?.total}
                  {mb(download.done)} von {mb(download.total)} MB · {Math.round((download.done / download.total) * 100)} %
                {:else if download}
                  {mb(download.done)} MB geladen
                {:else}
                  Wird installiert, Ringside startet gleich neu …
                {/if}
              </p>
            </div>
          {:else if update.state === "error"}
            <EmptyState inline tone="error" title="Die Suche nach Updates hat nicht geklappt" text={update.text} />
          {/if}
        </SettingRow>
        <SettingRow
          title="Über Ringside"
          sub="Open Source unter der AGPL-3.0. Ringside liest nur deine eigenen Hand-Histories und zeigt einfache Zahlen-Statistiken an – kein Data-Mining, keine Echtzeit-Ratschläge."
        >
          {#snippet control()}<Button size="sm" variant="quiet" onclick={openRepo}>Projekt auf GitHub</Button>{/snippet}
        </SettingRow>
      </Panel>
    </section>
  </div>
  {/if}
</div>

{#snippet head(i: number)}
  <div class="ghead" id="{GROUPS[i].id}-h"><SectionHead title={GROUPS[i].label} sub={GROUPS[i].sub} /></div>
{/snippet}

<style>
  /* Setting rows keep a readable line; the page frame itself stays the app's one width. */
  .settings:not(.wide) > :global(*) {
    max-width: 1040px;
  }
  .settings.wide {
    display: grid;
    grid-template-columns: 300px minmax(0, 1fr);
    gap: var(--sp-10);
  }
  .overview {
    position: sticky;
    top: var(--sp-6);
    align-self: start;
    display: grid;
    gap: var(--sp-2);
  }
  .ov-title {
    margin-bottom: var(--sp-1);
    color: var(--text-2);
    font-size: var(--fs-md);
    font-weight: var(--fw-medium);
  }
  .ov-item {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .ov-item b {
    font-size: var(--fs-base);
    font-weight: var(--fw-semibold);
  }
  .ov-line {
    display: flex;
    align-items: baseline;
    gap: var(--sp-2);
    color: var(--text-2);
    font-size: var(--fs-md);
    line-height: 1.4;
  }
  .ov-line .dot {
    transform: translateY(-1px);
  }
  .dot.ok {
    background: var(--pos);
    border-color: var(--pos);
  }
  .dot.wait {
    border-color: var(--amber);
  }
  .dot.bad {
    background: var(--neg);
    border-color: var(--neg);
  }
  .group {
    scroll-margin-top: var(--sp-6);
  }
  .group + .group {
    margin-top: var(--sp-10);
  }
  .ghead {
    margin-bottom: var(--sp-3);
  }
  p {
    margin: 0;
  }
  .ico {
    flex-shrink: 0;
    width: 16px;
    height: 16px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.3;
    stroke-linejoin: round;
    stroke-linecap: round;
    color: var(--text-3);
  }
  svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 1.6;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .files,
  .tables,
  .other-list {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .files,
  .tables {
    border: 1px solid var(--line);
    border-radius: var(--r-sm);
    background: var(--surface-sunken);
  }
  .files li,
  .tables li {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    min-height: 40px;
    padding: var(--sp-2) var(--sp-3);
  }
  .files li + li,
  .tables li + li {
    border-top: 1px solid var(--line);
  }
  code {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--fs-md);
    color: var(--text);
  }
  .count {
    color: var(--text-2);
    font-size: var(--fs-base);
    white-space: nowrap;
  }
  .found {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-1);
    flex-shrink: 0;
    color: var(--text-2);
    font-size: var(--fs-base);
  }
  .found svg {
    width: 14px;
    height: 14px;
    color: var(--pos);
  }
  .meta {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    margin-top: var(--sp-2);
    color: var(--text-2);
    font-size: var(--fs-base);
  }
  .dot {
    flex-shrink: 0;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    border: 1.5px solid var(--text-3);
  }
  .dot.on {
    background: var(--pos);
    border-color: var(--pos);
  }
  .dot.wait {
    border-color: var(--amber);
  }
  .tables :global(.ui-tooltip-anchor),
  .tables li > .tname {
    flex: 1;
    min-width: 0;
  }
  .tname {
    display: flex;
    flex-direction: column;
    min-width: 0;
    font-size: var(--fs-md);
    line-height: 1.4;
  }
  .rest {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tname b {
    overflow-wrap: anywhere;
    font-weight: var(--fw-medium);
    color: var(--text);
  }
  .rest {
    color: var(--text-2);
  }
  .tstate {
    flex-shrink: 0;
    color: var(--text-2);
    font-size: var(--fs-base);
  }
  .none {
    color: var(--text-2);
    font-size: var(--fs-md);
  }
  .others {
    margin: var(--sp-2) 0 0 calc(-1 * var(--sp-2));
  }
  .chev {
    width: 12px;
    height: 12px;
    transition: transform var(--dur-fast) var(--ease-standard);
  }
  .chev.open {
    transform: rotate(180deg);
  }
  .other-list {
    display: grid;
    gap: var(--sp-1);
    margin-top: var(--sp-2);
    padding-left: var(--sp-3);
    border-left: 1px solid var(--line);
  }
  .other-list li {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-2);
    font-size: var(--fs-md);
  }
  .error {
    margin-top: var(--sp-2);
    color: var(--danger-text);
    font-size: var(--fs-md);
    overflow-wrap: anywhere;
  }
  .files .error {
    margin: 0;
  }
  .ok {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-2);
    margin-right: var(--sp-2);
    color: var(--pos);
    font-size: var(--fs-md);
  }
  .download {
    display: grid;
    gap: var(--sp-1);
    max-width: 68ch;
  }
</style>
