<script lang="ts">
  import type { ConceptCount } from "../coach/types";
  import type { LearnKind, LearnProgress } from "../types";
  import Badge from "../ui/Badge.svelte";
  import Button from "../ui/Button.svelte";
  import EmptyState from "../ui/EmptyState.svelte";
  import SectionHead from "../ui/SectionHead.svelte";
  import { SKILLS, nextLevel, trainableKind, weakConcepts } from "./catalog";
  import { bbText, changeLine, costLine, unsureLine, weakGroups, type PlayProfile, type SpotGroup } from "./profile";
  import SkillBadge from "./SkillBadge.svelte";
  import { ARTICLE_BY_ID } from "./wissen/articles";
  import { preview } from "./preview";

  /**
   * Where the user really loses: kinds of spot from the playing profile, costliest first, then
   * concepts the coach found shaky. Renders nothing without either unless `empty` asks for a hint.
   */
  let {
    profile,
    concepts,
    progress,
    limit = 4,
    primary = false,
    empty = false,
    onPractice,
    onHands,
    onTrain,
    onRead,
  }: {
    profile: PlayProfile | null;
    concepts: ConceptCount[];
    progress: LearnProgress | null;
    limit?: number;
    /** The first row's practice button is the page's main action. */
    primary?: boolean;
    /** Show a hint while there is nothing to show yet. */
    empty?: boolean;
    onPractice: (key: string) => void;
    onHands?: (key: string, label: string) => void;
    onTrain: (kind: LearnKind, level: number) => void;
    onRead: (id: string) => void;
  } = $props();

  const groups = $derived(weakGroups(profile).slice(0, limit));
  const coach = $derived(weakConcepts(concepts).slice(0, Math.max(2, limit - groups.length)));
  const top = $derived(Math.max(...groups.map((g) => g.loss_bb), 0.01));
  const label = (g: SpotGroup) => `${g.title}, ${g.detail.split(" · ")[0]}`;

  let section = $state<HTMLElement>();
  $effect(() => {
    void groups.length;
    void coach.length;
    // After the page around it has settled, which the plan above changes once the profile is in.
    if (section && preview("weak")) requestAnimationFrame(() => section?.scrollIntoView({ block: "start" }));
  });

  function counts(c: ConceptCount): string {
    const parts = [];
    if (c.mistakes) parts.push(`${c.mistakes} Fehler`);
    if (c.questionable) parts.push(`${c.questionable} ${c.questionable === 1 ? "fragliche Entscheidung" : "fragliche Entscheidungen"}`);
    const where = `in ${c.tournaments} ${c.tournaments === 1 ? "Turnier" : "Turnieren"}`;
    return parts.length ? `${parts.join(" und ")} ${where}` : `${c.spots} ${c.spots === 1 ? "Spot" : "Spots"} ${where}`;
  }
</script>

{#snippet arrow()}<svg viewBox="0 0 24 24" width="14" height="14" aria-hidden="true"><path d="M5 12h14M13 6l6 6-6 6" /></svg>{/snippet}

{#if groups.length || coach.length}
  <section class="weak" aria-label="Deine Schwachstellen" bind:this={section}>
    <SectionHead
      title="Deine Schwachstellen"
      sub={groups.length
        ? `Aus ${profile?.decisions.toLocaleString("de-DE")} bewerteten Entscheidungen deiner Hände${coach.length ? " und deinen Coach-Berichten" : ""}. Was dich am meisten kostet, steht oben.`
        : "Aus deinen Coach-Berichten. Hier lohnt sich Üben gerade am meisten."}
    />
    <ol class="rows">
      {#each groups as grp, i (grp.key)}
        {@const unsure = unsureLine(grp)}
        {@const change = changeLine(grp)}
        <li class="row" data-key={grp.key}>
          <span class="rank num" aria-hidden="true">{grp.rank}</span>
          <div class="what">
            <b class="title">{label(grp)}</b>
            <span class="line">{costLine(grp)}</span>
            <span class="meta">
              <span>Fehlerquote <b class="num">{Math.round(grp.error_pct)} %</b></span>
              <span class="faint">{grp.detail.split(" · ")[1] ?? grp.detail}</span>
              {#if change}<Badge tone={grp.change && grp.change.after_error_pct < grp.change.before_error_pct ? "good" : "neutral"} size="sm">{change}</Badge>{/if}
            </span>
            {#if unsure}<span class="unsure"><Badge tone="warn" size="sm">Einschätzung</Badge> {unsure}</span>{/if}
          </div>
          <div class="cost" title="So viel hat dich diese Lage insgesamt gekostet">
            <span class="bar" aria-hidden="true"><span class="fill" class:unsure={grp.unsure} style:width="{Math.max(4, (grp.loss_bb / top) * 100)}%"></span></span>
            <span class="total"><b class="num">{bbText(grp.loss_bb)}</b> <span class="faint">insgesamt</span></span>
          </div>
          <div class="go">
            <Button variant={primary && i === 0 ? "primary" : "secondary"} icon={arrow} onclick={() => onPractice(grp.key)}>Üben</Button>
            {#if onHands}<Button variant="quiet" onclick={() => onHands(grp.key, label(grp))}>Hände ansehen</Button>{/if}
          </div>
        </li>
      {/each}
      {#each coach as c (c.id)}
        {@const kind = trainableKind(c)}
        {@const article = c.articles.map((a) => ARTICLE_BY_ID[a]).find(Boolean)}
        {@const skill = kind ? progress?.skills.find((s) => s.id === kind) : undefined}
        <li class="row coach">
          <span class="rank" aria-hidden="true">{#if kind}<SkillBadge {kind} {skill} size={34} />{/if}</span>
          <div class="what">
            <b class="title">{kind ? SKILLS[kind].short : c.name}</b>
            <span class="line">{counts(c)}</span>
            <span class="meta"><span class="faint">Aus deinen Coach-Berichten, ohne Betrag in BB</span></span>
          </div>
          <div class="cost"></div>
          <div class="go">
            {#if kind}
              <Button variant={primary && !groups.length && c === coach[0] ? "primary" : "secondary"} icon={arrow} onclick={() => onTrain(kind, nextLevel(skill))}>Üben</Button>
            {/if}
            {#if article}<Button variant="quiet" onclick={() => onRead(article.id)}>Nachlesen</Button>{/if}
          </div>
        </li>
      {/each}
    </ol>
  </section>
{:else if empty}
  <EmptyState
    compact
    title="Deine Schwachstellen erscheinen nach der Analyse"
    text="Sobald Ringside deine Hände bewertet hat (Seite „Hände“, „Hände analysieren“), steht hier, welche Spots du oft spielst und welche dich am meisten kosten."
  />
{/if}

<style>
  .weak {
    display: grid;
    gap: 10px;
  }
  .rows {
    display: grid;
    gap: 6px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .row {
    display: grid;
    grid-template-columns: 34px minmax(0, 1fr) minmax(140px, 200px) auto;
    align-items: center;
    gap: 8px 16px;
    padding: 12px 14px;
    border-radius: var(--r);
    border: 1px solid var(--line);
    background: var(--surface);
  }
  .rank {
    display: grid;
    place-items: center;
    width: 34px;
    height: 34px;
    border-radius: 50%;
    background: color-mix(in srgb, var(--neg) 14%, var(--surface-2));
    color: var(--text);
    font-weight: var(--fw-semibold);
  }
  .coach .rank {
    background: none;
  }
  .what {
    display: grid;
    gap: 3px;
    min-width: 0;
  }
  .title {
    font-size: var(--fs-base);
    font-weight: var(--fw-semibold);
  }
  .line {
    font-size: var(--fs-md);
    color: var(--text);
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 14px;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .meta b {
    color: var(--text);
    font-weight: var(--fw-semibold);
  }
  .unsure {
    display: flex;
    align-items: baseline;
    gap: 8px;
    font-size: var(--fs-sm);
    color: var(--text-2);
    line-height: 1.4;
  }
  .cost {
    display: grid;
    gap: 5px;
  }
  .bar {
    height: 8px;
    border-radius: var(--r-pill);
    background: var(--surface-3);
    overflow: hidden;
  }
  .fill {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: var(--neg);
    transition: width var(--dur) var(--ease-standard);
  }
  .fill.unsure {
    background: repeating-linear-gradient(135deg, var(--amber) 0 5px, color-mix(in srgb, var(--amber) 55%, transparent) 5px 10px);
  }
  .total {
    font-size: var(--fs-sm);
  }
  .total b {
    font-size: var(--fs-md);
    font-weight: var(--fw-semibold);
  }
  .go {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 6px;
  }
  svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  @container (max-width: 980px) {
    .row {
      grid-template-columns: 34px minmax(0, 1fr) minmax(120px, 160px);
    }
    .go {
      grid-column: 2 / -1;
      justify-content: flex-start;
    }
  }
  @container (max-width: 560px) {
    .row {
      grid-template-columns: 34px minmax(0, 1fr);
    }
    .cost {
      grid-column: 2;
    }
  }
</style>
