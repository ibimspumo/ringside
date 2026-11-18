<script lang="ts">
  import { onMount } from "svelte";
  import { EFFORTS, coachApi, cost, duration, errorText, onProgress, preview, tokens } from "./api";
  import { shortModel } from "./meaning";
  import type { CoachProgress, CoachReport, Effort, Estimate } from "./types";

  let {
    tournamentId,
    title,
    onDone,
    onClose,
    onSettings,
    onRunning,
  }: {
    tournamentId: number;
    title: string;
    onDone: (r: CoachReport) => void;
    onClose: () => void;
    onSettings: () => void;
    /** Whether an analysis is being sent right now; only one can run at a time. */
    onRunning?: (running: boolean) => void;
  } = $props();

  type State =
    | { kind: "preparing" }
    | { kind: "confirm"; estimate: Estimate }
    | { kind: "running"; estimate: Estimate; progress: CoachProgress | null }
    | { kind: "error"; message: string; noKey: boolean };

  let st = $state<State>({ kind: "preparing" });
  $effect(() => onRunning?.(st.kind === "running"));
  const TOOLS: Record<string, string> = {
    equity: "Equity",
    pot_odds: "Pot Odds",
    push_fold_nash: "Push/Fold",
    icm: "ICM",
    outs: "Outs",
  };
  const STEPS = ["Hände auswählen", "Kosten bestätigen", "Analyse läuft"];
  const stepOf = (k: State["kind"]) => (k === "preparing" ? 0 : k === "confirm" ? 1 : k === "running" ? 2 : -1);
  let used = $state<string[]>([]);
  let cancelling = $state(false);

  async function prepareRun() {
    st = { kind: "preparing" };
    try {
      const estimate = await coachApi.prepare(tournamentId);
      st = { kind: "confirm", estimate };
      if (preview.run === "running" || preview.run === "stalled") send(estimate);
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

  /** The estimate depends on the thoroughness, so a change prepares anew. */
  async function setEffort(e: Effort) {
    await coachApi.setEffort(e);
    prepareRun();
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
  const share = (a: number, b: number) => `${b > 0 ? Math.max(2, Math.min(100, (a / b) * 100)) : 0}%`;
</script>

<section class="panel run" aria-labelledby="run-title">
  <header class="head">
    <div class="titles">
      <p class="eyebrow">Analyse</p>
      <h2 id="run-title">{title}</h2>
    </div>
    {#if st.kind !== "error"}
      <ol class="stepper" aria-label="Ablauf">
        {#each STEPS as s, i (s)}
          <li class:done={i < stepOf(st.kind)} class:now={i === stepOf(st.kind)} aria-current={i === stepOf(st.kind) ? "step" : undefined}>
            <span class="dot" aria-hidden="true"
              >{#if i < stepOf(st.kind)}<svg viewBox="0 0 24 24" width="9" height="9"><path d="M5 12.5l4.5 4.5L19 7.5" /></svg
                >{/if}</span
            >{s}
          </li>
        {/each}
      </ol>
    {/if}
    {#if st.kind !== "running"}
      <button class="btn ghost close" onclick={onClose} aria-label="Analyse schließen" title="Schließen">
        <svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true"><path d="M6 6l12 12M18 6 6 18" /></svg>
      </button>
    {/if}
  </header>

  <div class="body" aria-live="polite">
    {#if st.kind === "preparing"}
      <div class="line">
        <span class="spinner" aria-hidden="true"></span>
        <span>Ringside sucht deine Entscheidungen heraus und rechnet die Spots vor …</span>
      </div>
      <p class="faint">Push/Fold-Spots am ganzen Tisch brauchen dabei einige Sekunden. Noch wird nichts gesendet.</p>
      <div class="skeleton" aria-hidden="true"><i></i><i></i></div>
    {:else if st.kind === "confirm"}
      {@const e = st.estimate}
      <div class="cards">
        <div class="card money">
          <span class="label">Das kostet die Analyse</span>
          <strong class="num">{e.cost_usd == null ? "unbekannt" : `ca. ${cost(e.cost_usd)}`}</strong>
          {#if e.cost_usd != null && e.max_cost_usd != null}
            <div class="cap" aria-hidden="true">
              <i style="width:{share(e.cost_usd, e.max_cost_usd)}"></i>
            </div>
            <span class="note">
              Deckel <b class="num">{cost(e.max_cost_usd)}</b>: Wird es teurer, bricht Ringside ab. Bezahlt wird direkt bei
              OpenRouter.
            </span>
          {:else}
            <span class="note">Der Preis des Modells ist nicht geladen, deshalb gibt es keine Schätzung.</span>
          {/if}
        </div>
        <div class="card">
          <span class="label">Das sieht sich der Coach an</span>
          <strong class="num">{e.hands_sent} <small>von {e.hands_total} Händen</small></strong>
          <div class="cap hands" aria-hidden="true"><i style="width:{share(e.hands_sent, e.hands_total)}"></i></div>
          <span class="note">
            Die <span class="num">{e.hands_without_decision}</span> Hände, die du nur gefoldet hast, bleiben hier{e.hands_dropped
              ? `, ${e.hands_dropped} weniger wichtige lässt Ringside weg`
              : ""}. <span class="num">{e.spots}</span> Push/Fold- und All-in-Spots hat Ringside schon vorgerechnet.
          </span>
        </div>
      </div>

      {#if e.effort}
        {@const current = EFFORTS.find((x) => x.id === e.effort)}
        <div class="effort">
          <span class="label" id="effort-run">Wie gründlich?</span>
          <div class="seg" role="radiogroup" aria-labelledby="effort-run">
            {#each [...EFFORTS].reverse() as x (x.id)}
              <button role="radio" aria-checked={e.effort === x.id} class:on={e.effort === x.id} onclick={() => setEffort(x.id)}
                >{x.name}</button
              >
            {/each}
          </div>
          <span class="hint">{current?.hint} Die Kosten oben passen sich an.</span>
        </div>
      {/if}

      <div class="privacy">
        <span class="label">Das geht raus – ohne Namen</span>
        <ul class="swaps">
          <li><s>Dein Name</s><span aria-hidden="true">→</span><b>„Du“</b></li>
          <li><s>Gegner</s><span aria-hidden="true">→</span><b>„Spieler 1“, „Spieler 2“ …</b></li>
          <li><s>Chips</s><span aria-hidden="true">→</span><b>Big Blinds</b></li>
          <li class="plain">dazu Buy-in und Platz</li>
        </ul>
        <p class="never">
          Nie dabei: Spielernamen, Hand- und Turniernummern, Dateipfade. Empfänger ist OpenRouter mit dem Modell
          <b class="num nowrap">{shortModel(e.model)}</b>
          <button class="link" onclick={onSettings}>Modell ändern</button>
        </p>
      </div>

      <div class="row">
        <button class="btn primary send" onclick={() => send(e)}>
          Senden und analysieren{e.cost_usd != null ? ` · ca. ${cost(e.cost_usd)}` : ""}
        </button>
        <button class="btn ghost" onclick={onClose}>Abbrechen</button>
        <details class="tech">
          <summary>Tokens und Preise</summary>
          <p>
            Geschätzt <b class="num">{tokens(e.prompt_tokens + e.completion_tokens)}</b> Tokens, höchstens
            <span class="num">{tokens(e.max_prompt_tokens + e.max_completion_tokens)}</span>. Preis
            <span class="num">{perM(e.prompt_per_m)} $</span> ein und <span class="num">{perM(e.completion_per_m)} $</span> aus je
            1 Mio. Tokens.
          </p>
        </details>
      </div>
    {:else if st.kind === "running"}
      {@const p = st.progress}
      {@const max = st.estimate.max_cost_usd}
      {@const live = p && (p.stage === "thinking" || p.stage === "writing" || p.stage === "calling")}
      <div class="now-row">
        <div class="line big">
          <span class="spinner" aria-hidden="true"></span>
          <span>{p?.message ?? "Verbindung zu OpenRouter …"}</span>
        </div>
        <div class="track" aria-label="Schritt {p?.step ?? 1} von höchstens {p?.max_steps ?? 12}">
          {#each Array.from({ length: Math.max(1, p?.step ?? 1) }, (_, k) => k) as k (k)}
            <i class:now={k === Math.max(1, p?.step ?? 1) - 1}></i>
          {/each}
          <span>Schritt <b class="num">{p?.step ?? 1}</b> von höchstens {p?.max_steps ?? 12}</span>
        </div>
      </div>
      <div class="figs">
        <div>
          <span class="label">Läuft seit</span>
          <b class="num">{duration(p?.total_seconds ?? 0)}</b>
          <span class="note">dieser Schritt seit <span class="num">{duration(p?.step_seconds ?? 0)}</span></span>
        </div>
        <div>
          <span class="label">Bisher bezahlt</span>
          <b class="num">{cost(p?.cost_usd ?? 0)}</b>
          <span class="note">{max != null ? `Deckel ${cost(max)}` : "kein Deckel bekannt"}</span>
        </div>
        <div>
          <span class="label">Ringside hat gerechnet</span>
          <b class="num">{p?.tool_calls ?? 0}-mal</b>
          {#if used.length}
            <span class="tools">
              {#each used as t (t)}{@const now = p?.stage === "tool" && p.tool === t}<span class="tool" class:now
                  >{#if !now}<svg viewBox="0 0 24 24" width="11" height="11" aria-hidden="true"><path d="M5 12.5l4.5 4.5L19 7.5" /></svg
                    >{/if}{TOOLS[t] ?? t}</span
                >{/each}
            </span>
          {:else}
            <span class="note">Jede Zahl rechnet Ringside nach.</span>
          {/if}
        </div>
      </div>
      {#if p?.idle_limit != null && p.idle_seconds >= 15}
        <p class="wait" role="status">
          Seit <b class="num">{duration(p.idle_seconds)}</b> keine Daten von OpenRouter. Nach
          <span class="num">{p.idle_limit} s</span> ohne Daten bricht Ringside den Schritt ab und versucht es neu. Du musst
          nichts tun.
        </p>
      {/if}
      <p class="explain">
        Jeder Schritt ist eine Anfrage an das Modell; bei gründlichem Nachdenken kann einer mehrere Minuten dauern.
        {#if p?.idle_limit != null}
          Solange Daten kommen, wartet Ringside.
        {:else if p}
          Dieses Modell liefert seine Antwort am Stück; Ringside wartet bis zu 10 Minuten je Schritt.
        {/if}
      </p>
      <div class="row">
        <button
          class="btn"
          disabled={cancelling}
          onclick={() => {
            cancelling = true;
            coachApi.cancel();
          }}>{cancelling ? "Wird abgebrochen …" : "Analyse abbrechen"}</button
        >
        <details class="tech">
          <summary>Technische Details</summary>
          <p>
            {#if live && p}
              {#if p.live_reasoning_tokens || p.live_output_tokens}
                In diesem Schritt ca. <span class="num">{tokens(p.live_reasoning_tokens)}</span> Tokens Nachdenken und
                <span class="num">{tokens(p.live_output_tokens)}</span> Tokens Antwort.
              {:else}
                Das Modell zeigt sein Nachdenken nicht; die Tokens dafür stehen erst nach dem Schritt fest.
              {/if}
            {/if}
            Abgerechnet bisher <span class="num">{tokens((p?.prompt_tokens ?? 0) + (p?.completion_tokens ?? 0))}</span>
            Tokens{#if p?.reasoning_tokens}, davon <span class="num">{tokens(p.reasoning_tokens)}</span> Nachdenken{/if}.
          </p>
        </details>
      </div>
    {:else}
      <div class="error" role="alert">
        <svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true"><circle cx="12" cy="12" r="9" /><path d="M12 7.5v5.5M12 16.2v.3" /></svg>
        <div>
          <b>{st.noKey ? "Es fehlt noch der Key von OpenRouter." : "Die Analyse hat nicht geklappt."}</b>
          <p>{st.message}</p>
        </div>
      </div>
      <div class="row">
        {#if st.noKey}
          <button class="btn primary" onclick={onSettings}>Key hinterlegen</button>
        {:else}
          <button class="btn primary" onclick={prepareRun}>Erneut versuchen</button>
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
  .head {
    display: flex;
    align-items: center;
    gap: 12px 24px;
    flex-wrap: wrap;
    padding: 18px 22px 0;
  }
  .titles {
    min-width: 0;
    margin-right: auto;
  }
  .eyebrow {
    margin: 0;
    font-size: 12.5px;
    color: var(--text-2);
  }
  h2 {
    margin: 0;
    font-family: var(--font-num);
    font-size: 20px;
    font-weight: 600;
  }
  .stepper {
    list-style: none;
    display: flex;
    gap: 18px;
    margin: 0;
    padding: 0;
    font-size: 12.5px;
    color: var(--text-2);
  }
  .stepper li {
    display: flex;
    align-items: center;
    gap: 7px;
    white-space: nowrap;
  }
  .stepper .dot {
    display: grid;
    place-items: center;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    border: 1.5px solid var(--text-3);
  }
  .stepper .dot svg {
    stroke: var(--bg);
    stroke-width: 3.5;
  }
  .stepper .done {
    color: var(--text-2);
  }
  .stepper .done .dot {
    background: var(--pos);
    border-color: var(--pos);
  }
  .stepper .now {
    color: var(--text);
    font-weight: 600;
  }
  .stepper .now .dot {
    border-color: var(--text);
    box-shadow: 0 0 0 3px rgba(236, 235, 231, 0.12);
  }
  .close {
    width: 32px;
    padding: 0;
  }
  svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linecap: round;
  }

  .body {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 18px 22px 22px;
  }
  p {
    margin: 0;
    line-height: 1.5;
  }
  .label {
    font-size: 12.5px;
    color: var(--text-2);
  }
  .note {
    font-size: 12.5px;
    line-height: 1.5;
    color: var(--text-2);
  }
  .explain {
    font-size: 13px;
    color: var(--text-2);
  }
  .note b {
    color: var(--text);
    font-weight: 600;
  }

  .cards {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 12px;
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 16px 18px;
    border-radius: var(--r-sm);
    background: var(--bg);
    border: 1px solid var(--line);
    min-width: 0;
  }
  .card strong {
    font-size: 30px;
    font-weight: 600;
    line-height: 1.1;
  }
  .card strong small {
    font-size: 14px;
    font-weight: 400;
    color: var(--text-2);
  }
  .cap {
    height: 6px;
    margin: 4px 0 2px;
    border-radius: 3px;
    background: var(--surface-3);
    overflow: hidden;
  }
  .cap i {
    display: block;
    height: 100%;
    border-radius: 3px;
    background: var(--text);
  }
  .cap.hands i {
    background: var(--text-2);
  }
  .money .cap {
    position: relative;
    overflow: visible;
  }
  .money .cap::after {
    content: "";
    position: absolute;
    right: -1px;
    top: -4px;
    bottom: -4px;
    width: 2px;
    border-radius: 1px;
    background: var(--amber);
  }

  .effort {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 14px;
  }
  .hint {
    flex: 1 1 260px;
    font-size: 12.5px;
    color: var(--text-2);
  }

  .privacy {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding-top: 14px;
    border-top: 1px solid var(--line);
  }
  .swaps {
    list-style: none;
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin: 0;
    padding: 0;
  }
  .swaps li {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 4px 10px;
    border-radius: 999px;
    background: var(--surface-2);
    font-size: 12.5px;
  }
  .swaps s {
    color: var(--text-2);
    text-decoration-color: var(--text-3);
  }
  .swaps span {
    color: var(--text-2);
  }
  .swaps b {
    font-weight: 600;
  }
  .swaps .plain {
    background: none;
    color: var(--text-2);
  }
  .never {
    font-size: 12.5px;
    color: var(--text-2);
  }
  .never b {
    color: var(--text);
  }
  .nowrap {
    white-space: nowrap;
  }
  .link {
    margin-left: 6px;
    padding: 0;
    border: 0;
    background: none;
    color: var(--text-2);
    font: inherit;
    text-decoration: underline;
    text-decoration-color: var(--line-2);
    text-underline-offset: 3px;
    cursor: pointer;
  }
  .link:hover {
    color: var(--text);
  }

  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
  .send {
    height: 38px;
    padding: 0 18px;
  }
  .tech {
    margin-left: auto;
    font-size: 12.5px;
    color: var(--text-2);
  }
  .tech summary {
    cursor: pointer;
    text-align: right;
  }
  .tech p {
    max-width: 420px;
    margin-top: 6px;
    font-size: 12.5px;
  }

  .line {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .line.big {
    font-size: 16px;
  }
  .spinner {
    width: 14px;
    height: 14px;
    flex-shrink: 0;
    border-radius: 50%;
    border: 2px solid var(--surface-3);
    border-top-color: var(--text);
    animation: spin var(--dur-loop, 900ms) var(--ease-spin, cubic-bezier(0.45, 0.05, 0.55, 0.95)) infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .skeleton {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 12px;
  }
  .skeleton i {
    height: 112px;
    border-radius: var(--r-sm);
    background: var(--bg);
    border: 1px solid var(--line);
    animation: pulse 1.6s var(--ease-standard, cubic-bezier(0.2, 0, 0, 1)) infinite;
  }
  @keyframes pulse {
    50% {
      opacity: 0.5;
    }
  }
  .now-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 10px 20px;
    padding-bottom: 14px;
    border-bottom: 1px solid var(--line);
  }
  .track {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: 12.5px;
    color: var(--text-2);
  }
  .track i {
    width: 18px;
    height: 6px;
    border-radius: 3px;
    background: var(--text-2);
  }
  .track i.now {
    background: var(--text);
    animation: pulse 1.6s var(--ease-standard, cubic-bezier(0.2, 0, 0, 1)) infinite;
  }
  .track span {
    margin-left: 6px;
  }
  .track b {
    color: var(--text);
    font-weight: 600;
  }
  .figs {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 12px;
  }
  .figs > div {
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding: 14px 16px;
    border-radius: var(--r-sm);
    background: var(--bg);
    border: 1px solid var(--line);
    min-width: 0;
  }
  .figs b {
    font-size: 24px;
    font-weight: 600;
    line-height: 1.15;
  }
  .tools {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin-top: 2px;
  }
  .tool {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 1px 8px;
    border-radius: 999px;
    border: 1px solid var(--line-2);
    font-size: 12.5px;
    color: var(--text-2);
  }
  .tool {
    transition:
      background var(--dur-fast, 120ms) var(--ease-standard, cubic-bezier(0.2, 0, 0, 1)),
      color var(--dur-fast, 120ms) var(--ease-standard, cubic-bezier(0.2, 0, 0, 1));
  }
  .tool svg {
    fill: none;
    stroke: var(--pos);
    stroke-width: 2.4;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .tool.now {
    background: var(--text);
    border-color: var(--text);
    color: var(--bg);
    font-weight: 600;
  }
  .wait {
    padding: 10px 14px;
    border-radius: var(--r-sm);
    border: 1px solid rgba(242, 177, 46, 0.35);
    background: rgba(242, 177, 46, 0.07);
    font-size: 13px;
  }
  .wait b {
    color: var(--amber);
  }

  .error {
    display: flex;
    gap: 12px;
    padding: 14px 16px;
    border-radius: var(--r-sm);
    border: 1px solid rgba(229, 72, 59, 0.35);
    background: rgba(229, 72, 59, 0.06);
  }
  .error svg {
    flex-shrink: 0;
    color: #ff9d93;
    margin-top: 1px;
  }
  .error div {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .error p {
    color: var(--text-2);
  }

  @container (max-width: 760px) {
    .cards,
    .figs,
    .skeleton {
      grid-template-columns: minmax(0, 1fr);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .spinner,
    .skeleton i,
    .track i.now {
      animation: none;
    }
  }
</style>
