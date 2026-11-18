<script lang="ts">
  import { cloudApi, errorText } from "./api";
  import Button from "../ui/Button.svelte";
  import Dialog from "../ui/Dialog.svelte";
  import IconButton from "../ui/IconButton.svelte";
  import SettingRow from "../ui/SettingRow.svelte";
  import type { MalformedHand } from "./types";

  /** Changes whenever the importer counted new unreadable hands. */
  let { count }: { count: number } = $props();

  let hands = $state<MalformedHand[]>([]);
  let index = $state(0);
  let open = $state(false);
  let busy = $state(false);
  let message = $state<{ ok: boolean; text: string } | null>(null);

  $effect(() => {
    void count;
    cloudApi.malformed().then((h) => (hands = h));
  });

  const current = $derived(hands[Math.min(index, hands.length - 1)] ?? null);

  function show() {
    index = 0;
    message = null;
    open = true;
  }

  async function reload() {
    hands = await cloudApi.malformed();
    index = Math.min(index, Math.max(hands.length - 1, 0));
    if (hands.length === 0) open = false;
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
  <SettingRow
    title={hands.length === 1 ? "Eine Hand nicht lesbar" : `${hands.length} Hände nicht lesbar`}
    sub="Schick sie anonymisiert an Ringside, damit das Lesen besser wird. Du siehst vorher genau, was gesendet wird."
  >
    {#snippet control()}<Button size="sm" onclick={show}>Ansehen und melden …</Button>{/snippet}
  </SettingRow>
{:else if message?.ok}
  <SettingRow title="Nicht lesbare Hände" sub={message.text} />
{/if}

<Dialog bind:open title="Hand melden" sub={hands.length > 1 ? `${index + 1} von ${hands.length} nicht lesbaren Händen` : undefined} size="lg">
  {#if current}
    <div class="report">
      <p class="hint">
        So geht die Hand an Ringside Cloud, damit der Fehler im Lesen behoben werden kann: Spielernamen sind durch „Hero“ und
        „Spieler 1 …“ ersetzt, Tisch-, Turnier- und Handnummern entfernt, Chat-Texte gelöscht. Sonst wird nichts gesendet
        außer dem Grund und der Ringside-Version. Bitte prüf den Text, bevor du ihn sendest.
      </p>
      <div class="reason">
        <span>Grund: <code>{current.reason}</code></span>
        {#if hands.length > 1}
          <span class="nav">
            <IconButton label="Vorherige Hand" size="sm" disabled={index === 0} onclick={() => index--}
              ><svg viewBox="0 0 16 16" aria-hidden="true"><path d="M10 4 6 8l4 4" /></svg></IconButton
            >
            <IconButton label="Nächste Hand" size="sm" disabled={index >= hands.length - 1} onclick={() => index++}
              ><svg viewBox="0 0 16 16" aria-hidden="true"><path d="m6 4 4 4-4 4" /></svg></IconButton
            >
          </span>
        {/if}
      </div>
      <pre>{current.hand}</pre>
      {#if message}<p class:ok={message.ok} class:error={!message.ok} role="status">{message.text}</p>{/if}
    </div>
  {/if}
  {#snippet actions()}
    <Button variant="quiet" onclick={dismiss} disabled={busy}>Nicht senden, aus der Liste nehmen</Button>
    <span class="spacer"></span>
    <Button onclick={() => (open = false)}>Abbrechen</Button>
    <Button variant="primary" onclick={send} loading={busy} disabled={busy}>{busy ? "Wird gesendet …" : "Diese Hand senden"}</Button>
  {/snippet}
</Dialog>

<style>
  .report {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }
  p {
    margin: 0;
    font-size: var(--fs-md);
    line-height: 1.5;
  }
  .hint,
  .reason {
    color: var(--text-2);
    font-size: var(--fs-md);
  }
  .reason {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-3);
  }
  .nav {
    display: flex;
    gap: var(--sp-1);
  }
  svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 1.6;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  pre {
    max-height: 45vh;
    margin: 0;
    padding: var(--sp-3);
    overflow: auto;
    border: 1px solid var(--line);
    border-radius: var(--r-sm);
    background: var(--surface-sunken);
    font-size: var(--fs-sm);
    line-height: 1.5;
    white-space: pre;
  }
  .spacer {
    flex: 1;
  }
  .ok {
    color: var(--pos);
  }
  .error {
    color: var(--danger-text);
  }
</style>
