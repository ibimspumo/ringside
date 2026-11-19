<script lang="ts">
  import { onMount } from "svelte";
  import { slide } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import Button from "../ui/Button.svelte";
  import { cloudApi, onCloudChanged } from "./api";

  /**
   * `onSettings` opens the settings page. `quiet` while a warning stands above: one grey line
   * without a card, so the warning stays the loudest thing on the page.
   */
  let { onSettings, quiet = false }: { onSettings: () => void; quiet?: boolean } = $props();

  let show = $state(false);
  let details = $state(false);

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
  <div class="notice" class:quiet role="status">
    <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
      <path d="M7 18.5h10a4 4 0 0 0 .6-8 5.5 5.5 0 0 0-10.7-1A4.5 4.5 0 0 0 7 18.5z" />
    </svg>
    <p class="line">
      <b>Ringside Cloud ist an.</b>
      <span class="long">Geteilt werden nur Rechen-Eingaben und Ergebnisse, nichts Persönliches.</span>
      <span class="short">Geteilt werden nur Rechendaten, nichts Persönliches.</span>
    </p>
    <div class="actions">
      <Button size="sm" variant="quiet" aria-expanded={details} aria-controls="cloud-details" onclick={() => (details = !details)}>
        {details ? "Weniger" : "Details"}
      </Button>
      {#if !quiet}<Button size="sm" variant="quiet" onclick={settings}>Einstellungen</Button>{/if}
      <Button size="sm" variant={quiet ? "quiet" : "secondary"} onclick={ack}>Verstanden</Button>
    </div>
    {#if details}
      <p class="full" id="cloud-details" transition:slide={{ duration: 220, easing: cubicOut }}>
        Lösungen, die andere schon gerechnet haben (Push/Fold, Spots der Bibliothek und Flop-Spots), werden geladen; deine eigenen
        werden geteilt. Geteilt wird nur die Rechen-Eingabe (Stacks, Blinds, Antes, Auszahlungen; bei Flop-Spots Board, Pot, Größen
        und die Modell-Linie der Ranges) und das Ergebnis, nie deine Hände, Karten oder Namen. Deine IP-Adresse sieht der Anbieter
        Bunny technisch, gespeichert wird sie nur gehasht.
      </p>
    {/if}
  </div>
{/if}

<style>
  .notice {
    container-type: inline-size;
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    column-gap: var(--sp-3);
    margin: var(--sp-3) var(--frame-right, var(--frame-auto)) 0 var(--frame-left, var(--frame-auto));
    padding: 6px 6px 6px var(--sp-3);
    border: 1px solid var(--line);
    border-radius: var(--r);
    background: var(--surface);
  }
  .quiet {
    border-color: transparent;
    background: none;
  }
  .quiet .line {
    font-size: var(--fs-sm);
    color: var(--text-3);
  }
  .quiet b {
    color: var(--text-2);
    font-weight: var(--fw-regular);
  }
  .quiet svg {
    stroke: var(--text-3);
  }
  svg {
    fill: none;
    stroke: var(--sd);
    stroke-width: 1.6;
    stroke-linejoin: round;
  }
  .line {
    margin: 0;
    min-width: 0;
    font-size: var(--fs-md);
    line-height: var(--lh-tight);
    color: var(--text-2);
  }
  b {
    color: var(--text);
    font-weight: var(--fw-semibold);
  }
  /* One line in every window: narrow ones get the short sentence, "Details" says the rest. */
  .line {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .short {
    display: none;
  }
  @container (max-width: 1060px) {
    .long {
      display: none;
    }
    .short {
      display: inline;
    }
  }
  .actions {
    display: flex;
    gap: var(--sp-1);
  }
  .full {
    grid-column: 2 / -1;
    margin: var(--sp-1) 0 var(--sp-2);
    font-size: var(--fs-md);
    line-height: var(--lh-base);
    color: var(--text-2);
  }
</style>
