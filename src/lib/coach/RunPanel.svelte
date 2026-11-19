<script lang="ts">
  import { onMount } from "svelte";
  import Badge from "../ui/Badge.svelte";
  import Button from "../ui/Button.svelte";
  import Card from "../ui/Card.svelte";
  import EmptyState from "../ui/EmptyState.svelte";
  import IconButton from "../ui/IconButton.svelte";
  import ProgressBar from "../ui/ProgressBar.svelte";
  import ProgressRing from "../ui/ProgressRing.svelte";
  import SegmentedControl from "../ui/SegmentedControl.svelte";
  import Stat from "../ui/Stat.svelte";
  import { EFFORTS, coachApi, cost, errorText, onProgress, preview, tokens } from "./api";
  import { duration } from "../ui/format";
  import { money, shortModel } from "./meaning";
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
    | { kind: "error"; message: string; noKey: boolean; paid?: number; at: number };

  let st = $state<State>({ kind: "preparing" });
  $effect(() => onRunning?.(st.kind === "running"));
  const TOOLS: Record<string, string> = {
    equity: "Equity",
    pot_odds: "Pot Odds",
    push_fold_nash: "Push/Fold",
    icm: "ICM",
    outs: "Outs",
  };
  const STEPS = ["Hände vorbereiten", "Kosten bestätigen", "Analyse läuft"];
  const stepOf = (s: State) => (s.kind === "preparing" ? 0 : s.kind === "confirm" ? 1 : s.kind === "running" ? 2 : s.at);
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
      st = { kind: "error", message, noKey: message.includes("OpenRouter-Key"), at: 0 };
    }
  }

  async function send(estimate: Estimate) {
    used = [];
    cancelling = false;
    st = { kind: "running", estimate, progress: null };
    try {
      onDone(await coachApi.run(estimate.token, tournamentId));
    } catch (e) {
      const paid = st.kind === "running" ? (st.progress?.cost_usd ?? 0) : 0;
      st = { kind: "error", message: errorText(e), noKey: false, paid, at: 2 };
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
      if ((p.stage === "tool" || p.stage === "calling") && p.tool && p.tool in TOOLS && !used.includes(p.tool)) used = [...used, p.tool];
    });
    return () => {
      sub.then((f) => f());
    };
  });

  const perM = (v: number | null) => (v == null ? "?" : v.toLocaleString("de-DE", { maximumFractionDigits: 3 }));
  /** Calculations Ringside has finished; the one running right now does not count yet. */
  const finished = (p: CoachProgress | null) => Math.max(0, (p?.tool_calls ?? 0) - (p?.stage === "tool" ? 1 : 0));
  const effortOptions = [...EFFORTS].reverse().map((x) => ({ value: x.id, label: x.name }));
</script>

{#snippet closeIcon()}<svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true"><path d="M6 6l12 12M18 6 6 18" /></svg>{/snippet}

<Card pad="lg" tone="raised" aria-labelledby="run-title">
  <header class="head">
    <div class="titles">
      <p class="eyebrow">Analyse</p>
      <h2 id="run-title">{title}</h2>
    </div>
    <ol class="stepper" class:failed={st.kind === "error"} aria-label="Ablauf">
      {#each STEPS as s, i (s)}
        <li class:done={i < stepOf(st)} class:now={i === stepOf(st)} aria-current={i === stepOf(st) ? "step" : undefined}>
          <span class="dot" aria-hidden="true"
            >{#if i < stepOf(st)}<svg viewBox="0 0 24 24" width="9" height="9"><path d="M5 12.5l4.5 4.5L19 7.5" /></svg
              >{:else if st.kind === "error" && i === stepOf(st)}<svg viewBox="0 0 24 24" width="9" height="9"><path d="M7 7l10 10M17 7L7 17" /></svg
                >{/if}</span
          >{s}
        </li>
      {/each}
    </ol>
    <span class="close" class:hidden={st.kind === "running"}
      ><IconButton label="Analyse schließen" onclick={onClose} disabled={st.kind === "running"}>{@render closeIcon()}</IconButton></span
    >
  </header>

  <div class="body" aria-live="polite">
    {#if st.kind === "preparing"}
      <div class="line">
        <ProgressRing value={null} size={18} stroke={2} label="Ringside bereitet vor" />
        <span>Ringside sucht deine Entscheidungen heraus und rechnet die Spots vor …</span>
      </div>
      <p class="explain">Push/Fold-Spots am ganzen Tisch brauchen dabei einige Sekunden. Noch wird nichts gesendet.</p>
      <div class="skeleton" aria-hidden="true"><i></i><i></i></div>
    {:else if st.kind === "confirm"}
      {@const e = st.estimate}
      <div class="cards">
        <Card tone="sunken">
          <div class="fact">
            {#if e.cost_usd != null}
              <Stat
                size="lg"
                label="Das kostet die Analyse"
                value="ca. {money(e.cost_usd)}"
                meaning={e.max_cost_usd != null ? `höchstens ${money(e.max_cost_usd)}` : undefined}
              />
            {:else}
              <Stat size="lg" label="Das kostet die Analyse" value="unbekannt" />
            {/if}
            {#if e.cost_usd != null && e.max_cost_usd != null}
              <ProgressBar value={e.cost_usd / e.max_cost_usd} size="md" label="Geschätzte Kosten im Verhältnis zur Obergrenze" />
              <p class="note">Würde es teurer, bricht Ringside ab. Bezahlt wird direkt bei OpenRouter.</p>
            {:else}
              <p class="note">Der Preis des Modells ist nicht geladen, deshalb gibt es keine Schätzung.</p>
            {/if}
          </div>
        </Card>
        <Card tone="sunken">
          <div class="fact">
            <Stat size="lg" label="Das sieht sich der Coach an" value={String(e.hands_sent)} unit="von {e.hands_total} Händen" />
            <ProgressBar value={e.hands_total ? e.hands_sent / e.hands_total : 0} size="md" label="Gesendete Hände" />
            <p class="note">
              Die <span class="num">{e.hands_without_decision}</span> Hände, die du nur gefoldet hast, bleiben hier{e.hands_dropped
                ? `, ${e.hands_dropped} weniger wichtige lässt Ringside weg`
                : ""}. <span class="num">{e.spots}</span> Push/Fold- und All-in-Spots hat Ringside schon vorgerechnet.
            </p>
          </div>
        </Card>
      </div>

      {#if e.effort}
        {@const current = EFFORTS.find((x) => x.id === e.effort)}
        <div class="effort">
          <span class="label">Wie gründlich?</span>
          <SegmentedControl options={effortOptions} value={e.effort} label="Gründlichkeit" onchange={setEffort} />
          <span class="hint">{current?.hint} Die Kosten oben passen sich an.</span>
        </div>
      {/if}

      <div class="confirm-end">
        <div class="privacy">
          <span class="label">Das geht raus – ohne Namen</span>
          <ul class="swaps">
            <li><s>Dein Name</s><span aria-hidden="true">→</span><b>„Du“</b></li>
            <li><s>Gegner</s><span aria-hidden="true">→</span><b>„Spieler 1“, „Spieler 2“ …</b></li>
            <li><s>Chips</s><span aria-hidden="true">→</span><b>Big Blinds</b></li>
            <li class="plain">dazu Buy-in und Platz</li>
          </ul>
          <div class="never-row">
            <p class="never">
              Nie dabei: Spielernamen, Hand- und Turniernummern, Dateipfade. Empfänger ist OpenRouter mit dem Modell
              <b class="num nowrap">{shortModel(e.model)}</b>.
            </p>
          </div>
        </div>

        <div class="row send">
          <Button variant="primary" size="lg" onclick={() => send(e)}>
            Senden und analysieren{e.cost_usd != null ? ` · ca. ${money(e.cost_usd)}` : ""}
          </Button>
          <Button variant="quiet" onclick={onClose}>Abbrechen</Button>
          <Button variant="quiet" onclick={onSettings}>Modell ändern</Button>
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
      </div>
    {:else if st.kind === "running"}
      {@const p = st.progress}
      {@const max = st.estimate.max_cost_usd}
      {@const live = p && (p.stage === "thinking" || p.stage === "writing" || p.stage === "calling")}
      <div class="now-row">
        <div class="line big">
          <ProgressRing value={null} size={18} stroke={2} label="Analyse läuft" />
          <span>{p?.message ?? "Verbindung zu OpenRouter …"}</span>
        </div>
        <div class="track" aria-label="Schritt {p?.step ?? 1} von höchstens {p?.max_steps ?? 12}">
          {#each Array.from({ length: p?.max_steps ?? 12 }, (_, k) => k) as k (k)}
            <i class:done={k < Math.max(1, p?.step ?? 1) - 1} class:now={k === Math.max(1, p?.step ?? 1) - 1}></i>
          {/each}
          <span>Schritt <b class="num">{p?.step ?? 1}</b> von höchstens {p?.max_steps ?? 12}</span>
        </div>
      </div>
      <div class="figs">
        <Card tone="sunken">
          <Stat
            label="Läuft seit"
            value={duration(p?.total_seconds ?? 0)}
            meaning={(p?.step ?? 1) > 1 ? `dieser Schritt seit ${duration(p?.step_seconds ?? 0)}` : "erster Schritt"}
          />
        </Card>
        <Card tone="sunken">
          <div class="fact">
            <Stat label="Bisher bezahlt" value={p?.cost_usd ? money(p.cost_usd) : "noch nichts"} meaning={max != null ? `höchstens ${money(max)}` : "keine Obergrenze bekannt"} />
            {#if max != null}<ProgressBar value={(p?.cost_usd ?? 0) / max} label="Bezahlt im Verhältnis zur Obergrenze" />{/if}
          </div>
        </Card>
        <Card tone="sunken">
          <div class="fact">
            <Stat label="Ringside hat gerechnet" value={st.estimate.spots + finished(p) ? `${st.estimate.spots + finished(p)}-mal` : "noch nichts"}
              meaning={st.estimate.spots ? `${st.estimate.spots} vorab, ${finished(p)} während der Analyse` : undefined} />
          </div>
        </Card>
      </div>
      <div class="tools" aria-label="Was Ringside nachgerechnet hat">
        {#if used.length}<span class="note">Nachgerechnet:</span>{/if}
        {#each used as t (t)}{@const now = (p?.stage === "tool" || p?.stage === "calling") && p.tool === t}<Badge size="sm" tone={now ? "info" : "good"} dot={now}
            >{TOOLS[t] ?? t}{now ? " läuft" : " ✓"}</Badge
          >{:else}<span class="note">Jede Zahl im Bericht rechnet Ringside selbst nach.</span>{/each}
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
        <Button
          loading={cancelling}
          disabled={cancelling}
          onclick={() => {
            cancelling = true;
            coachApi.cancel();
          }}>{cancelling ? "Wird abgebrochen …" : "Analyse abbrechen"}</Button
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
      <EmptyState tone="error" title={st.noKey ? "Es fehlt noch der Key von OpenRouter." : "Die Analyse hat nicht geklappt."} text={`${st.message} Bisher bezahlt: ${st.paid ? money(st.paid) : "nichts"}.`}>
        {#snippet action()}
          {#if st.kind === "error" && st.noKey}
            <Button variant="primary" onclick={onSettings}>Key hinterlegen</Button>
          {:else}
            <Button variant="primary" onclick={prepareRun}>Erneut versuchen</Button>
          {/if}
          <Button variant="quiet" onclick={onClose}>Schließen</Button>
        {/snippet}
      </EmptyState>
    {/if}
  </div>
</Card>

<style>
  .head {
    display: flex;
    align-items: center;
    gap: var(--sp-3) var(--sp-6);
    flex-wrap: wrap;
  }
  .titles {
    min-width: 0;
    margin-right: auto;
  }
  @container (max-width: 1100px) {
    .close {
      order: 1;
    }
    .stepper {
      order: 2;
      flex-basis: 100%;
    }
  }
  .eyebrow {
    margin: 0;
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  h2 {
    margin: 0;
    font-family: var(--font-num);
    font-size: var(--fs-2xl);
    font-weight: var(--fw-semibold);
  }
  .stepper {
    list-style: none;
    display: flex;
    gap: var(--sp-5);
    margin: 0;
    padding: 0;
    font-size: var(--fs-base);
    color: var(--text-2);
  }
  .stepper li {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
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
    fill: none;
    stroke: var(--bg);
    stroke-width: 3.5;
    stroke-linecap: round;
  }
  .stepper .done .dot {
    background: var(--pos);
    border-color: var(--pos);
  }
  .stepper .now {
    color: var(--text);
    font-weight: var(--fw-semibold);
  }
  .stepper.failed .now .dot {
    border-color: var(--danger);
    background: var(--danger);
    box-shadow: none;
  }
  .stepper.failed .now {
    color: var(--danger-text);
  }
  .stepper .now .dot {
    border-color: var(--text);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--text) 12%, transparent);
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
    gap: var(--sp-4);
    margin-top: var(--sp-5);
  }
  p {
    margin: 0;
    line-height: 1.5;
  }
  .label {
    font-size: var(--fs-base);
    color: var(--text-2);
  }
  .note,
  .explain,
  .hint,
  .never {
    font-size: var(--fs-base);
    line-height: 1.5;
    color: var(--text-2);
  }
  .never b {
    color: var(--text);
    font-weight: var(--fw-semibold);
  }
  .nowrap {
    white-space: nowrap;
  }
  .never-row {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: var(--sp-1) var(--sp-3);
  }
  .never-row .never {
    flex: 1 1 360px;
  }
  .hidden {
    visibility: hidden;
  }

  .cards {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--sp-3);
  }
  .fact {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }

  .effort {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2) var(--sp-4);
  }
  .hint {
    flex: 1 1 260px;
  }

  .privacy {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    padding-top: var(--sp-4);
    border-top: 1px solid var(--line);
  }
  .swaps {
    list-style: none;
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-2);
    margin: 0;
    padding: 0;
  }
  .swaps li {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    padding: var(--sp-1) var(--sp-3);
    border-radius: var(--r-pill);
    background: var(--surface-2);
    font-size: var(--fs-base);
  }
  .swaps s,
  .swaps span {
    color: var(--text-2);
  }
  .swaps b {
    font-weight: var(--fw-semibold);
  }
  .swaps .plain {
    background: none;
    color: var(--text-2);
  }

  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2);
  }
  .send {
    padding-top: var(--sp-2);
  }
  .confirm-end {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
  }
  /* Narrow windows: the decision comes right after the costs, what is sent follows directly. */
  @container (max-width: 980px) {
    .confirm-end .send {
      order: -1;
      padding-top: 0;
    }
  }
  .tech {
    margin-left: auto;
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .tech summary {
    cursor: pointer;
    text-align: right;
  }
  .tech p {
    max-width: 420px;
    margin-top: var(--sp-2);
  }

  .line {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
  }
  .line.big {
    min-height: 1.5em;
    font-size: var(--fs-lg);
  }
  .line.big > span:last-child {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .skeleton {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--sp-3);
  }
  .skeleton i {
    height: 112px;
    border-radius: var(--r-md);
    background: var(--surface-sunken);
    border: 1px solid var(--line);
    animation: pulse var(--dur-loop) var(--ease-spin) infinite alternate;
  }
  @keyframes pulse {
    50% {
      opacity: 0.5;
    }
  }
  .now-row {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    padding-bottom: var(--sp-4);
    border-bottom: 1px solid var(--line);
  }
  .track {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .track i {
    width: 10px;
    height: 6px;
    border-radius: var(--r-pill);
    background: var(--surface-3);
  }
  .track i.done {
    background: var(--text-2);
  }
  .track i.now {
    background: var(--text);
    animation: pulse var(--dur-loop) var(--ease-spin) infinite alternate;
  }
  .track span {
    margin-left: 6px;
  }
  .track b {
    color: var(--text);
    font-weight: var(--fw-semibold);
  }
  .figs {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: var(--sp-3);
  }
  .tools {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    min-height: 22px;
    gap: var(--sp-1);
  }
  .wait {
    padding: var(--sp-3) var(--sp-4);
    border-radius: var(--r-sm);
    border: 1px solid color-mix(in srgb, var(--amber) 35%, transparent);
    background: color-mix(in srgb, var(--amber) 7%, transparent);
    font-size: var(--fs-md);
  }
  .wait b {
    color: var(--amber);
  }

  @container (max-width: 560px) {
    .cards,
    .figs,
    .skeleton {
      grid-template-columns: minmax(0, 1fr);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .skeleton i,
    .track i.now {
      animation: none;
    }
  }
</style>
