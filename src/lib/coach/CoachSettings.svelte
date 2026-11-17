<script lang="ts">
  import { onMount } from "svelte";
  import { EFFORTS, coachApi, errorText, usd } from "./api";
  import type { CoachStatus, Effort, KeyInfo, ModelInfo } from "./types";

  let status = $state<CoachStatus | null>(null);
  let key = $state("");
  let keyInfo = $state<KeyInfo | null>(null);
  let message = $state<{ ok: boolean; text: string } | null>(null);
  let busy = $state(false);

  let models = $state<ModelInfo[] | null>(null);
  let modelsError = $state<string | null>(null);
  let search = $state("");
  let toolsOnly = $state(true);
  let open = $state(false);

  onMount(async () => {
    status = await coachApi.status();
    // Whether the chosen model reasons decides if the thoroughness choice shows. Only once
    // a key is set: until then the coach is unused and the settings page stays offline.
    if (!status.has_key) return;
    coachApi.models().then(
      (list) => (models ??= list),
      () => {},
    );
  });

  async function setEffort(e: Effort) {
    await coachApi.setEffort(e);
    if (status) status.effort = e;
  }

  async function act(f: () => Promise<KeyInfo | void>, ok: string) {
    busy = true;
    message = null;
    try {
      const info = await f();
      keyInfo = info ?? null;
      message = { ok: true, text: ok };
      status = await coachApi.status();
    } catch (e) {
      message = { ok: false, text: errorText(e) };
    } finally {
      busy = false;
    }
  }

  const save = () =>
    act(async () => {
      const info = await coachApi.setKey(key);
      key = "";
      return info;
    }, "Key gespeichert und von OpenRouter bestätigt.");
  const test = () => act(coachApi.testKey, "OpenRouter kennt den Key.");
  const remove = () => act(coachApi.deleteKey, "Key gelöscht.");

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
    if (status) status.model = m.id;
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
  const ctx = (n: number | null) => (n == null ? "–" : n >= 1e6 ? `${(n / 1e6).toLocaleString("de-DE")} Mio.` : `${Math.round(n / 1000)} Tsd.`);
</script>

<section class="panel coach">
  <div class="panel-head"><h2 class="panel-title">KI-Coach</h2></div>
  <div class="body">
    <p class="hint">
      Der Coach schickt nach einem Turnier deine Hände ohne Namen über OpenRouter an ein Sprachmodell, aber nur, wenn du
      eine Analyse startest und die Kosten bestätigst. Den Key bekommst du auf openrouter.ai; er liegt danach im
      Schlüsselspeicher von Windows, nicht in Ringsides Dateien.
    </p>

    <div class="keyrow">
      <span class="state" class:on={status?.has_key}>
        <i></i>{status === null ? "…" : status.has_key ? "Key hinterlegt" : "Kein Key hinterlegt"}
      </span>
      {#if status?.has_key}
        <button class="btn" onclick={test} disabled={busy}>Testen</button>
        <button class="btn ghost danger" onclick={remove} disabled={busy}>Löschen</button>
      {/if}
    </div>
    <form
      class="row"
      onsubmit={(e) => {
        e.preventDefault();
        if (key.trim()) save();
      }}
    >
      <input
        class="field key"
        type="password"
        autocomplete="off"
        spellcheck="false"
        placeholder={status?.has_key ? "Neuen Key einfügen, um ihn zu ersetzen" : "sk-or-v1-…"}
        aria-label="OpenRouter-API-Key"
        bind:value={key}
      />
      <button class="btn primary" type="submit" disabled={busy || !key.trim()}>Speichern und testen</button>
    </form>
    {#if message}
      <p class={message.ok ? "ok" : "error"}>
        {message.text}
        {#if message.ok && keyInfo}
          <span class="dim num">
            Verbraucht {usd(keyInfo.usage)}{keyInfo.limit != null ? ` von ${usd(keyInfo.limit)}` : ""}.
          </span>
        {/if}
      </p>
    {/if}

    <div class="model">
      <span class="label">Modell</span>
      <div class="chosen">
        <b class="num">{status?.model ?? "…"}</b>
        {#if current}
          <span class="dim num">{perM(current.prompt_per_m)} ein · {perM(current.completion_per_m)} aus je 1 Mio. Tokens</span>
        {/if}
      </div>
      {#if !open}<button class="btn" onclick={loadModels}>Modell wählen</button>{/if}
    </div>

    {#if status}
      <div class="effort">
        <span class="label" id="effort-label">Gründlichkeit</span>
        {#if reasons}
          <div class="seg" role="radiogroup" aria-labelledby="effort-label">
            {#each EFFORTS as e (e.id)}
              <button
                role="radio"
                aria-checked={status.effort === e.id}
                class:on={status.effort === e.id}
                onclick={() => setEffort(e.id)}>{e.name}</button
              >
            {/each}
          </div>
          <span class="dim small">{EFFORTS.find((e) => e.id === status?.effort)?.hint}</span>
        {:else}
          <span class="dim small">Dieses Modell denkt nicht gesondert nach, deshalb gibt es hier nichts zu wählen.</span>
        {/if}
      </div>
    {/if}

    {#if open}
      <div class="picker">
        <div class="row">
          <input class="field search" type="search" placeholder="Modell suchen" aria-label="Modell suchen" bind:value={search} />
          <label class="check"><input type="checkbox" bind:checked={toolsOnly} /> Nur mit Werkzeugen</label>
          <button class="btn ghost" onclick={() => (open = false)}>Fertig</button>
        </div>
        {#if modelsError}
          <p class="error">{modelsError}</p>
        {:else if !models}
          <p class="dim">Lade die Modelle von OpenRouter …</p>
        {:else}
          <div class="scroll">
            <table class="table">
              <thead>
                <tr><th>Modell</th><th>Ein</th><th>Aus</th><th>Kontext</th></tr>
              </thead>
              <tbody>
                {#each filtered.slice(0, 200) as m (m.id)}
                  <tr class:on={m.id === status?.model} onclick={() => choose(m)} title={m.tools ? m.id : "Ohne Werkzeuge kann der Coach nicht rechnen."}>
                    <td class="mname">
                      <span>{m.name}</span>
                      <small class="faint">{m.id}{m.tools ? "" : " · ohne Werkzeuge"}</small>
                    </td>
                    <td class="num">{perM(m.prompt_per_m)}</td>
                    <td class="num">{perM(m.completion_per_m)}</td>
                    <td class="num">{ctx(m.context_length)}</td>
                  </tr>
                {:else}
                  <tr><td colspan="4" class="faint">Kein Modell gefunden.</td></tr>
                {/each}
              </tbody>
            </table>
          </div>
          <p class="faint small">Preise je 1 Mio. Tokens laut OpenRouter. Klick wählt das Modell.</p>
        {/if}
      </div>
    {/if}
  </div>
</section>

<style>
  .coach {
    grid-column: 1 / -1;
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 12px 18px 18px;
  }
  p {
    margin: 0;
    line-height: 1.5;
  }
  .hint {
    color: var(--text-2);
    max-width: 760px;
  }
  .row,
  .keyrow {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
  .state {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    margin-right: 6px;
    color: var(--text-2);
  }
  .state i {
    width: 7px;
    height: 7px;
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
  .key {
    flex: 1 1 260px;
    max-width: 440px;
    font-family: var(--font-num);
  }
  .ok {
    color: var(--pos);
  }
  .error {
    color: #ff9d93;
  }
  .model {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 14px;
    padding-top: 12px;
    border-top: 1px solid var(--line);
  }
  .label {
    font-size: 12.5px;
    color: var(--text-2);
  }
  .chosen {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }
  .chosen .dim {
    font-size: 12.5px;
  }
  .effort {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 14px;
  }
  .picker {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .search {
    flex: 1 1 220px;
    max-width: 360px;
  }
  .scroll {
    max-height: 340px;
    overflow: auto;
    border: 1px solid var(--line);
    border-radius: var(--r-sm);
  }
  tbody tr {
    cursor: pointer;
  }
  tbody tr.on td {
    background: var(--surface-2);
  }
  .table td {
    height: auto;
    padding: 6px 10px;
  }
  td.mname {
    display: flex;
    flex-direction: column;
    text-align: left;
    font-family: var(--font-ui);
  }
  th:first-child {
    text-align: left;
  }
  .small {
    font-size: 12px;
  }
</style>
