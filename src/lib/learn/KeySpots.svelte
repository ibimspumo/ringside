<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { api } from "../api";
  import { appear } from "../coach/motion";
  import Badge from "../ui/Badge.svelte";
  import Button from "../ui/Button.svelte";
  import Card from "../ui/Card.svelte";
  import CardText from "../ui/CardText.svelte";
  import EmptyState from "../ui/EmptyState.svelte";
  import ProgressRing from "../ui/ProgressRing.svelte";
  import { errorText } from "./catalog";
  import DecisionDrill from "./DecisionDrill.svelte";
  import { preview } from "./preview";
  import type { DrillResult } from "./profile";
  import SpotRange from "./SpotRange.svelte";
  import type { KeySpotRow, KeySpots, KeySpotView, KeyTopic } from "./spots";

  /** „Wichtige Spots“: the bundled learning spots, practised in rounds; wrong answers come back first. */
  let { onProgress }: { onProgress: () => void } = $props();

  const ROUND = 10;

  let data = $state<KeySpots | null>(null);
  let error = $state<string | null>(null);
  let phase = $state<"list" | "run" | "done">("list");
  /** The round's spots, their answers and the topic it was started from. */
  let round = $state<string[]>([]);
  let results = $state<(boolean | null)[]>([]);
  let topicOfRound = $state<string | null>(null);
  let index = $state(0);
  let view = $state<KeySpotView | null>(null);
  let showRange = $state(false);

  async function load() {
    try {
      data = await api.learnKeySpots();
      error = null;
    } catch (e) {
      error = errorText(e);
    }
  }

  onMount(async () => {
    await load();
    const t = preview("spots-topic");
    if (preview("spots") === "run" || t) untrack(() => start(t));
  });

  const byId = $derived(new Map((data?.spots ?? []).map((s) => [s.id, s])));
  const sure = $derived((data?.spots ?? []).filter((s) => s.seen.right === true).length);
  const again = $derived((data?.spots ?? []).filter((s) => s.seen.right === false).length);
  const fresh = $derived((data?.spots ?? []).filter((s) => s.seen.right === null).length);
  const total = $derived(data?.spots.length ?? 0);

  function topicSpots(t: KeyTopic): KeySpotRow[] {
    return (data?.spots ?? []).filter((s) => s.topic === t.id);
  }

  /** What the next round holds, in words: "3 zum Wiederholen, 7 neue". */
  function roundText(ids: string[]): string {
    const n = (f: (s: KeySpotRow) => boolean) => ids.filter((id) => { const s = byId.get(id); return s && f(s); }).length;
    const parts = [
      [n((s) => s.seen.right === false), "zum Wiederholen", "zum Wiederholen"],
      [n((s) => s.seen.right === null), "neuer", "neue"],
      [n((s) => s.seen.right === true), "zur Auffrischung", "zur Auffrischung"],
    ] as const;
    return parts
      .filter(([k]) => k > 0)
      .map(([k, one, many]) => `${k} ${k === 1 ? one : many}`)
      .join(", ")
      .replace(/, ([^,]*)$/, " und $1");
  }

  function pick(topic: string | null): string[] {
    const q = data?.queue ?? [];
    return (topic ? q.filter((id) => byId.get(id)?.topic === topic) : q).slice(0, ROUND);
  }
  const nextRound = $derived(pick(null));

  async function show(i: number) {
    index = i;
    view = null;
    showRange = preview("spots-range") === "1";
    error = null;
    try {
      view = await api.learnKeySpot(round[i]);
    } catch (e) {
      error = errorText(e);
    }
  }

  function start(topic: string | null) {
    const ids = pick(topic);
    if (!ids.length) return;
    topicOfRound = topic;
    round = ids;
    results = ids.map(() => null);
    phase = "run";
    show(0);
  }

  function graded(r: DrillResult) {
    results[index] = r.correct;
    onProgress();
  }

  async function next() {
    if (index + 1 < round.length) return show(index + 1);
    phase = "done";
    await load();
  }

  async function finish() {
    phase = results.some((r) => r !== null) ? "done" : "list";
    await load();
  }

  const rights = $derived(results.filter((r) => r === true).length);
  const answered = $derived(results.filter((r) => r !== null).length);
  const topicTitle = (id: string | null) => data?.topics.find((t) => t.id === id)?.title ?? "";
  const tone = (s: KeySpotRow) => (s.seen.right === true ? "sure" : s.seen.right === false ? "again" : "new");
</script>

{#if error && !data}
  <EmptyState tone="error" title="Die Spots lassen sich gerade nicht laden" text={error}>
    {#snippet action()}<Button onclick={load}>Nochmal versuchen</Button>{/snippet}
  </EmptyState>
{:else if !data}
  <EmptyState tone="loading" title="Spots werden geladen …" />
{:else if phase === "list"}
  <div class="list" in:appear>
    <section class="hero" aria-labelledby="ks-title">
      <div class="intro">
        <span class="eyebrow">{total} Spots · jeder mit Ringsides Engine gelöst</span>
        <h2 id="ks-title" class="title">Die wichtigsten Spots für kleine Turniere</h2>
        <p class="lead">
          Die Entscheidungen, die Gelegenheitsspieler am meisten Chips kosten: kurzer Stack, All-ins, Big Blind, Bubble, Flop und River. Du entscheidest,
          Ringside zeigt sofort, was richtig ist und warum. Was danebengeht, kommt in der nächsten Runde wieder.
        </p>
        <div class="cta">
          <Button variant="primary" size="lg" disabled={!nextRound.length} onclick={() => start(null)}>
            {sure + again === 0 ? "Erste Runde starten" : "Nächste Runde starten"}
          </Button>
          <span class="cta-note">{nextRound.length} Spots: {roundText(nextRound)}</span>
        </div>
      </div>
      <div class="stand" aria-label="Dein Stand">
        <ProgressRing value={total ? sure / total : 0} size={128} stroke={10} tone="good" label="{sure} von {total} Spots sitzen">
          <b class="ring-num num">{sure}</b>
          <span class="ring-of">von {total}</span>
        </ProgressRing>
        <ul class="key">
          <li><i class="dot sure"></i>sitzen <b class="num">{sure}</b></li>
          <li><i class="dot again"></i>wiederholen <b class="num">{again}</b></li>
          <li><i class="dot new"></i>noch neu <b class="num">{fresh}</b></li>
        </ul>
      </div>
    </section>

    <section class="topics" aria-label="Themen">
      {#each data.topics as t (t.id)}
        {@const list = topicSpots(t)}
        {@const ok = list.filter((s) => s.seen.right === true).length}
        {@const bad = list.filter((s) => s.seen.right === false).length}
        <Card onclick={() => start(t.id)} aria-label="{t.title}: {ok} von {list.length} sitzen. Runde mit diesem Thema starten">
          <span class="topic">
          <span class="t-head">
            <b class="t-title">{t.title}</b>
            <span class="t-count num">{ok}/{list.length}</span>
          </span>
          <span class="strip" aria-hidden="true">
            {#each list as s (s.id)}<i class="dot {tone(s)}"></i>{/each}
          </span>
          <span class="t-lesson">{t.lesson}</span>
          <span class="t-foot">
            {#if bad}<Badge tone="bad" size="sm">{bad} wiederholen</Badge>{:else if ok === list.length}<Badge tone="good" size="sm">alle sitzen</Badge>{/if}
            <span class="t-go">Üben <svg viewBox="0 0 24 24" width="14" height="14" aria-hidden="true"><path d="M5 12h14M13 6l6 6-6 6" /></svg></span>
          </span>
          </span>
        </Card>
      {/each}
    </section>
  </div>
{:else if phase === "run"}
  <div class="run">
    <div class="bar">
      <span class="count num">{index + 1} / {round.length}</span>
      <ol class="steps" aria-label="Fortschritt der Runde">
        {#each round as id, i (id)}
          <li class:now={i === index} class:ok={results[i] === true} class:miss={results[i] === false}></li>
        {/each}
      </ol>
      <Button variant="quiet" onclick={finish}>Runde beenden</Button>
    </div>
    {#if error}
      <EmptyState tone="error" title="Dieser Spot lässt sich gerade nicht laden" text={error}>
        {#snippet action()}<Button onclick={() => show(index)}>Nochmal versuchen</Button>{/snippet}
      </EmptyState>
    {:else if view}
      {@const v = view}
      {#key v.spot.key}
        <DecisionDrill
          spot={v.spot}
          check={(c) => api.learnKeySpotCheck(v.spot.key, c)}
          onGraded={graded}
          onNext={next}
          nextLabel={index + 1 < round.length ? "Nächster Spot" : "Zum Ergebnis"}
        >
          {#snippet origin()}
            <Badge tone="info" size="sm">Wichtiger Spot {v.number} von {v.total}</Badge>
          {/snippet}
          {#snippet after()}
            <div class="lesson">
              <b>Merksatz</b>
              <p>{v.topic.lesson}</p>
            </div>
            {#if v.grid}
              <div class="depth">
                <Button variant="secondary" full onclick={() => (showRange = !showRange)} aria-expanded={showRange}>
                  {showRange ? "Range ausblenden" : "Wie spielt jede Hand hier?"}
                </Button>
                {#if showRange}<div in:appear><SpotRange grid={v.grid} cards={v.spot.hero_cards} /></div>{/if}
              </div>
            {/if}
          {/snippet}
        </DecisionDrill>
      {/key}
    {:else}
      <EmptyState tone="loading" compact title="Spot wird aufgebaut …" />
    {/if}
  </div>
{:else}
  <section class="done" in:appear aria-labelledby="ks-done">
    <div class="done-head">
      <ProgressRing value={answered ? rights / answered : 0} size={120} stroke={10} tone={rights === answered ? "good" : rights * 2 >= answered ? "warn" : "bad"} label="{rights} von {answered} richtig">
        <b class="ring-num num">{rights}</b>
        <span class="ring-of">von {answered}</span>
      </ProgressRing>
      <div>
        <h2 id="ks-done" class="title">{rights === answered ? "Alles richtig!" : rights * 2 >= answered ? "Gute Runde" : "Gut, dass du’s geübt hast"}</h2>
        <p class="lead">
          {#if rights < answered}Die {answered - rights} falsch beantworteten Spots kommen in der nächsten Runde zuerst.{:else}Diese Spots sitzen. In einer Woche fragt Ringside sie zur Auffrischung wieder.{/if}
        </p>
      </div>
    </div>
    <ol class="recap">
      {#each round as id, i (id)}
        {@const s = byId.get(id)}
        {#if s && results[i] !== null}
          <li class:miss={results[i] === false}>
            <span class="mark" aria-label={results[i] ? "richtig" : "falsch"}>{results[i] ? "✓" : "✗"}</span>
            <CardText hands text={s.cards.join("")} />
            <span class="r-what">{s.position} · {String(s.stack_bb).replace(".", ",")} BB</span>
            <span class="r-topic">{topicTitle(s.topic)}</span>
          </li>
        {/if}
      {/each}
    </ol>
    <div class="cta">
      <Button variant="primary" size="lg" disabled={!nextRound.length} onclick={() => start(topicOfRound)}>Nächste Runde</Button>
      <Button onclick={() => (phase = "list")}>Zur Übersicht</Button>
    </div>
  </section>
{/if}

<style>
  .list {
    display: flex;
    flex-direction: column;
    gap: var(--sp-6);
  }
  .hero {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: var(--sp-6);
    align-items: center;
    padding: var(--sp-6);
    border: 1px solid var(--line);
    border-radius: var(--r-lg);
    background: var(--surface);
  }
  .intro {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    min-width: 0;
  }
  .eyebrow {
    font-size: var(--fs-sm);
    color: var(--text-3);
    font-weight: var(--fw-medium);
  }
  .title {
    margin: 0;
    font-size: var(--fs-2xl);
    font-weight: var(--fw-semibold);
    line-height: var(--lh-tight);
  }
  .lead {
    margin: 0;
    max-width: 68ch;
    color: var(--text-2);
    font-size: var(--fs-md);
    line-height: var(--lh-base);
  }
  .cta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-3) var(--sp-4);
    margin-top: var(--sp-2);
  }
  .cta-note {
    font-size: var(--fs-sm);
    color: var(--text-3);
  }
  .stand {
    display: flex;
    align-items: center;
    gap: var(--sp-5);
    padding-left: var(--sp-6);
    border-left: 1px solid var(--line);
  }
  .ring-num {
    font-size: var(--fs-3xl);
    font-weight: var(--fw-semibold);
    line-height: 1;
  }
  .ring-of {
    font-size: var(--fs-sm);
    color: var(--text-3);
  }
  .key {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    margin: 0;
    padding: 0;
    list-style: none;
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .key li {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
  }
  .key b {
    margin-left: auto;
    padding-left: var(--sp-4);
    color: var(--text);
  }
  .dot {
    display: inline-block;
    width: 10px;
    height: 10px;
    border-radius: var(--r-xs);
    background: var(--surface-3);
    flex: none;
  }
  .dot.sure {
    background: var(--pos);
  }
  .dot.again {
    background: var(--neg);
  }
  .dot.new {
    background: var(--surface-3);
    box-shadow: inset 0 0 0 1px var(--line-2);
  }

  .topics {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(250px, 1fr));
    gap: var(--sp-4);
  }
  .topic {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    height: 100%;
  }
  .t-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: var(--sp-3);
  }
  .t-title {
    font-size: var(--fs-lg);
    font-weight: var(--fw-semibold);
  }
  .t-count {
    font-size: var(--fs-sm);
    color: var(--text-3);
  }
  .strip {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .t-lesson {
    font-size: var(--fs-sm);
    color: var(--text-2);
    line-height: var(--lh-base);
  }
  .t-foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-2);
    margin-top: auto;
    min-height: 22px;
  }
  .t-go {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-1);
    margin-left: auto;
    font-size: var(--fs-sm);
    color: var(--text-3);
    transition: color var(--dur-fast) var(--ease-standard);
  }
  .t-go svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .topic:hover .t-go {
    color: var(--text);
  }

  .run {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
  }
  .bar {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
  }
  .count {
    font-size: var(--fs-md);
    color: var(--text-2);
    min-width: 4ch;
  }
  .steps {
    display: flex;
    gap: 4px;
    flex: 1;
    max-width: 420px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .steps li {
    flex: 1;
    height: 6px;
    border-radius: var(--r-pill);
    background: var(--surface-3);
    transition: background var(--dur) var(--ease-standard);
  }
  .steps li.now {
    background: var(--text-2);
  }
  .steps li.ok {
    background: var(--pos);
  }
  .steps li.miss {
    background: var(--neg);
  }
  .bar :global(.ui-btn) {
    margin-left: auto;
  }
  .lesson {
    display: flex;
    flex-direction: column;
    gap: var(--sp-1);
    padding: var(--sp-3) var(--sp-4);
    border-left: 3px solid var(--info);
    border-radius: var(--r-sm);
    background: var(--surface-2);
  }
  .lesson b {
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .lesson p {
    margin: 0;
    font-size: var(--fs-md);
    line-height: var(--lh-base);
  }
  .depth {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }

  .done {
    display: flex;
    flex-direction: column;
    gap: var(--sp-5);
    padding: var(--sp-6);
    border: 1px solid var(--line);
    border-radius: var(--r-lg);
    background: var(--surface);
  }
  .done-head {
    display: flex;
    align-items: center;
    gap: var(--sp-6);
  }
  .done-head > div {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }
  .recap {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
    gap: var(--sp-2) var(--sp-4);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .recap li {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding: var(--sp-2) var(--sp-3);
    border-radius: var(--r-sm);
    background: var(--surface-2);
    font-size: var(--fs-md);
  }
  .mark {
    width: 20px;
    text-align: center;
    color: var(--pos);
    font-weight: var(--fw-bold);
  }
  .recap li.miss .mark {
    color: var(--neg);
  }
  .r-what {
    color: var(--text-2);
  }
  .r-topic {
    margin-left: auto;
    font-size: var(--fs-sm);
    color: var(--text-3);
    text-align: right;
  }

  @container (max-width: 760px) {
    .hero {
      grid-template-columns: minmax(0, 1fr);
    }
    .stand {
      padding-left: 0;
      border-left: 0;
    }
  }
</style>
