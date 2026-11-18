<script lang="ts" module>
  import type { LearnTarget } from "../app/Leaks.svelte";

  /** Where a concept leads in the learning area. */
  export type Practice = { train: LearnTarget | null; articles: { id: string; title: string }[] };
</script>

<script lang="ts">
  import { onMount } from "svelte";
  import Button from "../ui/Button.svelte";
  import Card from "../ui/Card.svelte";
  import CardText from "../ui/CardText.svelte";
  import GradeBars from "../ui/GradeBars.svelte";
  import GradeIcon from "../ui/GradeIcon.svelte";
  import Kbd from "../ui/Kbd.svelte";
  import PlayingCard from "../ui/PlayingCard.svelte";
  import ProgressRing from "../ui/ProgressRing.svelte";
  import SectionHead from "../ui/SectionHead.svelte";
  import SegmentedControl from "../ui/SegmentedControl.svelte";
  import SourceNote from "../ui/SourceNote.svelte";
  import { cost, duration, tokens } from "./api";
  import { conceptName } from "./concepts";
  import type { Choice } from "./drill";
  import { COACH_GRADES, VERDICTS, shortModel, toPractise, verdictOf, say } from "./meaning";
  import { rise } from "./motion";
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
    onDrill,
    onRead,
    returnTo = null,
    practised,
    canAnalyze = true,
  }: {
    report: CoachReport;
    title: string;
    practice: (concept: string) => Practice;
    onHand: (id: number) => void;
    onTraining: (target: LearnTarget | null) => void;
    onReanalyze: () => void;
    onDelete: () => void;
    /** Opens spot `n` (1-based) as an exercise, or its article. */
    onDrill: (n: number) => void;
    onRead: (n: number) => void;
    /** A spot to scroll back to, after coming back from practising it. */
    returnTo?: number | null;
    /** How spot `n` was answered when practised in this visit. */
    practised: (n: number) => { answer: Choice; right: boolean | null } | undefined;
    /** False without a key or while another analysis runs. */
    canAnalyze?: boolean;
  } = $props();

  type Show = "all" | "practise" | "good";
  let show = $state<Show>("all");
  let confirmDelete = $state(false);

  const spots = $derived(report.spots.map((spot, i) => ({ spot, n: i + 1 })));
  const weak = $derived(spots.filter((s) => toPractise(s.spot.verdict)));
  const shown = $derived(
    show === "all" ? spots : show === "practise" ? weak : spots.filter((s) => !toPractise(s.spot.verdict)),
  );
  const counts = $derived(
    Object.fromEntries(VERDICTS.map((v) => [v.grade, report.spots.filter((s) => s.verdict === v.id).length])),
  );
  const good = $derived(spots.length - weak.length);
  const filters = $derived<{ value: Show; label: string }[]>([
    { value: "all", label: `Alle ${spots.length}` },
    { value: "practise", label: `Zum Üben ${weak.length}` },
    { value: "good", label: `Gut gespielt ${good}` },
  ]);
  /** Long names keep their bracketed or second part out of the headline: "Day 2 · $109 … [PKO, Turbo]". */
  const name = $derived.by(() => {
    const cut = title.search(/\s(?:·|\[|\()/);
    return cut > 12 ? { main: title.slice(0, cut), extra: title.slice(cut).replace(/^\s·\s/, "").trim() } : { main: title, extra: "" };
  });
  const created = $derived(
    new Date(report.meta.created_at * 1000).toLocaleString("de-DE", { dateStyle: "medium", timeStyle: "short" }),
  );
  /** The concept of the first spot to practise: the report's main lesson. */
  const lesson = $derived(weak[0] ? { n: weak[0].n, concept: weak[0].spot.concept, ...practice(weak[0].spot.concept) } : null);

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

  onMount(() => {
    if (returnTo !== null) jump(returnTo, true);
  });

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

{#snippet arrow()}<svg viewBox="0 0 24 24" width="14" height="14" aria-hidden="true"><path d="M5 12h14M13 6l6 6-6 6" /></svg>{/snippet}
{#snippet again()}<svg viewBox="0 0 24 24" width="14" height="14" aria-hidden="true"><path d="M19.5 12a7.5 7.5 0 1 1-2.2-5.3M19.5 4.5v4h-4" /></svg>{/snippet}

<div class="report">
  <Card pad="lg" aria-labelledby="report-title">
    <header class="top">
      <div class="titles">
        <h2 id="report-title" title={title}>{name.main}</h2>
        {#if name.extra}<p class="extra">{name.extra}</p>{/if}
        <p class="sub">Besprochen am <span class="nowrap">{created}</span></p>
      </div>
      <div class="buttons">
        {#if confirmDelete}
          <span class="sub">Bericht löschen?</span>
          <Button variant="danger" onclick={onDelete}>Ja, löschen</Button>
          <Button variant="quiet" onclick={() => (confirmDelete = false)}>Behalten</Button>
        {:else}
          <Button icon={again} onclick={onReanalyze} disabled={!canAnalyze}
            title={canAnalyze ? undefined : "Erst den Key von OpenRouter hinterlegen oder die laufende Analyse abwarten"}>Neu analysieren</Button
          >
          <Button variant="quiet" onclick={() => (confirmDelete = true)}>Löschen</Button>
        {/if}
      </div>
    </header>

    <div class="answer">
      <div class="say">
        <p class="summary"><CardText hands text={say(report.summary)} /></p>
        {#if report.focus.length}
          <div class="focus">
            <h3 class="focus-title">Woran du arbeiten solltest</h3>
            <ol class="plan">
              {#each report.focus as f, i (i)}
                <li><span class="rank num">{i + 1}</span><span><CardText hands text={say(f)} /></span></li>
              {/each}
            </ol>
          </div>
        {/if}
        <div class="next">
          {#if lesson?.train}
            <Button variant="primary" size="lg" icon={arrow} onclick={() => onTraining(lesson.train)}>{conceptName(lesson.concept)} üben</Button>
          {:else if lesson}
            <Button variant="primary" size="lg" onclick={() => onDrill(lesson.n)}>Entscheidung {lesson.n} üben</Button>
          {:else}
            <Button variant="primary" size="lg" onclick={() => onTraining(null)}>Zum Lernen</Button>
          {/if}
          {#if lesson?.articles.length}
            <Button variant="quiet" onclick={() => onRead(lesson.n)}>Nachlesen: {lesson.articles[0].title}</Button>
          {/if}
        </div>
      </div>

      <Card tone="sunken" aria-label="Urteile">
        <div class="score">
          <div class="score-top">
            <ProgressRing value={spots.length ? good / spots.length : 0} size={76} stroke={7} tone="good" label="Gut gespielt">
              <span class="ring"><span class="ring-n num">{good}/{spots.length}</span><span class="ring-l">gut</span></span>
            </ProgressRing>
            <p class="big">
              {#if weak.length}
                <b class="num">{weak.length}</b> von <span class="num">{spots.length}</span>
                {spots.length === 1 ? "Entscheidung" : "Entscheidungen"} zum Üben
              {:else}
                Alle <b class="num">{spots.length}</b> Entscheidungen gut gespielt
              {/if}
            </p>
          </div>
          <GradeBars grades={COACH_GRADES} {counts} label="Urteile des Coaches" />
          {#if weak.length}
            <ol class="worst" aria-label="Entscheidungen zum Üben">
              {#each weak as s (s.n)}
                {@const v = verdictOf(s.spot.verdict)}
                <li>
                  <Card pad="sm" onclick={() => jump(s.n)} aria-label="Zur Entscheidung {s.n}">
                    <span class="row">
                      <GradeIcon grade={v.grade} size={15} />
                      <span class="mini-cards">{#each s.spot.hero_cards as c (c)}<PlayingCard card={c} size={18} />{/each}</span>
                      <span class="seat">{s.spot.position ?? "–"}</span>
                      <span class="txt"
                        >{#if s.spot.better}Besser: <CardText hands text={say(s.spot.better)} />{:else}<CardText hands text={say(s.spot.action)} />{/if}</span
                      >
                      <span class="go num" aria-hidden="true">{s.n} →</span>
                    </span>
                  </Card>
                </li>
              {/each}
            </ol>
          {/if}
        </div>
      </Card>
    </div>
  </Card>


  <div class="list-head">
    <h3>Deine Entscheidungen</h3>
    <span class="keys"><Kbd keys="←" /><Kbd keys="→" /> springen</span>
    <SegmentedControl options={filters} bind:value={show} label="Entscheidungen filtern" />
  </div>

  <div class="spots">
    {#each shown as s, i (s.n)}
      <div in:rise={{ delay: Math.min(i, 6) * 40 }}>
        <SpotCard
          spot={s.spot}
          index={s.n}
          practice={practice(s.spot.concept)}
          practised={practised(s.n)}
          {onHand}
          onDrill={() => onDrill(s.n)}
          onRead={() => onRead(s.n)}
        />
      </div>
    {:else}
      <p class="none">Hier ist nichts. Alle Entscheidungen fallen unter den anderen Filter.</p>
    {/each}
  </div>

  <Card tone="sunken">
    <SectionHead level={3} title="So ist der Bericht entstanden" />
    <dl class="how">
      <div><dt>Modell</dt><dd class="num">{shortModel(report.meta.model)}</dd></div>
      <div><dt>Kosten</dt><dd class="num">{cost(report.meta.cost_usd)}</dd></div>
      <div><dt>Dauer</dt><dd class="num">{duration(report.meta.seconds)}</dd></div>
      <div>
        <dt>Gesendet</dt>
        <dd><span class="num">{report.meta.hands_sent}</span> von <span class="num">{report.meta.hands_total}</span> Händen</dd>
      </div>
      <div><dt>Nachgerechnet</dt><dd><span class="num">{report.meta.tool_calls}</span>-mal</dd></div>
      <div>
        <dt>Tokens</dt><dd class="num">{tokens(report.meta.prompt_tokens + report.meta.completion_tokens)}</dd>
      </div>
    </dl>
    <p class="legend">
      <SourceNote kind="computed" /> stammt aus Ringsides Rechenkern (Equity, Pot Odds, Push/Fold-Nash, ICM, Outs).
      <SourceNote kind="opinion" /> ist die Meinung des Sprachmodells, etwa über die Ranges deiner Gegner.
    </p>
    {#if report.meta.downgraded}
      <p class="downgraded">
        {report.meta.downgraded}
        {report.meta.downgraded === 1 ? "Zahl hat" : "Zahlen hat"} das Modell als berechnet ausgegeben, ohne dass ein Werkzeug sie
        geliefert hat; sie stehen hier als Einschätzung.
      </p>
    {/if}
  </Card>
</div>

<style>
  .report {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    min-width: 0;
  }
  .sub,
  .extra {
    margin: 0;
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .extra {
    margin-bottom: 2px;
    color: var(--text);
  }
  .nowrap {
    white-space: nowrap;
  }
  svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .top {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: var(--sp-3) var(--sp-5);
  }
  .titles {
    flex: 1 1 360px;
    min-width: 0;
  }
  h2 {
    margin: 0 0 2px;
    font-family: var(--font-num);
    font-size: var(--fs-2xl);
    font-weight: var(--fw-semibold);
    line-height: 1.2;
  }
  .buttons {
    display: flex;
    flex-shrink: 0;
    align-items: center;
    gap: var(--sp-2);
    flex-wrap: wrap;
  }

  .answer {
    display: grid;
    grid-template-columns: minmax(0, 1.5fr) minmax(300px, 1fr);
    gap: var(--sp-6);
    margin-top: var(--sp-5);
  }
  .say {
    display: flex;
    flex-direction: column;
    gap: var(--sp-5);
  }
  .summary {
    margin: 0;
    font-size: var(--fs-xl);
    line-height: 1.6;
    text-wrap: pretty;
    max-width: 68ch;
  }
  .next {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-3) var(--sp-4);
  }

  .score {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
  }
  .score-top {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
  }
  .ring {
    display: flex;
    flex-direction: column;
    align-items: center;
    line-height: 1.1;
  }
  .ring-l {
    font-size: var(--fs-xs);
    color: var(--text-2);
  }
  .ring-n {
    font-size: var(--fs-lg);
    font-weight: var(--fw-semibold);
  }
  .big {
    margin: 0;
    font-size: var(--fs-base);
    color: var(--text-2);
  }
  .big b {
    font-size: var(--fs-3xl);
    font-weight: var(--fw-semibold);
    color: var(--text);
    margin-right: 2px;
  }
  .worst {
    list-style: none;
    margin: 0;
    padding: var(--sp-3) 0 0;
    border-top: 1px solid var(--line);
    display: flex;
    flex-direction: column;
    gap: var(--sp-1);
  }
  .row {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    font-size: var(--fs-md);
  }
  .mini-cards {
    display: flex;
    gap: 2px;
    flex-shrink: 0;
  }
  .seat {
    width: 28px;
    flex-shrink: 0;
    font-size: var(--fs-sm);
    font-weight: var(--fw-semibold);
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
    font-size: var(--fs-sm);
    color: var(--text-2);
  }

  .focus-title {
    margin: 0 0 var(--sp-2);
    font-size: var(--fs-md);
    font-weight: var(--fw-semibold);
    color: var(--text-2);
  }
  .plan {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }
  .plan li {
    display: flex;
    gap: var(--sp-3);
    align-items: center;
    font-size: var(--fs-base);
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
    font-size: var(--fs-sm);
    color: var(--text-2);
  }

  .list-head {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: var(--sp-3);
    margin-top: var(--sp-2);
  }
  .list-head h3 {
    margin: 0;
    font-size: var(--fs-lg);
    font-weight: var(--fw-semibold);
  }
  .keys {
    margin-right: auto;
    display: flex;
    align-items: center;
    gap: var(--sp-1);
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .spots {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }
  .none {
    margin: 0;
    padding: var(--sp-6);
    text-align: center;
    color: var(--text-2);
  }

  .how {
    display: grid;
    grid-template-columns: repeat(6, minmax(0, 1fr));
    gap: var(--sp-3) var(--sp-4);
    margin: var(--sp-3) 0 0;
  }
  dt {
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  dd {
    margin: 1px 0 0;
    font-size: var(--fs-base);
  }
  .legend,
  .downgraded {
    margin: var(--sp-4) 0 0;
    font-size: var(--fs-md);
    line-height: 1.9;
    color: var(--text-2);
  }
  .downgraded {
    margin-top: var(--sp-2);
    line-height: 1.5;
    color: var(--amber);
  }

  @container (max-width: 1100px) {
    .how {
      grid-template-columns: repeat(3, minmax(0, 1fr));
    }
  }
  @container (max-width: 860px) {
    .answer {
      grid-template-columns: minmax(0, 1fr);
      gap: var(--sp-5);
    }
  }
</style>
