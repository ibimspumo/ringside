<script lang="ts">
  import { coachApi, errorText, usd } from "./api";
  import type { KeyInfo } from "./types";

  /** Pastes, stores and checks the OpenRouter key; `replace` words it for an existing key. */
  let { replace = false, onSaved }: { replace?: boolean; onSaved: (info: KeyInfo) => void } = $props();

  let key = $state("");
  let busy = $state(false);
  let message = $state<{ ok: boolean; text: string } | null>(null);

  async function save(e: SubmitEvent) {
    e.preventDefault();
    if (!key.trim()) return;
    busy = true;
    message = null;
    try {
      const info = await coachApi.setKey(key.trim());
      key = "";
      const spent = `Verbraucht ${usd(info.usage)}${info.limit != null ? ` von ${usd(info.limit)}` : ""}.`;
      message = { ok: true, text: `Key gespeichert und von OpenRouter bestätigt. ${spent}` };
      onSaved(info);
    } catch (err) {
      message = { ok: false, text: errorText(err) };
    } finally {
      busy = false;
    }
  }
</script>

<form class="key" onsubmit={save}>
  <input
    class="field"
    type="password"
    autocomplete="off"
    spellcheck="false"
    placeholder={replace ? "Neuen Key einfügen, um den alten zu ersetzen" : "sk-or-v1-…"}
    aria-label="OpenRouter-API-Key"
    bind:value={key}
  />
  <button class="btn primary" type="submit" disabled={busy || !key.trim()}>
    {busy ? "Wird geprüft …" : "Speichern und testen"}
  </button>
</form>
{#if message}
  <p class="msg" class:ok={message.ok} role="status">{message.text}</p>
{/if}

<style>
  .key {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .field {
    flex: 1 1 240px;
    max-width: 420px;
    font-family: var(--font-num);
  }
  .msg {
    margin: 8px 0 0;
    font-size: 13px;
    line-height: 1.45;
    color: #ff9d93;
  }
  .msg.ok {
    color: var(--pos);
  }
</style>
