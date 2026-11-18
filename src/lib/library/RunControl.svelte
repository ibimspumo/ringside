<script lang="ts">
  import { libraryApi } from "./api";
  import { flopName, threadsFor, titleOf } from "./labels";
  import type { LibraryOverview, LibraryPower, WorkerStatus } from "./types";

  /** The start/stop button with the worker's state in words and the power choice. */
  let { data, worker, onChange }: { data: LibraryOverview; worker: WorkerStatus; onChange: () => void } = $props();

  let busy = $state(false);
  let error = $state<string | null>(null);

  const running = $derived(worker.manual || worker.state === "solving" || worker.state === "cloud");
  const done = $derived(worker.state === "done");
  const spot = $derived.by(() => {
    const s = data.scenarios.find((x) => x.id === worker.scenario);
    return s && worker.flop ? `${titleOf(s)}, Flop ${flopName(worker.flop)}` : null;
  });
  const pct = $derived(worker.state === "solving" && worker.progress !== null ? Math.round(worker.progress * 100) : null);

  /** Headline and the sentence under it. */
  const words = $derived.by((): [string, string] => {
    const how = worker.manual
      ? "Von dir gestartet. Läuft, bis du anhältst, PokerStars startet oder alle Spots fertig sind."
      : "Die Automatik rechnet, solange du die App nicht benutzt.";
    switch (worker.state) {
      case "solving":
        return ["Rechnet gerade:", how];
      case "cloud":
        return ["Schaut in der Cloud nach:", "Hat schon jemand diesen Spot gerechnet? Dann lädt Ringside ihn, statt selbst zu rechnen."];
      case "packs":
        return ["Lädt fertige Spots herunter", "Das kostet deinen PC keine Rechenzeit."];
      case "done":
        return ["Fertig", "Alle Spots der Bibliothek sind gelöst. Danke fürs Mitrechnen!"];
      case "off":
        return ["Aus", "Die Automatik ist aus. Mit „Jetzt mitrechnen“ startest du selbst."];
    }
    switch (worker.reason) {
      case "client":
        return ["Wartet: PokerStars läuft", "Solange PokerStars offen ist, rechnet Ringside nie."];
      case "halted":
        return ["Angehalten", "Die Automatik ruht, bis du wieder startest oder bei „Automatik“ etwas anderes wählst."];
      case "idle":
        return ["Wartet auf eine Pause", "Die Automatik legt los, wenn fünf Minuten niemand Maus oder Tastatur benutzt. Oder starte gleich selbst."];
      case "idle_unknown":
        return ["Wartet", "Ob der PC gerade frei ist, erkennt Ringside nur unter Windows. Starte hier selbst."];
      case "in_use":
        return ["Wartet, solange du Ringside benutzt", "Die Automatik lässt dich in Ruhe arbeiten. Mit „Jetzt mitrechnen“ geht es sofort los, auch während du Ringside benutzt."];
    }
    return ["Startet …", ""];
  });

  async function toggle() {
    busy = true;
    try {
      await libraryApi.run(!running);
      error = null;
      onChange();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  const POWERS: { id: LibraryPower; label: string; text: string }[] = [
    { id: "gentle", label: "Sanft", text: "Leise und sparsam. Dauert länger, der PC bleibt kühl." },
    { id: "normal", label: "Normal", text: "Empfohlen. Schnell genug, und der PC bleibt flott." },
    { id: "full", label: "Volle Kraft", text: "Am schnellsten, braucht aber deutlich mehr Strom, und der Lüfter wird hörbar." },
  ];
  const power = $derived(POWERS.find((p) => p.id === data.power) ?? POWERS[1]);

  function setPower(p: LibraryPower) {
    data.power = p;
    libraryApi.setPower(p).catch((e) => (error = String(e)));
  }
</script>

<div class="run">
  <button
    class="go"
    class:running
    style="--p:{running ? (worker.progress ?? 0) : 0}"
    disabled={busy || (done && !running)}
    aria-pressed={running}
    onclick={toggle}
  >
    <span class="face">
      <svg viewBox="0 0 24 24" width="22" height="22" aria-hidden="true">
        {#if running}
          <rect x="6" y="5" width="4" height="14" rx="1" fill="currentColor" />
          <rect x="14" y="5" width="4" height="14" rx="1" fill="currentColor" />
        {:else}
          <path d="M8 5.2v13.6a.8.8 0 0 0 1.2.7l10.6-6.8a.8.8 0 0 0 0-1.4L9.2 4.5A.8.8 0 0 0 8 5.2z" fill="currentColor" />
        {/if}
      </svg>
      <b>{running ? "Anhalten" : done ? "Alles fertig" : "Jetzt mitrechnen"}</b>
    </span>
  </button>

  <div class="say" aria-live="polite">
    <p class="head" class:live={running}>
      {words[0]}
      {#if spot && (worker.state === "solving" || worker.state === "cloud")}
        <span class="spot">{spot}{#if pct !== null}&nbsp;<span class="pct num">–&nbsp;{pct}&nbsp;%</span>{/if}</span>
      {/if}
    </p>
    {#if worker.state === "solving"}
      <div class="track" aria-hidden="true"><i style="width:{Math.max(2, (worker.progress ?? 0) * 100)}%"></i></div>
    {/if}
    {#if words[1]}<p class="sub">{words[1]}</p>{/if}
    {#if error}<p class="err">{error}</p>{/if}
  </div>
</div>

<div class="power">
  <span class="label" id="power-label">Kraft</span>
  <div class="seg" role="radiogroup" aria-labelledby="power-label">
    {#each POWERS as p (p.id)}
      <button role="radio" aria-checked={data.power === p.id} class:on={data.power === p.id} onclick={() => setPower(p.id)}>{p.label}</button>
    {/each}
  </div>
  <p>
    <b class="num">{threadsFor(data.power, data.cores)} von {data.cores}</b> Rechenkernen. {power.text}
    <span class="faint">Gilt ab dem nächsten Spot; Arbeitsspeicher höchstens ein Viertel.</span>
  </p>
</div>

<style>
  .run {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    gap: 20px;
    align-items: center;
  }
  /* The button is its own progress ring: the current spot fills it while it runs. */
  .go {
    --p: 0;
    display: grid;
    place-items: center;
    width: 124px;
    height: 124px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    cursor: pointer;
    background: conic-gradient(var(--pos) calc(var(--p) * 100%), var(--surface-3) 0);
    transition: transform 0.12s;
  }
  .go:not(:disabled):hover {
    transform: scale(1.03);
  }
  .go:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: 3px;
  }
  .go:disabled {
    cursor: default;
    opacity: 0.55;
  }
  .face {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 6px;
    width: 112px;
    height: 112px;
    border-radius: 50%;
    padding: 0 14px;
    background: var(--pos);
    color: #0f1a08;
    text-align: center;
  }
  .face b {
    font-size: 13.5px;
    line-height: 1.2;
    font-weight: 650;
  }
  .running .face {
    background: var(--surface);
    color: var(--text);
  }
  .say {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
  }
  .head {
    margin: 0;
    font-size: 16px;
    font-weight: 600;
    line-height: 1.4;
  }
  .head.live {
    color: var(--pos);
  }
  .spot {
    color: var(--text);
    font-weight: 500;
  }
  .pct {
    white-space: nowrap;
    color: var(--pos);
    font-weight: 600;
  }
  .sub {
    margin: 0;
    color: var(--text-2);
    font-size: 12.5px;
    line-height: 1.5;
    max-width: 70ch;
  }
  .err {
    margin: 0;
    color: #ff9d93;
    font-size: 12.5px;
  }
  .track {
    height: 6px;
    max-width: 560px;
    overflow: hidden;
    border-radius: 99px;
    background: var(--surface-3);
  }
  .track i {
    display: block;
    height: 100%;
    background: var(--pos);
    transition: width 0.4s;
  }
  .power {
    display: grid;
    grid-template-columns: 76px auto minmax(0, 1fr);
    gap: 8px 14px;
    align-items: center;
  }
  .power .seg {
    justify-self: start;
  }
  .label {
    color: var(--text-2);
    font-size: 12.5px;
    font-weight: 600;
  }
  .power p {
    margin: 0;
    color: var(--text-2);
    font-size: 12.5px;
    line-height: 1.5;
  }
  .power b {
    color: var(--text);
    font-weight: 600;
  }
  @container (max-width: 1240px) {
    .power {
      grid-template-columns: 76px minmax(0, 1fr);
    }
    .power p {
      grid-column: 2;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .go,
    .track i {
      transition: none;
    }
  }
</style>
