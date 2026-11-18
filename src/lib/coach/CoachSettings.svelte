<script lang="ts">
  import { onMount } from "svelte";
  import { COACH_INTRO, DEFAULT_MODEL, EFFORTS, coachApi, errorText, usd } from "./api";
  import Button from "../ui/Button.svelte";
  import Card from "../ui/Card.svelte";
  import IconButton from "../ui/IconButton.svelte";
  import Panel from "../ui/Panel.svelte";
  import SegmentedControl from "../ui/SegmentedControl.svelte";
  import SettingRow from "../ui/SettingRow.svelte";
  import Switch from "../ui/Switch.svelte";
  import KeyForm from "./KeyForm.svelte";
  import { modelLabel, money } from "./meaning";
  import type { CoachStatus, Effort, KeyInfo, ModelInfo } from "./types";

  let { onClose, onChange }: { onClose?: () => void; onChange?: (s: CoachStatus) => void } = $props();

  let status = $state<CoachStatus | null>(null);
  let message = $state<{ ok: boolean; text: string } | null>(null);
  let busy = $state(false);
  let replacing = $state(false);
  let confirmDelete = $state(false);

  let models = $state<ModelInfo[] | null>(null);
  let modelsError = $state<string | null>(null);
  let search = $state("");
  let toolsOnly = $state(true);
  let open = $state(false);
  let technical = $state(false);

  function setStatus(s: CoachStatus) {
    status = s;
    onChange?.(s);
  }

  onMount(async () => {
    setStatus(await coachApi.status());
    // Whether the chosen model reasons decides if the thoroughness choice shows. Only once
    // a key is set: until then the coach is unused and the settings page stays offline.
    if (!status?.has_key) return;
    coachApi.models().then(
      (list) => (models ??= list),
      () => {},
    );
  });

  async function setEffort(e: Effort) {
    await coachApi.setEffort(e);
    if (status) setStatus({ ...status, effort: e });
  }

  async function act(f: () => Promise<KeyInfo | void>, ok: (info: KeyInfo | null) => string) {
    busy = true;
    message = null;
    try {
      const info = await f();
      message = { ok: true, text: ok(info ?? null) };
      setStatus(await coachApi.status());
    } catch (e) {
      message = { ok: false, text: errorText(e) };
    } finally {
      busy = false;
    }
  }

  const spent = (i: KeyInfo | null) =>
    i ? ` Verbraucht ${usd(i.usage)}${i.limit != null ? ` von ${usd(i.limit)}` : ""}.` : "";
  const test = () => act(coachApi.testKey, (i) => `OpenRouter kennt den Key.${spent(i)}`);
  const remove = () => act(coachApi.deleteKey, () => "Key gelöscht.");

  async function saved() {
    replacing = false;
    message = null;
    setStatus(await coachApi.status());
  }

  async function loadModels() {
    open = true;
    if (models) return;
    try {
      models = await coachApi.models();
      modelsError = null;
    } catch (e) {
      modelsError = errorText(e);
    }
  }

  async function choose(m: ModelInfo) {
    await coachApi.setModel(m.id);
    if (status) setStatus({ ...status, model: m.id });
    open = false;
  }

  const filtered = $derived.by(() => {
    const q = search.trim().toLowerCase();
    return (models ?? [])
      .filter((m) => !toolsOnly || m.tools)
      .filter((m) => !q || m.id.toLowerCase().includes(q) || m.name.toLowerCase().includes(q));
  });
  const current = $derived(models?.find((m) => m.id === status?.model) ?? null);
  const reasons = $derived(current?.reasoning ?? true);
  const perM = (v: number | null) => (v == null ? "–" : usd(v, v < 1 ? 3 : 2));
  const ctx = (n: number | null) =>
    n == null ? "–" : n >= 1e6 ? `${(n / 1e6).toLocaleString("de-DE", { maximumFractionDigits: 1 })} Mio.` : `${Math.round(n / 1000)} Tsd.`;
  /** Rough cost of one tournament (about 30 hands sent), to compare models; each run shows its own estimate. */
  const perTournament = (m: ModelInfo) =>
    m.prompt_per_m == null || m.completion_per_m == null ? null : (30000 * m.prompt_per_m + 8000 * m.completion_per_m) / 1e6;
  const cheapest = $derived.by(() => {
    const priced = (models ?? []).filter((m) => m.tools && perTournament(m) != null);
    return priced.length ? priced.reduce((a, b) => (perTournament(b)! < perTournament(a)! ? b : a)).id : null;
  });
  const dearest = $derived(Math.max(0, ...(models ?? []).map((m) => perTournament(m) ?? 0)));
  /** Bar width on a linear scale, so the bars compare like the prices; the cheapest still show a sliver. */
  const priceShare = (t: number | null) => (t == null || dearest <= 0 ? 0 : Math.max(3, (t / dearest) * 100));
  const RICHTWERT = "Richtwert für ein Turnier mit rund 30 gesendeten Händen. Vor jeder Analyse siehst du die genaue Schätzung.";
  const provider = (m: ModelInfo) => m.name.split(":")[0];
  /** "DeepSeek: V4" → "V4" reads as nothing alone, so short names keep their maker. */
  const modelName = (m: ModelInfo) => {
    const name = m.name.slice(m.name.indexOf(":") + 1).trim();
    return name.length <= 4 && m.name.includes(":") ? `${provider(m)} ${name}` : name;
  };
</script>

{#snippet rows()}
  <SettingRow
    title="Zugang"
    sub={status?.has_key
      ? "Dein Key von OpenRouter. Er liegt im Schlüsselspeicher von Windows, nicht in Ringsides Dateien."
      : "Den Key bekommst du auf openrouter.ai/keys. Er liegt danach im Schlüsselspeicher von Windows, nicht in Ringsides Dateien."}
  >
    {#snippet control()}
      {#if status?.has_key}
        <span class="state on"><i></i>Key hinterlegt</span>
        {#if confirmDelete}
          <span class="ask">Wirklich löschen?</span>
          <Button size="sm" variant="danger" onclick={() => { confirmDelete = false; remove(); }} disabled={busy}>Ja, löschen</Button>
          <Button size="sm" variant="quiet" onclick={() => (confirmDelete = false)}>Behalten</Button>
        {:else}
          <Button size="sm" onclick={test} loading={busy} disabled={busy}>Testen</Button>
          <Button size="sm" onclick={() => (replacing = !replacing)} aria-expanded={replacing}>Ersetzen</Button>
          <Button size="sm" variant="danger" onclick={() => (confirmDelete = true)} disabled={busy}>Löschen</Button>
        {/if}
      {:else if status}
        <span class="state"><i></i>Noch kein Key</span>
      {/if}
    {/snippet}
    {#if (status && !status.has_key) || replacing || message}
      <div class="detail">
        {#if status && !status.has_key}
          <KeyForm onSaved={saved} />
        {:else if replacing}
          <KeyForm replace onSaved={saved} />
        {/if}
        {#if message}<p class="msg" class:ok={message.ok} role="status">{message.text}</p>{/if}
      </div>
    {/if}
  </SettingRow>

  <SettingRow
    title="Modell"
    sub={status?.model === DEFAULT_MODEL
      ? "Welches Sprachmodell deine Hände bespricht. Das ist Ringsides Empfehlung für den Einstieg – im Zweifel bleib dabei."
      : "Welches Sprachmodell deine Hände bespricht. Der Preis ist ein Richtwert für ein Turnier."}
  >
    {#snippet control()}
      <div class="what">
        {#if current}
          {@const t = perTournament(current)}
          <b>{modelName(current)}</b>
          <span class="fine num" title={RICHTWERT}>{provider(current)} · {t == null ? "Preis unbekannt" : `ca. ${money(t)} je Turnier`}</span>
        {:else if status}
          <b>{modelLabel(status.model)}</b>
          <span class="fine">{status.has_key ? "Lädt Preise …" : "Preise lädt Ringside, sobald dein Key da ist"}</span>
        {:else}
          <b>…</b>
        {/if}
      </div>
      {#if !open}
        <Button size="sm" onclick={loadModels} disabled={!status?.has_key}
          title={status?.has_key ? undefined : "Erst mit Key lädt Ringside die Modelle von OpenRouter."}>Anderes Modell</Button
        >
      {/if}
    {/snippet}

    {#if open}
      <div class="picker">
        <div class="tools-row">
          <input class="field search" type="search" placeholder="Modell suchen" aria-label="Modell suchen" bind:value={search} />
          <Button size="sm" onclick={() => (open = false)}>Fertig</Button>
        </div>
        <div class="opts">
          <label class="opt"><Switch label="Nur Modelle, die mit Ringside funktionieren" bind:checked={toolsOnly} />Nur Modelle, die mit Ringside funktionieren</label>
          <label class="opt"><Switch label="Technische Angaben" bind:checked={technical} />Technische Angaben</label>
        </div>
        {#if modelsError}
          <p class="msg">{modelsError}</p>
        {:else if !models}
          <p class="fine">Lade die Modelle von OpenRouter …</p>
        {:else}
          <div class="list" class:technical role="group" aria-label="Modelle">
            <div class="lhead" aria-hidden="true">
              <span>Modell</span><span title={RICHTWERT}>ca. je Turnier</span>{#if technical}<span>Ein / Aus je 1 Mio.</span><span
                  >Kontext</span
                >{/if}
            </div>
            {#each filtered.slice(0, 200) as m (m.id)}
              {@const t = perTournament(m)}
              <Card
                pad="sm"
                selected={m.id === status?.model}
                disabled={!m.tools}
                onclick={() => choose(m)}
                title={m.tools ? m.id : "Ohne Werkzeuge kann der Coach nicht rechnen."}
              >
                <span class="lgrid">
                <span class="mname">
                  <span class="mtitle">
                    {modelName(m)}
                    {#if m.id === status?.model}<em class="tag on">deine Wahl</em>{/if}
                    {#if m.id === DEFAULT_MODEL}<em class="tag" title="Ringsides Voreinstellung. Im Zweifel bleib dabei.">Empfohlen für den Einstieg</em>{/if}
                    {#if m.id === cheapest}<em class="tag">am günstigsten</em>{/if}
                  </span>
                  <small>{provider(m)}{m.tools ? "" : " · kann nicht rechnen"}</small>
                </span>
                <span class="cost">
                  <span class="pbar" aria-hidden="true"><i style="width:{priceShare(t)}%"></i></span>
                  <span class="num strong">{t == null ? "–" : money(t)}</span>
                </span>
                {#if technical}
                  <span class="num">{perM(m.prompt_per_m)} / {perM(m.completion_per_m)}</span>
                  <span class="num">{ctx(m.context_length)}</span>
                {/if}
                </span>
              </Card>
            {:else}
              <p class="fine pad">Kein Modell gefunden.</p>
            {/each}
          </div>
          <p class="fine">
            Faustregel: Teurere Modelle besprechen meist gründlicher, günstige reichen für einen ersten Überblick. Im Zweifel bleib
            bei der Empfehlung. „Je Turnier“ ist ein Richtwert für rund 30 gesendete Hände; vor jeder Analyse siehst du die genaue
            Schätzung.
          </p>
        {/if}
      </div>
    {/if}
  </SettingRow>

  {#if status}
    {@const effort = EFFORTS.find((e) => e.id === status?.effort)}
    <SettingRow
      title="Gründlichkeit"
      sub={reasons
        ? (effort?.hint ?? "Wie lange das Modell nachdenkt.")
        : "Dieses Modell denkt nicht gesondert nach, deshalb gibt es hier nichts zu wählen."}
    >
      {#snippet control()}
        {#if reasons}
          <SegmentedControl
            label="Gründlichkeit"
            value={status!.effort}
            options={[...EFFORTS].reverse().map((e) => ({ value: e.id, label: e.name, hint: e.hint }))}
            onchange={setEffort}
          />
        {/if}
      {/snippet}
    </SettingRow>
  {/if}
{/snippet}

{#if onClose}
  <Panel flush title="KI-Coach einrichten" sub={COACH_INTRO}>
    {#snippet actions()}
      <IconButton label="Einstellungen schließen" onclick={onClose}
        ><svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true"><path d="M6 6l12 12M18 6 6 18" /></svg></IconButton
      >
    {/snippet}
    <div class="rows">{@render rows()}</div>
  </Panel>
{:else}
  {@render rows()}
{/if}

<style>
  svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linecap: round;
  }
  p {
    margin: 0;
    line-height: 1.5;
  }
  .rows {
    border-top: 1px solid var(--line);
    margin-top: var(--sp-3);
  }
  .ask {
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .fine {
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .detail {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }
  .state {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-2);
    margin-right: var(--sp-2);
    color: var(--text-2);
    font-size: var(--fs-md);
    white-space: nowrap;
  }
  .state i {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    border: 1.5px solid var(--text-3);
  }
  .state.on {
    color: var(--text);
  }
  .state.on i {
    background: var(--pos);
    border-color: var(--pos);
  }
  .msg {
    font-size: var(--fs-md);
    color: var(--danger-text);
  }
  .msg.ok {
    color: var(--pos);
  }

  .what {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    margin-right: var(--sp-2);
    min-width: 0;
    text-align: right;
  }
  .what b {
    font-size: var(--fs-base);
    font-weight: var(--fw-semibold);
  }
  .what .fine {
    cursor: help;
    white-space: nowrap;
  }

  .picker {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }
  .tools-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2) var(--sp-3);
  }
  .search {
    flex: 1 1 220px;
  }
  .opts {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-2) var(--sp-6);
  }
  .opt {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-2);
    color: var(--text-2);
    font-size: var(--fs-md);
    cursor: pointer;
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: var(--sp-1);
    max-height: 380px;
    overflow: auto;
    padding: 0 var(--sp-1) var(--sp-1);
    border: 1px solid var(--line);
    border-radius: var(--r-sm);
  }
  .lhead,
  .lgrid {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 190px;
    gap: var(--sp-3);
    align-items: center;
  }
  .technical .lhead,
  .technical .lgrid {
    grid-template-columns: minmax(0, 1fr) 190px 130px 76px;
  }
  .lhead {
    position: sticky;
    top: 0;
    z-index: 1;
    padding: var(--sp-2) var(--sp-3);
    background: var(--surface);
    border-bottom: 1px solid var(--line);
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .lhead span:not(:first-child),
  .lgrid > span:not(:first-child) {
    text-align: right;
  }
  .lgrid {
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .mname {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .mtitle {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    min-width: 0;
    color: var(--text);
    white-space: nowrap;
  }
  .tag {
    padding: 0 6px;
    border-radius: 4px;
    border: 1px solid var(--line-2);
    font-size: var(--fs-xs);
    font-style: normal;
    line-height: 17px;
    color: var(--text-2);
  }
  .tag.on {
    border-color: var(--text-2);
    color: var(--text);
  }
  .cost {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 10px;
  }
  .pbar {
    width: 90px;
    height: 5px;
    border-radius: 3px;
    background: var(--surface-3);
    overflow: hidden;
  }
  .pbar i {
    display: block;
    height: 100%;
    border-radius: 3px;
    background: var(--text-2);
  }
  .lgrid .strong {
    min-width: 52px;
    text-align: right;
  }
  .mname small {
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .strong {
    color: var(--text);
    font-weight: var(--fw-semibold);
  }
  .pad {
    padding: var(--sp-3);
  }

  @container (max-width: 760px) {
    .technical .lhead,
    .technical .lgrid {
      grid-template-columns: minmax(0, 1fr) 150px 110px;
    }
    .technical .lhead span:last-child,
    .technical .lgrid > span:last-child {
      display: none;
    }
    .pbar {
      width: 60px;
    }
  }
</style>
