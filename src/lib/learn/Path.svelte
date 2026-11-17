<script lang="ts">
  import type { ConceptCount } from "../coach/types";
  import type { LearnKind, LearnProgress } from "../types";
  import { SKILLS, UNITS, fmtPct, mastery, nextLevel } from "./catalog";
  import Mastery from "./Mastery.svelte";
  import Ring from "./Ring.svelte";
  import Weaknesses from "./Weaknesses.svelte";
  import { ARTICLE_BY_ID } from "./wissen/articles";
  import { loadRead } from "./wissen/read";

  let {
    progress,
    concepts,
    onTrain,
    onRead,
  }: {
    progress: LearnProgress | null;
    concepts: ConceptCount[];
    onTrain: (kind: LearnKind, level: number) => void;
    onRead: (id: string) => void;
  } = $props();

  const read = loadRead();
  const byKind = $derived(new Map((progress?.skills ?? []).map((s) => [s.id, s])));

  /** Skills count fully; units without exercises yet show how much of their reading is done. */
  function unitShare(skills: LearnKind[], articles: string[]): number {
    if (skills.length) return skills.reduce((a, k) => a + mastery(byKind.get(k)), 0) / skills.length;
    return articles.length ? articles.filter((a) => read.has(a)).length / articles.length : 0;
  }

  const units = $derived(UNITS.map((u) => ({ ...u, share: unitShare(u.skills, u.articles) })));
  /** First unit with a skill that is not yet "mit Hilfe". */
  const here = $derived(units.find((u) => u.skills.some((k) => (byKind.get(k)?.level ?? 0) < 2))?.id ?? null);
  const total = $derived(progress?.skills.filter((s) => s.id !== "all_in") ?? []);
  const solid = $derived(total.filter((s) => s.level >= 3).length);
</script>

<div class="path">
  <header class="head">
    <div>
      <h1>Lernpfad</h1>
      <p class="dim">Elf Einheiten, aufeinander aufgebaut. Spring ruhig dorthin, wo du stehst – nichts ist gesperrt.</p>
    </div>
    <div class="sum">
      <b class="num">{solid}</b><span class="faint">/ {total.length} Fertigkeiten selbstständig</span>
    </div>
  </header>

  <Weaknesses {concepts} {progress} limit={6} {onTrain} {onRead} />

  <ol class="units">
    {#each units as u (u.id)}
      <li class="stop" class:here={here === u.id} class:reading={!u.skills.length}>
        <div class="node">
          <Ring value={u.share} size={52} stroke={4} tone={u.share >= 0.75 ? "var(--pos)" : "var(--text)"} label="Einheit {u.n}: {Math.round(u.share * 100)} %">
            <b class="num n">{u.n}</b>
          </Ring>
        </div>
        <div class="card panel">
          <div class="top">
            <h2>{u.title}</h2>
            {#if here === u.id}<span class="tag now">Hier weitermachen</span>{/if}
            {#if !u.skills.length}<span class="tag">Bisher nur Wissen</span>{/if}
          </div>
          <p class="goal dim">{u.goal}</p>

          {#if u.skills.length}
            <ul class="skills">
              {#each u.skills as k (k)}
                {@const s = byKind.get(k)}
                <li>
                  <span class="ico" aria-hidden="true"><svg viewBox="0 0 24 24" width="18" height="18"><path d={SKILLS[k].icon} /></svg></span>
                  <span class="sname">
                    <b>{s?.name_de ?? SKILLS[k].short}</b>
                    <span class="faint small">
                      {#if s && s.attempts}
                        Stufe {s.max_level} · {s.attempts} Antworten{#if s.mean_score !== null} · Ø {fmtPct(s.mean_score * 100)}{/if}
                      {:else}
                        Noch nicht geübt
                      {/if}
                    </span>
                  </span>
                  <Mastery skill={s} />
                  {#if k !== "all_in"}
                    <button class="btn" onclick={() => onTrain(k, nextLevel(s))}>Üben</button>
                  {/if}
                </li>
              {/each}
            </ul>
          {/if}

          {#if u.articles.length}
            <div class="reads">
              <span class="faint small">Wissen</span>
              {#each u.articles as id (id)}
                {@const a = ARTICLE_BY_ID[id]}
                {#if a}
                  <button class="chip-btn" class:done={read.has(id)} onclick={() => onRead(id)}>
                    {#if read.has(id)}<svg viewBox="0 0 24 24" width="12" height="12" aria-label="gelesen"><path d="M5 12.5l4.5 4.5L19 7.5" /></svg>{/if}
                    {a.title}
                  </button>
                {/if}
              {/each}
            </div>
          {/if}

          {#if u.next.length}
            <p class="next faint small">Kommt noch: {u.next.join(", ")}</p>
          {/if}
        </div>
      </li>
    {/each}
  </ol>
</div>

<style>
  .path {
    display: grid;
    gap: 18px;
  }
  .head {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 12px 20px;
  }
  h1 {
    margin: 0;
    font-family: var(--font-num);
    font-size: 26px;
    font-weight: 600;
  }
  .head p {
    margin: 3px 0 0;
  }
  .sum {
    display: flex;
    align-items: baseline;
    gap: 6px;
    font-size: 13px;
  }
  .sum b {
    font-size: 22px;
    font-weight: 600;
  }
  .units {
    position: relative;
    display: grid;
    gap: 12px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .units::before {
    content: "";
    position: absolute;
    left: 25px;
    top: 26px;
    bottom: 26px;
    border-left: 2px dashed var(--surface-3);
  }
  .stop {
    position: relative;
    display: grid;
    grid-template-columns: 52px minmax(0, 1fr);
    gap: 14px;
    align-items: start;
  }
  .node {
    position: sticky;
    top: 8px;
    border-radius: 50%;
    background: var(--bg);
  }
  .n {
    font-size: 18px;
    font-weight: 600;
  }
  .card {
    display: grid;
    gap: 10px;
    padding: 14px 16px;
    transition: border-color 0.15s;
  }
  .here .card {
    border-color: var(--line-2);
    box-shadow: 0 0 0 1px var(--line-2);
  }
  .reading .card {
    background: transparent;
  }
  .top {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 10px;
  }
  h2 {
    margin: 0;
    font-size: 16px;
    font-weight: 600;
  }
  .tag {
    padding: 1px 8px;
    border-radius: 99px;
    border: 1px solid var(--line-2);
    font-size: 11.5px;
    color: var(--text-2);
  }
  .tag.now {
    background: var(--text);
    border-color: var(--text);
    color: var(--bg);
    font-weight: 600;
  }
  .goal {
    margin: -4px 0 0;
    font-size: 13px;
  }
  .skills {
    display: grid;
    gap: 6px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .skills li {
    display: grid;
    grid-template-columns: 34px minmax(0, 1fr) auto auto;
    align-items: center;
    gap: 12px;
    padding: 8px 10px;
    border-radius: var(--r-sm);
    background: var(--surface-2);
  }
  .ico {
    display: grid;
    place-items: center;
    width: 34px;
    height: 34px;
    border-radius: 8px;
    background: var(--surface-3);
  }
  svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 1.7;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .sname {
    display: grid;
    min-width: 0;
  }
  .sname b {
    font-size: 13.5px;
    font-weight: 600;
  }
  .small {
    font-size: 12px;
  }
  .reads {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }
  .chip-btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 26px;
    padding: 0 10px;
    border-radius: 99px;
    border: 1px solid var(--line-2);
    background: transparent;
    font-size: 12.5px;
    cursor: pointer;
  }
  .chip-btn:hover {
    background: var(--surface-2);
  }
  .chip-btn.done {
    color: var(--text-2);
  }
  .chip-btn svg {
    stroke-width: 2.6;
  }
  .next {
    margin: 0;
  }
  @container (max-width: 640px) {
    .skills li {
      grid-template-columns: 34px minmax(0, 1fr) auto;
    }
    .skills li :global(.mastery) {
      grid-column: 2;
    }
    .skills li .btn {
      grid-column: 3;
      grid-row: 1 / span 2;
    }
  }
</style>
