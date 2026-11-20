<script lang="ts">
  import ScoreChip from "../ui/ScoreChip.svelte";
  import { WORDS } from "../ui/words";
  import { delta, points, spread } from "./labels";
  import { skill } from "./store.svelte";

  /** The Spielwertung at the head of the side bar; a click opens the profile. Locked while PokerStars runs. */
  let { active = false, compact = false, onclick }: { active?: boolean; compact?: boolean; onclick: () => void } = $props();

  const p = $derived(skill.profile);
  const last = $derived(p?.entries.filter((e) => e.performance !== null).at(-1) ?? null);
  const d = $derived(last?.delta ?? 0);
  const word = WORDS.skill_rating.word;
</script>

{#if skill.state === "locked"}
  <ScoreChip locked value="Wertung" sub="gesperrt" {active} {compact} wide title="{word}: gesperrt, solange ein Poker-Client läuft" {onclick} data-skill-chip />
{:else if p?.recalculating}
  <ScoreChip value="Wertung" sub="wird neu berechnet …" {active} {compact} wide title="{word}: wird mit der verbesserten Analyse neu berechnet." {onclick} data-skill-chip />
{:else if p?.rating != null}
  <ScoreChip
    value={points(p.rating)}
    trend={last ? d : null}
    sub={last ? `${delta(d)} · Spielwertung` : "Spielwertung"}
    subTone={d > 0 ? "good" : d < 0 ? "bad" : "neutral"}
    {active}
    {compact}
    wide
    title="{word} {points(p.rating)} {spread(p.sigma)} · {p.tier}{p.provisional ? ' (vorläufig)' : ''}{last ? ` · ${delta(d)} im letzten Turnier` : ''}. Klick öffnet dein Profil."
    {onclick}
    data-skill-chip
  />
{:else}
  <ScoreChip value="Wertung" sub={skill.state === "loading" ? "…" : "noch keine"} {active} {compact} wide title="{word}: noch keine. Sie entsteht nach deinem ersten analysierten Turnier." {onclick} data-skill-chip />
{/if}
