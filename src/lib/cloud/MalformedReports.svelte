<script lang="ts">
  import { cloudApi, errorText } from "./api";
  import type { MalformedHand } from "./types";

  /** Changes whenever the importer counted new unreadable hands. */
  let { count }: { count: number } = $props();

  let hands = $state<MalformedHand[]>([]);
  let index = $state(0);
  let dialog = $state<HTMLDialogElement | null>(null);
  let busy = $state(false);
  let message = $state<{ ok: boolean; text: string } | null>(null);

  $effect(() => {
    void count;
    cloudApi.malformed().then((h) => (hands = h));
  });

  const current = $derived(hands[Math.min(index, hands.length - 1)] ?? null);

  function open() {
    index = 0;
    message = null;
    dialog?.showModal();
  }

  async function reload() {
    hands = await cloudApi.malformed();
    index = Math.min(index, Math.max(hands.length - 1, 0));
    if (hands.length === 0) dialog?.close();
  }

  async function send() {
    if (!current) return;
    busy = true;
    message = null;
    try {
      await cloudApi.sendReport(current.id);
      message = { ok: true, text: "Danke, die Hand ist gesendet." };
      await reload();
    } catch (e) {
      message = { ok: false, text: errorText(e) };
    } finally {
      busy = false;
    }
  }

  async function dismiss() {
    if (!current) return;
    await cloudApi.dismiss(current.id);
    message = null;
    await reload();
  }
</script>

{#if hands.length}
  <div class="row">
    <span>{hands.length === 1 ? "Eine Hand" : `${hands.length} Hände`} konnte Ringside nicht lesen.</span>
    <button class="btn small" onclick={open}>Fehler melden …</button>
  </div>
{:else if message?.ok}
  <p class="ok">{message.text}</p>
{/if}

<dialog bind:this={dialog} aria-labelledby="report-title">
  {#if current}
    <header>
      <h2 id="report-title">Hand melden</h2>
      {#if hands.length > 1}
        <div class="nav">
          <button class="btn small" disabled={index === 0} onclick={() => index--} aria-label="Vorherige Hand">‹</button>
          <span class="num">{index + 1} von {hands.length}</span>
          <button class="btn small" disabled={index >= hands.length - 1} onclick={() => index++} aria-label="Nächste Hand"
            >›</button
          >
        </div>
      {/if}
    </header>
    <p class="hint">
      So geht die Hand an Ringside Cloud, damit der Fehler im Lesen behoben werden kann: Spielernamen sind durch „Hero“ und
      „Spieler 1 …“ ersetzt, Tisch-, Turnier- und Handnummern entfernt, Chat-Texte gelöscht. Sonst wird nichts gesendet außer dem
      Grund und der Ringside-Version. Bitte prüf den Text, bevor du ihn sendest.
    </p>
    <p class="reason">Grund: <code>{current.reason}</code></p>
    <pre>{current.hand}</pre>
    {#if message}<p class:ok={message.ok} class:error={!message.ok}>{message.text}</p>{/if}
    <footer>
      <button class="btn" onclick={dismiss} disabled={busy}>Nicht senden, aus der Liste nehmen</button>
      <span class="spacer"></span>
      <button class="btn" onclick={() => dialog?.close()}>Abbrechen</button>
      <button class="btn primary" onclick={send} disabled={busy}>{busy ? "Wird gesendet …" : "Diese Hand senden"}</button>
    </footer>
  {/if}
</dialog>

<style>
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    font-size: 13px;
    color: var(--text-2);
  }
  .btn.small {
    height: 28px;
    padding: 0 10px;
    font-size: 12.5px;
  }
  dialog {
    width: min(760px, calc(100vw - 48px));
    max-height: calc(100vh - 64px);
    padding: 18px 20px;
    border: 1px solid var(--line-2);
    border-radius: var(--r);
    background: var(--surface);
    color: var(--text);
  }
  dialog[open] {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  dialog::backdrop {
    background: rgba(0, 0, 0, 0.55);
  }
  header,
  footer,
  .nav {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  header {
    justify-content: space-between;
  }
  h2 {
    margin: 0;
    font-size: 16px;
    font-weight: 600;
  }
  .hint,
  .reason {
    margin: 0;
    font-size: 12.5px;
    line-height: 1.5;
    color: var(--text-2);
  }
  pre {
    flex: 1;
    min-height: 120px;
    margin: 0;
    padding: 10px 12px;
    overflow: auto;
    border: 1px solid var(--line);
    border-radius: var(--r-sm);
    background: var(--bg);
    font-size: 12px;
    line-height: 1.5;
    white-space: pre;
  }
  .spacer {
    flex: 1;
  }
  .ok {
    margin: 0;
    color: var(--pos);
    font-size: 12.5px;
  }
  .error {
    margin: 0;
    color: var(--neg);
    font-size: 12.5px;
  }
</style>
