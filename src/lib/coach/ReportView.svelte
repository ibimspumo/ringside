<script lang="ts" module>
  import type { LearnTarget } from "../app/Leaks.svelte";

  /** Where a concept leads in the learning area. */
  export type Practice = { train: LearnTarget | null; article: { id: string; title: string } | null };
</script>

<script lang="ts">
  import PlayingCard from "../PlayingCard.svelte";
  import { cost, duration, tokens } from "./api";
  import { conceptName } from "./concepts";
  import { VERDICTS, shortModel, toPractise } from "./meaning";
  import SpotCard from "./SpotCard.svelte";
  import type { CoachReport } from "./types";

  let {
    report,
    title,
    practice,
    onHand,
    onTraining,
    onReanalyze,
    onDelete,
  }: {
    report: CoachReport;
    title: string;
    practice: (concept: string) => Practice;
    onHand: (id: number) => void;
    onTraining: (target: LearnTarget | null) => void;
    onReanalyze: () => void;
    onDelete: () => void;
  } = $props();

  type Show = "all" | "practise" | "good";
  let show = $state<Show>("all");
  let confirmDelete = $state(false);

  const spots = $derived(report.spots.map((spot, i) => ({ spot, n: i + 1 })));
  const weak = $derived(spots.filter((s) => toPractise(s.spot.verdict)));
  const shown = $derived(
    show === "all" ? spots : show === "practise" ? weak : spots.filter((s) => !toPractise(s.spot.verdict)),
  );
  const counts = $derived(VERDICTS.map((v) => ({ ...v, n: report.spots.filter((s) => s.verdict === v.id).length })));
  const created = $derived(
    new Date(report.meta.created_at * 1000).toLocaleString("de-DE", { dateStyle: "medium", timeStyle: "short" }),
  );
  /** The concept of the first spot to practise: the report's main lesson. */
  const lesson = $derived(weak[0] ? { concept: weak[0].spot.concept, ...practice(weak[0].spot.concept) } : null);

  /** The spot last jumped to, for stepping with the arrow keys. */
  let current = $state<number | null>(null);

  function jump(n: number, keepFilter = false) {
    if (!keepFilter) show = "all";
    current = n;
    requestAnimationFrame(() => {
      const el = document.getElementById(`spot-${n}`);
      el?.scrollIntoView({ behavior: "smooth", block: "start" });
      el?.focus({ preventScroll: true });
    });
  }

  /** ← and → step through the shown decisions, like a hand replayer. */
  function onKey(e: KeyboardEvent) {
    if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return;
    if (e.altKey || e.ctrlKey || e.metaKey || e.shiftKey) return;
    const t = e.target as HTMLElement | null;
    if (t?.closest("input, textarea, select, [role=radiogroup], [contenteditable]")) return;
    if (!shown.length) return;
    e.preventDefault();
    const at = shown.findIndex((s) => s.n === current);
    const next = at < 0 ? 0 : Math.max(0, Math.min(shown.length - 1, at + (e.key === "ArrowRight" ? 1 : -1)));
    jump(shown[next].n, true);
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="report">
  <section class="panel hero" aria-labelledby="report-title">
    <header class="top">
      <div class="titles">
        <h2 id="report-title">{title}</h2>
        <p class="sub">Besprochen am {created}</p>
      </div>
      <div class="buttons">
        {#if confirmDelete}
          <span class="sub">Bericht löschen?</span>
          <button class="btn danger" onclick={onDelete}>Ja, löschen</button>
          <button class="btn ghost" onclick={() => (confirmDelete = false)}>Behalten</button>
        {:else}
          <button class="btn" onclick={onReanalyze}>
            <svg viewBox="0 0 24 24" width="14" height="14" aria-hidden="true"
              ><path d="M19.5 12a7.5 7.5 0 1 1-2.2-5.3M19.5 4.5v4h-4" /></svg
            >
            Neu analysieren
          </button>
          <button class="btn ghost" onclick={() => (confirmDelete = true)}>Löschen</button>
        {/if}
      </div>
    </header>

    <div class="answer">
      <div class="say">
        <p class="summary">{report.summary}</p>
        <div class="next">
          {#if lesson?.train}
            <button class="btn primary" onclick={() => onTraining(lesson.train)}>
              {conceptName(lesson.concept)} üben
              <svg viewBox="0 0 24 24" width="14" height="14" aria-hidden="true"><path d="M5 12h14M13 6l6 6-6 6" /></svg>
            </button>
          {:else}
            <button class="btn primary" onclick={() => onTraining(lesson?.article ? { article: lesson.article.id } : null)}>
              {lesson?.article ? `Nachlesen: ${lesson.article.title}` : "Zum Training"}
            </button>
          {/if}
          {#if lesson?.train && lesson.article}
            <button class="link" onclick={() => onTraining({ article: lesson.article!.id })}>
              Nachlesen: {lesson.article.title}
            </button>
          {/if}
        </div>
      </div>

      <div class="score" aria-label="Urteile">
        <p class="big">
          {#if weak.length}
            <b class="num">{weak.length}</b> von <span class="num">{spots.length}</span>
            {spots.length === 1 ? "Entscheidung" : "Entscheidungen"} zum Üben
          {:else}
            Alle <b class="num">{spots.length}</b> Entscheidungen gut gespielt
          {/if}
        </p>
        <div class="strip">
          {#each spots as s (s.n)}
            <button
              class="cell v-{s.spot.verdict}"
              title="{s.n}. {VERDICTS.find((v) => v.id === s.spot.verdict)?.name}: {s.spot.action}"
              aria-label="Zur Entscheidung {s.n}"
              onclick={() => jump(s.n)}
            ></button>
          {/each}
        </div>
        <ul class="legend">
          {#each counts as c (c.id)}
            <li class:zero={c.n === 0} title="{c.name}: {c.meaning}">
              <i class="v-{c.id}" aria-hidden="true"></i><b class="num">{c.n}</b> {c.name}
            </li>
          {/each}
        </ul>
        {#if weak.length}
          <ol class="worst" aria-label="Entscheidungen zum Üben">
            {#each weak as s (s.n)}
              <li>
                <button class="v-{s.spot.verdict}" onclick={() => jump(s.n)}>
                  <i aria-hidden="true"></i>
                  <span class="mini-cards">
                    {#each s.spot.hero_cards as c (c)}<PlayingCard card={c} size={18} />{/each}
                  </span>
                  <span class="pos">{s.spot.position ?? "–"}</span>
                  <span class="txt">{s.spot.better ? `Besser: ${s.spot.better}` : s.spot.action}</span>
                  <span class="go num" aria-hidden="true">{s.n} →</span>
                </button>
              </li>
            {/each}
          </ol>
        {/if}
      </div>
    </div>
  </section>

  {#if report.focus.length}
    <section class="panel plan" aria-labelledby="plan-title">
      <div class="plan-head">
        <h3 id="plan-title">Woran du arbeiten solltest</h3>
        <button class="btn ghost small" onclick={() => onTraining(null)}>Zum Training</button>
      </div>
      <ol>
        {#each report.focus as f, i (i)}
          <li><span class="rank num">{i + 1}</span><span>{f}</span></li>
        {/each}
      </ol>
    </section>
  {/if}

  <div class="list-head">
    <h3>Deine Entscheidungen</h3>
    <span class="keys"><kbd>←</kbd><kbd>→</kbd> springen</span>
    <div class="seg" role="radiogroup" aria-label="Entscheidungen filtern">
      {#each [["all", "Alle", spots.length], ["practise", "Zum Üben", weak.length], ["good", "Gut gespielt", spots.length - weak.length]] as [id, label, n] (id)}
        <button role="radio" aria-checked={show === id} class:on={show === id} onclick={() => (show = id as Show)}>
          {label} <span class="num faint">{n}</span>
        </button>
      {/each}
    </div>
  </div>

  <div class="spots">
    {#each shown as s (s.n)}
      <SpotCard spot={s.spot} index={s.n} practice={practice(s.spot.concept)} {onHand} {onTraining} />
    {:else}
      <p class="none">Hier ist nichts. Alle Entscheidungen fallen unter den anderen Filter.</p>
    {/each}
  </div>

  <footer class="panel how">
    <h3>So ist der Bericht entstanden</h3>
    <dl>
      <div><dt>Modell</dt><dd class="num">{shortModel(report.meta.model)}</dd></div>
      <div><dt>Kosten</dt><dd class="num">{cost(report.meta.cost_usd)}</dd></div>
      <div><dt>Dauer</dt><dd class="num">{duration(report.meta.seconds)}</dd></div>
      <div>
        <dt>Gesendet</dt>
        <dd><span class="num">{report.meta.hands_sent}</span> von <span class="num">{report.meta.hands_total}</span> Händen</dd>
      </div>
      <div><dt>Ringside hat gerechnet</dt><dd><span class="num">{report.meta.tool_calls}</span>-mal</dd></div>
      <div>
        <dt>Tokens</dt><dd class="num">{tokens(report.meta.prompt_tokens + report.meta.completion_tokens)}</dd>
      </div>
    </dl>
    <p>
      <span class="tag calc">berechnet</span> stammt aus Ringsides Rechenkern (Equity, Pot Odds, Push/Fold-Nash, ICM, Outs).
      <span class="tag guess">Einschätzung</span> ist die Meinung des Sprachmodells, etwa über die Ranges deiner Gegner.
      {#if report.meta.downgraded}
        <b class="warn">
          {report.meta.downgraded}
          {report.meta.downgraded === 1 ? "Zahl hat" : "Zahlen hat"} das Modell als berechnet ausgegeben, ohne dass ein Werkzeug
          sie geliefert hat; sie stehen hier als Einschätzung.
        </b>
      {/if}
    </p>
  </footer>
</div>

<style>
  .report {
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-width: 0;
  }
  .sub {
    margin: 0;
    font-size: 12.5px;
    color: var(--text-3);
  }

  .hero {
    padding: 20px 24px 24px;
  }
  .top {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 10px 20px;
  }
  .titles {
    min-width: 0;
  }
  h2 {
    margin: 0 0 2px;
    font-family: var(--font-num);
    font-size: 24px;
    font-weight: 600;
    line-height: 1.2;
  }
  .buttons {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }
  .buttons svg,
  .next svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .answer {
    display: grid;
    grid-template-columns: minmax(0, 1.5fr) minmax(280px, 1fr);
    gap: 28px;
    margin-top: 18px;
  }
  .say {
    display: flex;
    flex-direction: column;
    gap: 18px;
  }
  .summary {
    margin: 0;
    font-size: 17px;
    line-height: 1.6;
    text-wrap: pretty;
    max-width: 68ch;
  }
  .next {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px 18px;
    margin-top: auto;
  }
  .next .btn {
    height: 36px;
    padding: 0 16px;
  }
  .link {
    padding: 0;
    border: 0;
    background: none;
    color: var(--text-2);
    font-size: 13px;
    text-decoration: underline;
    text-decoration-color: var(--line-2);
    text-underline-offset: 3px;
    cursor: pointer;
  }
  .link:hover {
    color: var(--text);
  }

  .score {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 16px 18px;
    border-radius: var(--r-sm);
    background: var(--bg);
    border: 1px solid var(--line);
  }
  .big {
    margin: 0;
    font-size: 14px;
    color: var(--text-2);
  }
  .big b {
    font-size: 26px;
    font-weight: 600;
    color: var(--text);
    margin-right: 2px;
  }
  .strip {
    display: flex;
    gap: 4px;
    height: 14px;
  }
  .cell {
    flex: 1;
    min-width: 10px;
    padding: 0;
    border: 0;
    border-radius: 3px;
    background: var(--c);
    cursor: pointer;
    transition: transform 0.12s;
  }
  .cell:hover {
    transform: scaleY(1.35);
  }
  .v-fehler {
    --c: var(--neg);
  }
  .v-fraglich {
    --c: var(--amber);
  }
  .v-ok {
    --c: var(--text-3);
  }
  .v-stark {
    --c: var(--pos);
  }
  .legend {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-wrap: wrap;
    gap: 4px 14px;
    font-size: 12.5px;
    color: var(--text-2);
  }
  .legend li {
    display: flex;
    align-items: baseline;
    gap: 5px;
    cursor: help;
  }
  .legend i {
    width: 8px;
    height: 8px;
    border-radius: 2px;
    background: var(--c);
    flex-shrink: 0;
    transform: translateY(-1px);
  }
  .legend b {
    color: var(--text);
    font-weight: 600;
  }
  .worst {
    list-style: none;
    margin: 2px 0 0;
    padding: 8px 0 0;
    border-top: 1px solid var(--line);
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .worst button {
    display: flex;
    align-items: center;
    gap: 9px;
    width: 100%;
    padding: 5px 6px;
    border: 0;
    border-radius: var(--r-sm);
    background: transparent;
    text-align: left;
    font-size: 13px;
    cursor: pointer;
  }
  .worst button:hover {
    background: var(--surface-2);
  }
  .worst i {
    width: 3px;
    align-self: stretch;
    border-radius: 2px;
    background: var(--c);
    flex-shrink: 0;
  }
  .mini-cards {
    display: flex;
    gap: 2px;
    flex-shrink: 0;
  }
  .pos {
    width: 28px;
    flex-shrink: 0;
    font-size: 12px;
    font-weight: 600;
    color: var(--text-2);
  }
  .txt {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .go {
    flex-shrink: 0;
    font-size: 12px;
    color: var(--text-3);
  }
  .worst button:hover .go {
    color: var(--text);
  }
  .legend li.zero {
    opacity: 0.45;
  }

  h3 {
    margin: 0;
    font-size: 13px;
    font-weight: 600;
    color: var(--text);
  }
  .plan {
    padding: 14px 18px 16px;
  }
  .plan-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }
  .small {
    height: 28px;
    font-size: 12.5px;
  }
  .plan ol {
    list-style: none;
    margin: 10px 0 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: 8px;
  }
  .plan li {
    display: flex;
    gap: 10px;
    align-items: flex-start;
    padding: 10px 12px;
    border-radius: var(--r-sm);
    background: var(--surface-2);
    line-height: 1.45;
  }
  .rank {
    display: grid;
    place-items: center;
    flex-shrink: 0;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    background: var(--surface-3);
    font-size: 12px;
    color: var(--text-2);
  }

  .list-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 10px;
    margin-top: 6px;
  }
  .list-head h3 {
    font-size: 15px;
  }
  .keys {
    margin-right: auto;
    display: flex;
    align-items: center;
    gap: 3px;
    font-size: 12px;
    color: var(--text-3);
  }
  kbd {
    display: inline-grid;
    place-items: center;
    min-width: 20px;
    height: 20px;
    padding: 0 4px;
    border: 1px solid var(--line-2);
    border-bottom-width: 2px;
    border-radius: 4px;
    font-family: var(--font-ui);
    font-size: 11px;
    color: var(--text-2);
  }
  .keys kbd:last-of-type {
    margin-right: 4px;
  }
  .spots {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .none {
    margin: 0;
    padding: 24px;
    text-align: center;
    color: var(--text-2);
  }

  .how {
    padding: 14px 18px 16px;
    margin-top: 4px;
    background: transparent;
  }
  .how h3 {
    color: var(--text-2);
  }
  dl {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(130px, 1fr));
    gap: 10px 16px;
    margin: 12px 0 0;
  }
  dt {
    font-size: 12px;
    color: var(--text-3);
  }
  dd {
    margin: 1px 0 0;
    font-size: 13.5px;
  }
  .how p {
    margin: 14px 0 0;
    font-size: 12.5px;
    line-height: 1.7;
    color: var(--text-2);
  }
  .tag {
    padding: 1px 6px;
    border-radius: 4px;
    font-size: 11px;
  }
  .tag.calc {
    background: var(--surface-3);
    color: var(--text);
  }
  .tag.guess {
    border: 1px dashed var(--line-2);
    color: var(--text-2);
  }
  .warn {
    display: block;
    margin-top: 6px;
    font-weight: 400;
    color: var(--amber);
  }

  @container (max-width: 860px) {
    .answer {
      grid-template-columns: minmax(0, 1fr);
      gap: 20px;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .cell {
      transition: none;
    }
  }
</style>
