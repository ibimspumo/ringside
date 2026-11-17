<script lang="ts">
  import { onMount } from "svelte";
  import { EFFORTS, coachApi, cost, duration, errorText, onProgress, tokens } from "./api";
  import type { CoachProgress, CoachReport, Estimate } from "./types";

  let {
    tournamentId,
    title,
    onDone,
    onClose,
    onSettings,
  }: {
    tournamentId: number;
    title: string;
    onDone: (r: CoachReport) => void;
    onClose: () => void;
    onSettings: () => void;
  } = $props();

  type State =
    | { kind: "preparing" }
    | { kind: "confirm"; estimate: Estimate }
    | { kind: "running"; estimate: Estimate; progress: CoachProgress | null }
    | { kind: "error"; message: string; noKey: boolean };

  let st = $state<State>({ kind: "preparing" });
  const TOOLS: Record<string, string> = {
    equity: "Equity",
    pot_odds: "Pot Odds",
    push_fold_nash: "Push/Fold",
    icm: "ICM",
    outs: "Outs",
  };
  let used = $state<string[]>([]);
  let cancelling = $state(false);

  async function prepareRun() {
    st = { kind: "preparing" };
    try {
      st = { kind: "confirm", estimate: await coachApi.prepare(tournamentId) };
    } catch (e) {
      const message = errorText(e);
      st = { kind: "error", message, noKey: message.includes("OpenRouter-Key") };
    }
  }

  async function send(estimate: Estimate) {
    used = [];
    cancelling = false;
    st = { kind: "running", estimate, progress: null };
    try {
      onDone(await coachApi.run(estimate.token, tournamentId));
    } catch (e) {
      st = { kind: "error", message: errorText(e), noKey: false };
    }
  }

  onMount(() => {
    prepareRun();
    const sub = onProgress((p) => {
      if (st.kind !== "running" || p.tournament_id !== tournamentId) return;
      st.progress = p;
      if (p.stage === "tool" && p.tool && !used.includes(p.tool)) used = [...used, p.tool];
    });
    return () => {
      sub.then((f) => f());
    };
  });

  const perM = (v: number | null) => (v == null ? "?" : v.toLocaleString("de-DE", { maximumFractionDigits: 3 }));
</script>

<section class="panel run" aria-live="polite">
  <div class="panel-head">
    <h2 class="panel-title">Analyse: {title}</h2>
    {#if st.kind !== "running"}
      <button class="btn ghost" onclick={onClose}>Schließen</button>
    {/if}
  </div>

  <div class="body">
    {#if st.kind === "preparing"}
      <p class="dim">Ringside sucht deine Entscheidungen heraus und rechnet die Spots vor. Push/Fold-Spots am ganzen Tisch brauchen dabei einige Sekunden …</p>
    {:else if st.kind === "confirm"}
      {@const e = st.estimate}
      <div class="facts">
        <div class="fact">
          <span class="label">Hände</span>
          <strong class="num">{e.hands_sent} <small>von {e.hands_total}</small></strong>
          <small>
            {e.hands_without_decision} nur gefoldet{e.hands_dropped ? `, ${e.hands_dropped} weniger wichtige weggelassen` : ""}
          </small>
        </div>
        <div class="fact">
          <span class="label">Vorab berechnet</span>
          <strong class="num">{e.spots}</strong>
          <small>Push/Fold- und All-in-Spots</small>
        </div>
        <div class="fact">
          <span class="label">Tokens, geschätzt</span>
          <strong class="num">{tokens(e.prompt_tokens + e.completion_tokens)}</strong>
          <small class="num">{perM(e.prompt_per_m)} $ / {perM(e.completion_per_m)} $ je 1 Mio. ein/aus</small>
        </div>
        <div class="fact cost">
          <span class="label">Kosten, geschätzt</span>
          <strong class="num">{e.cost_usd == null ? "unbekannt" : `ca. ${cost(e.cost_usd)}`}</strong>
          <small>{e.max_cost_usd == null ? "Preis des Modells nicht geladen" : `höchstens ${cost(e.max_cost_usd)}, dann bricht Ringside ab`}</small>
        </div>
      </div>
      {#if e.effort}
        {@const effort = EFFORTS.find((x) => x.id === e.effort)}
        <p class="effort">
          Gründlichkeit <b>{effort?.name}</b>: {effort?.hint}
          <button class="link" onclick={onSettings}>Ändern</button>
        </p>
      {/if}
      <p class="privacy">
        Gesendet werden an OpenRouter und das Modell <b class="num">{e.model}</b>: deine Hände ohne Namen (du bist „Du“, die
        Gegner „Spieler 1“, „Spieler 2“ …), Beträge in Big Blinds, Buy-in und Platz. Keine Spielernamen, keine Hand- oder
        Turniernummern, keine Dateipfade.
      </p>
      <div class="row">
        <button class="btn primary" onclick={() => send(e)}>Senden und analysieren</button>
        <button class="btn ghost" onclick={onClose}>Abbrechen</button>
      </div>
    {:else if st.kind === "running"}
      {@const p = st.progress}
      {@const live = p && (p.stage === "thinking" || p.stage === "writing" || p.stage === "calling")}
      <div class="progress">
        <div class="line">
          <span class="spinner" aria-hidden="true"></span>
          <span>{p?.message ?? "Verbindung zu OpenRouter …"}</span>
        </div>
        <div class="bar"><i style="width:{p ? Math.min(100, (p.step / 4) * 100) : 4}%"></i></div>
        <div class="stats">
          <span>Schritt <b class="num">{p?.step ?? 0}</b> von höchstens {p?.max_steps ?? 12}</span>
          <span>Schritt läuft seit <b class="num">{duration(p?.step_seconds ?? 0)}</b></span>
          <span>Analyse läuft seit <b class="num">{duration(p?.total_seconds ?? 0)}</b></span>
        </div>
        {#if live && p}
          <div class="stats now">
            <span>In diesem Schritt</span>
            {#if p.live_reasoning_tokens || p.live_output_tokens}
              <span>ca. <b class="num">{tokens(p.live_reasoning_tokens)}</b> Tokens Nachdenken</span>
              <span>ca. <b class="num">{tokens(p.live_output_tokens)}</b> Tokens Antwort</span>
            {:else}
              <span class="faint">Das Modell zeigt sein Nachdenken nicht; die Tokens dafür stehen erst nach dem Schritt fest.</span>
            {/if}
          </div>
        {/if}
        <div class="stats">
          <span><b class="num">{tokens((p?.prompt_tokens ?? 0) + (p?.completion_tokens ?? 0))}</b> Tokens abgerechnet</span>
          {#if p?.reasoning_tokens}
            <span>davon <b class="num">{tokens(p.reasoning_tokens)}</b> Nachdenken</span>
          {/if}
          <span><b class="num">{cost(p?.cost_usd ?? 0)}</b> bisher</span>
          <span><b class="num">{p?.tool_calls ?? 0}</b> {p?.tool_calls === 1 ? "Rechnung" : "Rechnungen"}</span>
        </div>
        {#if used.length}
          <div class="tools">
            {#each used as t (t)}<span class="tool" class:now={p?.stage === "tool" && p.tool === t}>{TOOLS[t] ?? t}</span>{/each}
          </div>
        {/if}
        {#if p?.idle_limit != null && p.idle_seconds >= 15}
          <p class="wait">
            Seit <span class="num">{duration(p.idle_seconds)}</span> keine Daten von OpenRouter. Nach
            <span class="num">{p.idle_limit} s</span> ohne Daten bricht Ringside den Schritt ab und versucht es neu.
          </p>
        {/if}
        <p class="faint">
          Jeder Schritt ist eine Anfrage an das Modell. Bei gründlichem Nachdenken kann ein Schritt mehrere Minuten
          dauern.
          {#if p?.idle_limit != null}
            Solange OpenRouter Daten schickt, wartet Ringside; erst nach {p.idle_limit} s ohne Daten bricht der Schritt
            ab.
          {:else if p}
            Dieses Modell liefert seine Antwort am Stück; Ringside wartet bis zu 10 Minuten je Schritt.
          {/if}
          Ringside rechnet jede Zahl selbst nach.
        </p>
      </div>
      <div class="row">
        <button
          class="btn"
          disabled={cancelling}
          onclick={() => {
            cancelling = true;
            coachApi.cancel();
          }}>{cancelling ? "Wird abgebrochen …" : "Abbrechen"}</button
        >
      </div>
    {:else}
      <p class="error">{st.message}</p>
      <div class="row">
        {#if st.noKey}
          <button class="btn primary" onclick={onSettings}>Zu den Einstellungen</button>
        {:else}
          <button class="btn" onclick={prepareRun}>Erneut versuchen</button>
        {/if}
        <button class="btn ghost" onclick={onClose}>Schließen</button>
      </div>
    {/if}
  </div>
</section>

<style>
  .run {
    border-color: var(--line-2);
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 12px 18px 18px;
  }
  p {
    margin: 0;
    line-height: 1.5;
  }
  .facts {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(160px, 1fr));
    border: 1px solid var(--line);
    border-radius: var(--r-sm);
  }
  .fact {
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding: 12px 14px;
    border-left: 1px solid var(--line);
    min-width: 0;
  }
  .fact:first-child {
    border-left: 0;
  }
  .label {
    font-size: 12.5px;
    color: var(--text-2);
  }
  .fact strong {
    font-size: 22px;
    font-weight: 600;
    line-height: 1.15;
  }
  .fact strong small {
    font-size: 13px;
    color: var(--text-3);
  }
  .fact > small {
    font-size: 12px;
    color: var(--text-3);
  }
  .effort {
    font-size: 13px;
    color: var(--text-2);
  }
  .effort b {
    color: var(--text);
    font-weight: 600;
  }
  .link {
    margin-left: 4px;
    padding: 0;
    border: 0;
    background: none;
    color: var(--text);
    font: inherit;
    text-decoration: underline;
    text-underline-offset: 2px;
    cursor: pointer;
  }
  .stats.now {
    color: var(--text-3);
  }
  .wait {
    font-size: 13px;
    color: var(--text);
  }
  .privacy {
    font-size: 13px;
    color: var(--text-2);
    max-width: 760px;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .progress {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .line {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 15px;
  }
  .spinner {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    border: 2px solid var(--surface-3);
    border-top-color: var(--text);
    animation: spin 0.9s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .bar {
    height: 4px;
    border-radius: 2px;
    background: var(--surface-3);
    overflow: hidden;
  }
  .bar i {
    display: block;
    height: 100%;
    background: var(--text);
    transition: width 0.4s;
  }
  .stats {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 18px;
    font-size: 13px;
    color: var(--text-2);
  }
  .stats b {
    color: var(--text);
    font-weight: 600;
  }
  .tools {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .tool {
    padding: 2px 9px;
    border-radius: 999px;
    border: 1px solid var(--line-2);
    font-size: 12px;
    color: var(--text-2);
  }
  .tool.now {
    background: var(--surface-3);
    color: var(--text);
  }
  .error {
    color: #ff9d93;
  }
  @media (prefers-reduced-motion: reduce) {
    .spinner {
      animation: none;
    }
  }
</style>
