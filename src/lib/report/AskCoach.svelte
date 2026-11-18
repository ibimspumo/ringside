<script lang="ts">
  import { onMount } from "svelte";
  import { coachApi, cost, errorText } from "../coach/api";
  import type { CoachStatus } from "../coach/types";
  import Button from "../ui/Button.svelte";
  import type { AskAbout, AskAnswer, AskEstimate } from "./types";

  /**
   * "Frag den Coach": an optional question about figures Ringside already computed. Without a
   * key it only says how to set one up; with one it shows the cost before anything is sent.
   */
  let {
    about,
    presets,
    onSetup,
    compact = false,
  }: {
    about: AskAbout;
    /** Questions offered with one click. */
    presets: string[];
    /** Opens the coach's setup (key, model). */
    onSetup: () => void;
    compact?: boolean;
  } = $props();

  let status = $state<CoachStatus | null>(null);
  let open = $state(false);
  let question = $state("");
  let estimate = $state<AskEstimate | null>(null);
  let answer = $state<AskAnswer | null>(null);
  let busy = $state(false);
  let failed = $state<string | null>(null);

  onMount(() => {
    coachApi.status().then(
      (s) => (status = s),
      () => (status = null),
    );
  });

  async function check(q: string) {
    question = q;
    failed = null;
    answer = null;
    busy = true;
    try {
      estimate = await coachApi.askPrepare(about, q);
    } catch (e) {
      failed = errorText(e);
    } finally {
      busy = false;
    }
  }

  async function send() {
    if (!estimate) return;
    failed = null;
    busy = true;
    try {
      answer = await coachApi.ask(estimate.token, about);
      estimate = null;
    } catch (e) {
      failed = errorText(e);
      estimate = null;
    } finally {
      busy = false;
    }
  }

  function reset() {
    estimate = null;
    answer = null;
    failed = null;
    question = "";
  }
</script>

{#snippet bubble()}<svg class="icon" viewBox="0 0 24 24" width="15" height="15" aria-hidden="true"
    ><path d="M4.5 5.5h15v10.5h-8l-4.5 3.5v-3.5H4.5zM8.5 9.5h7M8.5 12.5h4.5" /></svg
  >{/snippet}

<section class="ask" class:compact aria-label="Frag den Coach">
  {#if !open}
    <div class="closed">
      <Button size="sm" variant="quiet" icon={bubble} onclick={() => (open = true)} aria-expanded="false">Frag den Coach</Button>
      <span class="hint">Optional: Ein Sprachmodell erklärt dir diese Zahlen in Alltagssprache. Es rechnet nichts selbst.</span>
    </div>
  {:else}
    <div class="head">
      <span class="title">{@render bubble()} Frag den Coach</span>
      <Button size="sm" variant="quiet" onclick={() => (open = false)} aria-expanded="true">Schließen</Button>
    </div>
    {#if status && !status.has_key}
      <p class="line">
        Für den Coach brauchst du einen eigenen Key von OpenRouter. Du zahlst dort wenige Cent je Frage; ohne Key bleibt alles
        hier kostenlos.
      </p>
      <div class="row"><Button size="sm" onclick={onSetup}>KI-Coach einrichten</Button></div>
    {:else if answer}
      <div class="answer" role="status">
        {#each answer.text.split(/\n\s*\n/) as p, i (i)}<p>{p}</p>{/each}
      </div>
      {#if answer.unchecked.length}
        <p class="warn">
          Diese Zahlen stehen nicht in Ringsides Rechnung: <b class="num">{answer.unchecked.join(", ")}</b>. Verlass dich auf die Zahlen im
          Bericht.
        </p>
      {/if}
      <p class="meta">Antwort von {answer.model} · hat {cost(answer.cost_usd)} gekostet</p>
      <div class="row"><Button size="sm" onclick={reset}>Neue Frage</Button></div>
    {:else if estimate}
      <p class="line">„{estimate.question}“</p>
      <p class="price">
        Kostet etwa <b class="num">{cost(estimate.cost_usd)}</b>{#if estimate.max_cost_usd !== null}, höchstens
          <b class="num">{cost(estimate.max_cost_usd)}</b>{/if} · {estimate.model}
      </p>
      <details class="sent">
        <summary>Was gesendet wird (ohne Namen)</summary>
        <pre>{estimate.sent}</pre>
      </details>
      <div class="row">
        <Button size="sm" variant="primary" loading={busy} onclick={send}>Senden</Button>
        <Button size="sm" variant="quiet" disabled={busy} onclick={() => (estimate = null)}>Abbrechen</Button>
      </div>
    {:else}
      <div class="presets">
        {#each presets as p (p)}
          <Button size="sm" variant="secondary" disabled={busy || !status} onclick={() => check(p)}>{p}</Button>
        {/each}
      </div>
      <form
        class="own"
        onsubmit={(e) => {
          e.preventDefault();
          if (question.trim()) check(question.trim());
        }}
      >
        <input class="field" placeholder="Eigene Frage, z. B. „Warum ist das ein Fold?“" aria-label="Eigene Frage an den Coach" maxlength="600" bind:value={question} />
        <Button size="sm" type="submit" loading={busy} disabled={busy || !question.trim() || !status}>Kosten prüfen</Button>
      </form>
      <p class="meta">Vor dem Senden siehst du, was es kostet und was gesendet wird.</p>
    {/if}
    {#if failed}<p class="warn" role="alert">{failed}</p>{/if}
  {/if}
</section>

<style>
  .ask {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }
  .closed {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-1) var(--sp-3);
  }
  .hint,
  .meta {
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .meta {
    margin: 0;
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-2);
  }
  .title {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-2);
    font-weight: var(--fw-semibold);
  }
  .icon {
    fill: none;
    stroke: currentColor;
    stroke-width: 1.8;
    stroke-linejoin: round;
    stroke-linecap: round;
  }
  .line,
  .price {
    margin: 0;
    font-size: var(--fs-md);
    line-height: var(--lh-base);
    color: var(--text-2);
  }
  .price b {
    color: var(--text);
  }
  .row,
  .presets {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-2);
  }
  .own {
    display: flex;
    gap: var(--sp-2);
  }
  .own input {
    flex: 1;
    min-width: 0;
  }
  .answer {
    padding: var(--sp-3) var(--sp-4);
    border-left: 2px solid var(--line-2);
    background: var(--surface-sunken);
    border-radius: 0 var(--r-sm) var(--r-sm) 0;
  }
  .answer p {
    margin: 0;
    line-height: var(--lh-base);
  }
  .answer p + p {
    margin-top: var(--sp-2);
  }
  .warn {
    margin: 0;
    font-size: var(--fs-md);
    color: var(--amber);
  }
  .sent summary {
    cursor: pointer;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .sent pre {
    max-height: 180px;
    overflow: auto;
    margin: var(--sp-2) 0 0;
    padding: var(--sp-2) var(--sp-3);
    border-radius: var(--r-sm);
    background: var(--surface-sunken);
    font-size: var(--fs-xs);
    white-space: pre-wrap;
  }
  .compact .hint {
    display: none;
  }
</style>
