<script lang="ts">
  import { onMount } from "svelte";
  import { cloudApi, onCloudChanged } from "./api";

  /** Opens the settings page. */
  let { onSettings }: { onSettings: () => void } = $props();

  let show = $state(false);

  onMount(() => {
    cloudApi.settings().then((s) => (show = !s.notice_seen));
    const sub = onCloudChanged((c) => (show = !c.notice_seen));
    return () => {
      sub.then((f) => f());
    };
  });

  async function ack() {
    show = false;
    await cloudApi.ackNotice();
  }

  async function settings() {
    await ack();
    onSettings();
  }
</script>

{#if show}
  <div class="notice" role="status">
    <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
      <path d="M7 18.5h10a4 4 0 0 0 .6-8 5.5 5.5 0 0 0-10.7-1A4.5 4.5 0 0 0 7 18.5z" />
    </svg>
    <p>
      <b>Ringside Cloud ist an.</b> Lösungen, die andere schon gerechnet haben (Push/Fold, Spots der Bibliothek und Flop-Spots),
      werden geladen; deine eigenen werden geteilt. Geteilt wird nur die Rechen-Eingabe (Stacks, Blinds, Antes, Auszahlungen;
      bei Flop-Spots Board, Pot, Größen und die Modell-Linie der Ranges) und das Ergebnis, nie deine Hände, Karten oder Namen. Deine IP-Adresse sieht der Anbieter
      Bunny technisch, gespeichert wird sie nur gehasht.
    </p>
    <div class="actions">
      <button class="btn small primary" onclick={ack}>Verstanden</button>
      <button class="btn small" onclick={settings}>Einstellungen</button>
    </div>
  </div>
{/if}

<style>
  .notice {
    display: flex;
    align-items: center;
    gap: 12px;
    margin: 12px 24px 0;
    padding: 10px 14px;
    border: 1px solid var(--line-2);
    border-radius: var(--r);
    background: var(--surface-2);
  }
  svg {
    flex: none;
    fill: none;
    stroke: var(--sd);
    stroke-width: 1.6;
    stroke-linejoin: round;
  }
  p {
    flex: 1;
    margin: 0;
    font-size: 12.5px;
    line-height: 1.45;
    color: var(--text-2);
  }
  b {
    color: var(--text);
    font-weight: 600;
  }
  .actions {
    display: flex;
    flex: none;
    gap: 8px;
  }
  .btn.small {
    height: 28px;
    padding: 0 12px;
    font-size: 12.5px;
  }
</style>
