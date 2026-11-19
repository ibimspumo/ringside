<script lang="ts">
  import { go } from "../nav/go";
  import { GRADE_OF } from "../analysis/labels";
  import type { LearnTarget } from "../app/Leaks.svelte";
  import AtTable from "../app/AtTable.svelte";
  import { date } from "../app/format";
  import { tournamentName } from "../stats";
  import type { TournamentRow } from "../types";
  import Badge from "../ui/Badge.svelte";
  import Button from "../ui/Button.svelte";
  import Card from "../ui/Card.svelte";
  import EmptyState from "../ui/EmptyState.svelte";
  import GradeBadge from "../ui/GradeBadge.svelte";
  import GradeBars from "../ui/GradeBars.svelte";
  import MiniCard from "../ui/MiniCard.svelte";
  import { usePageHead } from "../ui/page.svelte";
  import Panel from "../ui/Panel.svelte";
  import SourceNote from "../ui/SourceNote.svelte";
  import Term from "../ui/Term.svelte";
  import { WORDS } from "../ui/words";
  import { AREA_HELP, PRACTICE, cardsOf, decisions, delta, points, spread, spreadOf } from "./labels";
  import SkillChart from "./SkillChart.svelte";
  import { loadSkill, skill } from "./store.svelte";
  import type { AreaScore, SkillArea, SkillEntry } from "./types";

  let {
    refresh,
    tournaments,
    hands,
    onHand,
    onHands,
    onLearn,
    onReport,
    onAllHands,
  }: {
    refresh: number;
    tournaments: TournamentRow[];
    /** Stored hands, for the note while PokerStars runs. */
    hands: number | null;
    onHand: (handId: number, actionIndex: number) => void;
    onHands: (area: SkillArea, label: string) => void;
    onLearn: (target: LearnTarget) => void;
    onReport: (tournamentId: number) => void;
    /** The hand list, where any hand can be analysed now. */
    onAllHands: () => void;
  } = $props();

  $effect(() => {
    void refresh;
    loadSkill();
  });

  const p = $derived(skill.profile);
  const rating = $derived(p?.rating ?? null);
  const counted = $derived(p?.entries.filter((e) => e.performance !== null) ?? []);
  const last = $derived<SkillEntry | null>(counted[counted.length - 1] ?? null);
  const names = $derived(new Map(tournaments.map((t) => [t.id, tournamentName(t)])));
  const nameOf = (e: SkillEntry) => (e.tournament_id !== null ? (names.get(e.tournament_id) ?? "Turnier") : "Cash-Hände");

  usePageHead(() => ({
    sub: last ? `Nach ${counted.length === 1 ? "1 Turnier" : `${counted.length} Turnieren`} · zuletzt ${date(last.started_at)}` : undefined,
  }));

  // The ladder: every level as a stretch of the scale, the rating and its band on it.
  const LADDER_MIN = 700;
  const LADDER_MAX = 2200;
  const at = (v: number) => `${((Math.min(LADDER_MAX, Math.max(LADDER_MIN, v)) - LADDER_MIN) / (LADDER_MAX - LADDER_MIN)) * 100}%`;
  const steps = $derived(
    (p?.tiers ?? []).map(([from, name], i, all) => ({ from: Math.max(from, LADDER_MIN), to: all[i + 1]?.[0] ?? LADDER_MAX, name })),
  );
  const next = $derived(rating === null ? null : (p?.tiers.find(([from]) => from > rating) ?? null));

  const VERDICT: Record<AreaScore["verdict"], { label: string; tone: "good" | "bad" | "neutral" }> = {
    strong: { label: "Stärke", tone: "good" },
    weak: { label: "Schwäche", tone: "bad" },
    even: { label: "Wie dein Schnitt", tone: "neutral" },
    few: { label: "Noch zu wenig", tone: "neutral" },
  };
  const ORDER: AreaScore["verdict"][] = ["weak", "strong", "even", "few"];
  /** Weaknesses by what they cost, then strengths by how strong; the first weakness is the focus above. */
  const areas = $derived(
    [...(p?.areas ?? [])].sort(
      (a, b) =>
        ORDER.indexOf(a.verdict) - ORDER.indexOf(b.verdict) ||
        (a.verdict === "weak" ? b.burn_share_pct - a.burn_share_pct : a.verdict === "strong" ? b.rating - a.rating : a.rating - b.rating),
    ),
  );
  /** The bars run ±250 points around the rating. */
  const REACH = 250;
  const offset = (a: AreaScore) => Math.max(-1, Math.min(1, (a.rating - (rating ?? 0)) / REACH));

  const gradeCounts = $derived(
    p ? { best: p.grades.perfect, good: p.grades.good, inaccurate: p.grades.inaccurate, mistake: p.grades.mistake, blunder: p.grades.blunder } : {},
  );
  const total = $derived(p ? p.grades.perfect + p.grades.good + p.grades.inaccurate + p.grades.mistake + p.grades.blunder : 0);
  const share = (n: number) => (total ? Math.round((n / total) * 100) : 0);
  const areaName = (a: SkillArea) => p?.areas.find((x) => x.area === a)?.name ?? "";
  /** What to work on first: the weakness that costs most. */
  const focus = $derived(areas[0]?.verdict === "weak" ? areas[0] : null);
</script>

{#snippet arrow(d: number)}
  <svg class="sk-arrow" class:up={d > 0} class:down={d < 0} viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">
    {#if d > 0}<path d="M8 3.5 13 9.5H3z" />{:else if d < 0}<path d="M8 12.5 3 6.5h10z" />{:else}<path d="M3 7h10v2H3z" />{/if}
  </svg>
{/snippet}

<div class="page sk">
  {#if skill.state === "locked"}
    <AtTable {hands} />
  {:else if skill.state === "error"}
    <EmptyState page tone="error" title="Die Wertung konnte nicht geladen werden" text="Deine Hände und Bewertungen sind sicher gespeichert. Versuch es noch einmal.">
      {#snippet action()}<Button variant="primary" onclick={() => loadSkill()}>Erneut laden</Button>{/snippet}
    </EmptyState>
  {:else if !p}
    <EmptyState page tone="loading" title="Wertung wird berechnet …" />
  {:else if rating === null}
    <EmptyState
      page
      title="Noch keine Wertung"
      text="Deine Spielwertung ist eine Zahl wie im Schach, die nach jedem Turnier steigt oder fällt; jeder startet bei {points(p.start)}. {hands
        ? 'Ringside analysiert neue Turniere von selbst, sobald du den Poker-Client schließt – oder du lässt deine Hände gleich jetzt bewerten.'
        : 'Richte zuerst ein, dass Ringside deine Hände bekommt – danach analysiert es jedes Turnier von selbst.'}"
    >
      {#snippet icon()}<path d="M4 19.5h16M5 15l4.5-4.5 3 3L19 7" />{/snippet}
      {#snippet action()}{#if hands}<Button variant="primary" onclick={onAllHands}>Zu deinen Händen</Button>{:else}<Button variant="primary" onclick={() => go("settings")}>Hände einrichten</Button>{/if}{/snippet}
    </EmptyState>
  {:else}
    <section class="sk-hero" aria-label="Deine Spielwertung">
      <div class="sk-score">
        <span class="sk-kicker">Deine {WORDS.skill_rating.word}</span>
        <div class="sk-number">
          <b class="num" data-skill-rating>{points(rating)}</b>
          <span class="sk-spread"><span class="num sk-spread-n">{spread(p.sigma)}</span><Term tip={WORDS.skill_spread.short}>wohl zwischen {Math.round(rating) - spreadOf(p.sigma)} und {Math.round(rating) + spreadOf(p.sigma)}</Term></span>
        </div>
        <div class="sk-tier">
          <Badge tone="info">{p.tier}</Badge>
          {#if p.provisional}<Badge dashed size="sm" title="Nach wenigen Turnieren schwankt die Zahl noch stark.">vorläufig – schwankt noch stark</Badge>{/if}
        </div>
        {#if last}
          <p class="sk-trend">
            {@render arrow(last.delta)}
            <b class="num" class:up={last.delta > 0} class:down={last.delta < 0}>{delta(last.delta)}</b>
            <span>im letzten Turnier</span>
          </p>
        {/if}
        {#if focus}
          {@const practice = PRACTICE[focus.area]}
          <div class="sk-focus">
            <span>Bringt dir jetzt am meisten: <b>{focus.name}</b> – dort entstehen {Math.round(focus.burn_share_pct)} % deiner Fehlerkosten.</span>
            {#if practice}
              <Button size="sm" variant="primary" onclick={() => onLearn(practice)}>{focus.name} üben</Button>
            {:else}
              <Button size="sm" variant="primary" onclick={() => onHands(focus.area, focus.name)}>Deine Hände dazu</Button>
            {/if}
          </div>
        {/if}
      </div>
      <div class="sk-ladder">
        <p class="sk-lead">
          Gezählt wird, <b>wie gut du entscheidest</b> – nicht, ob die Karten halten. Ein grober Fehler mit deinem ganzen Stack wiegt viel
          mehr als ein kleiner vor dem Flop.
        </p>
        <div class="sk-track" role="img" aria-label="Stufen der Wertung, du stehst bei {points(rating)}: {p.tier}">
          {#each steps as s, i (s.name)}
            <span class="sk-step" class:on={rating >= s.from && rating < s.to} style:left={at(s.from)} style:width="calc({at(s.to)} - {at(s.from)})" style:--i={i}>
              <i></i>
              <small>{s.name}</small>
              <em class="num">{s.from <= LADDER_MIN ? "" : s.from}</em>
            </span>
          {/each}
          <span class="sk-band" style:left={at(rating - p.sigma)} style:width="calc({at(rating + p.sigma)} - {at(rating - p.sigma)})"></span>
          <span class="sk-mark" style:left={at(rating)}><b class="num">{points(rating)}</b></span>
        </div>
        <p class="sk-goal">{#if next}Noch <b class="num">{Math.ceil(next[0] - rating)}</b> Punkte bis „{next[1]}“{:else}Höchste Stufe erreicht{/if}</p>
        <p class="sk-ref">Jeder startet bei {points(p.start)}. „Profi-Niveau“ ab 2000: So entscheiden Profis und die Poker-KI Pluribus nach Ringsides Maßstab.</p>
      </div>
    </section>

    <Panel title="Verlauf" sub="Nach jedem analysierten Turnier · grün gestiegen, rot gefallen · das helle Band zeigt, wie sicher die Zahl schon ist">
      <SkillChart height={210} entries={p.entries} tiers={p.tiers} start={p.start} {names} onPick={(e) => e.tournament_id !== null && onReport(e.tournament_id)} />
    </Panel>

    <div class="sk-grid">
      <Panel
        title="Stärken und Schwächen"
        sub="{counted.length >= 20 ? 'Deine letzten 20 Turniere' : counted.length === 1 ? 'Dein Turnier' : `Deine ${counted.length} Turniere`} je Bereich, das Teuerste zuerst: Punkte über oder unter deiner Wertung. „Kosten“: dein Anteil an dem, was deine Fehler im Schnitt an Stack kosten."
      >
        <ul class="sk-areas">
          {#each areas as a (a.area)}
            {@const v = VERDICT[a.verdict]}
            {@const o = offset(a)}
            {@const practice = PRACTICE[a.area]}
            <li class="sk-area" class:few={a.verdict === "few"} data-skill-area={a.area}>
              <div class="sk-area-name">
                <b><Term tip={AREA_HELP[a.area]}>{a.name}</Term>{#if focus?.area === a.area}{" "}<Badge size="sm" tone="info">Dein Fokus</Badge>{/if}</b>
                <small>{decisions(a.decisions)}{#if a.burn_share_pct >= 1}{` · ${Math.round(a.burn_share_pct)} % der Kosten`}{/if}</small>
              </div>
              <div class="sk-diverge" role="img" aria-label="{a.name}: {a.verdict === 'few' ? 'noch zu wenig' : `${points(a.rating)}, ${delta(a.rating - rating)} gegen deine Wertung`}">
                <span class="sk-axis"></span>
                {#if a.verdict !== "few"}
                  <span class="sk-bar" class:sk-plus={o > 0} class:sk-minus={o < 0} style:left={o < 0 ? `${50 + o * 50}%` : "50%"} style:width="{Math.abs(o) * 50}%"></span>
                {/if}
              </div>
              <span class="sk-area-num num" class:up={a.verdict === "strong"} class:down={a.verdict === "weak"}>
                {#if a.verdict === "few"}–{:else}{delta(a.rating - rating)}<small>Punkte</small>{/if}
              </span>
              <span class="sk-area-tag">
                <Badge size="sm" tone={v.tone} dashed={a.verdict === "few" || (p.provisional && a.verdict !== "even")}>{p.provisional && (a.verdict === "strong" || a.verdict === "weak") ? `eher ${v.label}` : v.label}</Badge>
              </span>
              <span class="sk-area-do">
                {#if practice}<Button size="sm" variant="quiet" onclick={() => onLearn(practice)}>Üben</Button>{/if}
                <Button size="sm" variant="quiet" onclick={() => onHands(a.area, a.name)}>Hände ansehen</Button>
              </span>
            </li>
          {/each}
        </ul>
      </Panel>

      <div class="sk-side">
        <Panel title="Deine Entscheidungen" sub="Alle bewerteten Entscheidungen, die in die Wertung eingehen">
          <GradeBars counts={gradeCounts} grades={["best", "good", "inaccurate", "mistake", "blunder"]} label="Deine Entscheidungen nach Note" />
          <p class="sk-note">
            {share(p.grades.perfect + p.grades.good)} % gut oder besser · {p.grades.blunder === 1 ? "1 grober Fehler" : `${p.grades.blunder} grobe Fehler`}
          </p>
        </Panel>

        {#if last}
          <Panel title="Was dich zuletzt bewegt hat" sub="{nameOf(last)} · {date(last.started_at)} · {delta(last.delta)}">
            {#if p.moments.length}
              <div class="sk-moments">
                {#each p.moments as m (`${m.hand_id}:${m.action_index}`)}
                  <Card pad="sm" onclick={() => onHand(m.hand_id, m.action_index)} title="Hand an dieser Stelle öffnen">
                    <span class="sk-moment">
                      <span class="sk-cards">{#each cardsOf(m.hero_cards) as c (c)}<MiniCard card={c} />{/each}</span>
                      <span class="sk-moment-text">
                        <b>{m.played}{#if m.best && m.points < 0}{" "}<span class="sk-better">statt {m.best}</span>{/if}</b>
                        <small>
                          {areaName(m.area)}
                          {#if m.lost_pct >= 0.5} · kostet im Schnitt {Math.round(m.lost_pct)} % deines Stacks{:else if m.pot_pct >= 50} · Pot so groß wie {Math.round(m.pot_pct)} % deines Stacks{/if}
                        </small>
                      </span>
                      <span class="sk-moment-tags">
                        {#if m.bad_luck}<Badge size="sm" tone="info">Gut entschieden, Pech gehabt</Badge>{:else}<GradeBadge grade={GRADE_OF[m.rating]} size="sm" />{/if}
                      </span>
                      <span class="sk-points num" class:up={m.points > 0} class:down={m.points < 0}>{delta(m.points)}<small>Punkte</small></span>
                    </span>
                  </Card>
                {/each}
              </div>
              <p class="sk-note">Wertungspunkte, ungefähr: Jede Entscheidung zählt nach ihrem Einsatz. <SourceNote kind="model" hint="Bewertet mit Gleichgewicht, Modell, Solver und Einschätzung; Einschätzungen zählen schwächer." /></p>
            {:else}
              <p class="sk-note">Keine Entscheidung hat die Zahl spürbar bewegt.</p>
            {/if}
          </Panel>
        {/if}
      </div>
    </div>

    <details class="sk-how">
      <summary>So entsteht die Zahl</summary>
      <ul>
        <li><b>Entscheidung statt Ergebnis:</b> Bewertet wird jede deiner Entscheidungen zum Zeitpunkt der Entscheidung. Ob die Karten danach halten, zählt nicht.</li>
        <li><b>Gewicht nach Einsatz:</b> Was eine Entscheidung im Schnitt kostet, zählt als Anteil deines Stacks. Ein falsches All-in kostet viel, ein kleiner Fehler vor dem Flop wenig; leichte Folds zählen gar nicht.</li>
        <li><b>Unsicherheit:</b> Am Anfang bewegt sich die Zahl stark, mit jedem Turnier ruhiger. Nach einer langen Pause wird sie wieder beweglicher. Das „±“ zeigt, wie sicher sie ist.</li>
        <li><b>Woher die Bewertung kommt:</b> Gleichgewicht, Modell und Solver zählen voll, Einschätzungen schwächer. Rechnet Ringside eine Hand neu, rechnet es auch die Wertung neu.</li>
      </ul>
    </details>
  {/if}
</div>

<style>
  .sk {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
  }
  .up {
    color: var(--pos);
  }
  .down {
    color: var(--neg);
  }

  .sk-hero {
    display: grid;
    grid-template-columns: minmax(240px, 300px) minmax(0, 1fr);
    gap: var(--sp-8);
    align-items: center;
    padding: var(--sp-5) var(--sp-4);
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    background: var(--surface);
  }
  .sk-score {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }
  .sk-kicker {
    color: var(--text-2);
    font-size: var(--fs-sm);
    font-weight: var(--fw-semibold);
  }
  .sk-number {
    display: flex;
    gap: var(--sp-3);
    align-items: baseline;
  }
  .sk-number b {
    font-size: var(--fs-hero);
    font-weight: var(--fw-bold);
    line-height: 1;
  }
  .sk-spread {
    display: inline-flex;
    flex-direction: column;
    gap: 2px;
    color: var(--text-2);
    font-size: var(--fs-xs);
  }
  .sk-spread-n {
    font-size: var(--fs-xl);
    line-height: 1;
  }
  .sk-focus {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--sp-2);
    margin-top: var(--sp-2);
    padding-top: var(--sp-3);
    border-top: 1px solid var(--line);
    color: var(--text-2);
    font-size: var(--fs-md);
  }
  .sk-focus b {
    color: var(--text);
  }
  .sk-tier {
    display: flex;
    gap: var(--sp-2);
    align-items: center;
  }
  .sk-trend {
    display: flex;
    gap: var(--sp-1);
    align-items: center;
    margin: 0;
    color: var(--text-2);
    font-size: var(--fs-base);
  }
  .sk-trend b {
    font-size: var(--fs-lg);
  }
  .sk-arrow {
    fill: var(--text-3);
  }
  .sk-arrow.up {
    fill: var(--pos);
  }
  .sk-arrow.down {
    fill: var(--neg);
  }

  .sk-ladder {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    min-width: 0;
  }
  .sk-lead {
    max-width: 68ch;
    margin: 0;
    color: var(--text-2);
    font-size: var(--fs-base);
    line-height: var(--lh-base);
  }
  .sk-lead b {
    color: var(--text);
    font-weight: var(--fw-semibold);
  }
  .sk-track {
    position: relative;
    height: 64px;
    margin-top: var(--sp-4);
  }
  .sk-step {
    position: absolute;
    top: 0;
    bottom: 0;
  }
  .sk-step i {
    position: absolute;
    top: 0;
    left: 1px;
    right: 1px;
    height: 10px;
    border-radius: var(--r-xs);
    background: var(--surface-3);
  }
  .sk-step.on i {
    background: var(--sd);
    opacity: 0.55;
  }
  .sk-step small {
    position: absolute;
    top: 18px;
    left: 4px;
    right: 4px;
    overflow: hidden;
    color: var(--text-2);
    font-size: var(--fs-xs);
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sk-step.on small {
    color: var(--text);
    font-weight: var(--fw-semibold);
  }
  .sk-step em {
    position: absolute;
    top: 36px;
    left: 4px;
    color: var(--text-2);
    font-size: var(--fs-sm);
    font-style: normal;
  }
  .sk-band {
    position: absolute;
    top: -3px;
    height: 16px;
    border-radius: var(--r-xs);
    background: var(--sd);
    opacity: 0.25;
  }
  .sk-mark {
    position: absolute;
    top: -6px;
    width: 2px;
    height: 22px;
    margin-left: -1px;
    background: var(--text);
  }
  .sk-mark b {
    position: absolute;
    bottom: 26px;
    left: 50%;
    padding: 1px 6px;
    border-radius: var(--r-xs);
    background: var(--text);
    color: var(--text-on-light);
    font-size: var(--fs-sm);
    transform: translateX(-50%);
  }
  .sk-goal {
    margin: 0;
    font-size: var(--fs-lg);
    font-weight: var(--fw-semibold);
  }
  .sk-goal b {
    color: var(--sd);
  }
  .sk-ref {
    margin: calc(-1 * var(--sp-3)) 0 0;
    color: var(--text-2);
    font-size: var(--fs-sm);
  }

  .sk-grid {
    display: grid;
    grid-template-columns: minmax(0, 1.25fr) minmax(0, 1fr);
    gap: var(--sp-4);
    align-items: start;
  }
  .sk-side {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    min-width: 0;
  }

  .sk-areas {
    display: grid;
    grid-template-columns: minmax(0, 1.7fr) minmax(64px, 1fr) max-content max-content max-content;
    gap: 0 var(--sp-3);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .sk-area {
    display: contents;
  }
  .sk-area > * {
    display: flex;
    align-items: center;
    min-height: 52px;
    border-bottom: 1px solid var(--line);
  }
  .sk-area:last-child > * {
    border-bottom: 0;
  }
  .sk-area-name {
    flex-direction: column;
    align-items: flex-start !important;
    justify-content: center;
    gap: 2px;
    min-width: 0;
  }
  .sk-area-name b {
    font-size: var(--fs-base);
    font-weight: var(--fw-semibold);
  }
  .sk-area-name small {
    color: var(--text-2);
    font-size: var(--fs-sm);
  }
  .few .sk-area-name b {
    color: var(--text-2);
  }
  .sk-diverge {
    position: relative;
  }
  .sk-axis {
    position: absolute;
    left: 50%;
    top: 50%;
    width: 1px;
    height: 18px;
    background: var(--line-2);
    transform: translateY(-50%);
  }
  .sk-diverge::before {
    content: "";
    position: absolute;
    left: 0;
    right: 0;
    top: 50%;
    height: 6px;
    border-radius: var(--r-pill);
    background: var(--surface-2);
    transform: translateY(-50%);
  }
  .sk-bar {
    position: absolute;
    top: 50%;
    height: 6px;
    border-radius: var(--r-pill);
    background: var(--text-3);
    transform: translateY(-50%);
  }
  .sk-bar.sk-plus {
    background: var(--pos);
  }
  .sk-bar.sk-minus {
    background: var(--neg);
  }
  .sk-area-num small {
    margin-left: 3px;
    color: var(--text-3);
    font-size: var(--fs-xs);
    font-weight: var(--fw-regular);
  }
  .sk-area-tag {
    gap: var(--sp-1);
  }
  .sk-area-num {
    justify-content: flex-end;
    color: var(--text-2);
    font-size: var(--fs-md);
    font-weight: var(--fw-semibold);
  }
  .sk-area-do {
    gap: var(--sp-1);
    justify-content: flex-end;
  }

  .sk-note {
    margin: var(--sp-3) 0 0;
    color: var(--text-2);
    font-size: var(--fs-sm);
  }

  .sk-moments {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }
  .sk-moment {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto 44px;
    gap: var(--sp-3);
    align-items: center;
  }
  .sk-cards {
    display: flex;
    gap: 2px;
  }
  .sk-moment-text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .sk-moment-text b {
    overflow: hidden;
    font-size: var(--fs-md);
    font-weight: var(--fw-semibold);
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sk-better {
    color: var(--text-2);
    font-weight: var(--fw-regular);
  }
  .sk-moment-text small {
    color: var(--text-2);
    font-size: var(--fs-sm);
  }
  .sk-points small {
    display: block;
    color: var(--text-3);
    font-size: var(--fs-xs);
    font-weight: var(--fw-regular);
  }
  .sk-points {
    color: var(--text-2);
    font-size: var(--fs-lg);
    font-weight: var(--fw-bold);
    text-align: right;
  }

  .sk-how {
    padding: var(--sp-3) var(--sp-4);
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    background: var(--surface);
  }
  .sk-how summary {
    color: var(--text-2);
    font-size: var(--fs-md);
    font-weight: var(--fw-semibold);
    cursor: pointer;
  }
  .sk-how ul {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--sp-3) var(--sp-6);
    margin: var(--sp-3) 0 var(--sp-1);
    padding: 0;
    list-style: none;
    color: var(--text-2);
    font-size: var(--fs-md);
    line-height: var(--lh-base);
  }
  .sk-how b {
    color: var(--text);
  }

  @media (max-width: 1279px) {
    .sk-hero {
      grid-template-columns: 1fr;
      gap: var(--sp-4);
    }
    .sk-grid {
      grid-template-columns: 1fr;
    }
  }
</style>
