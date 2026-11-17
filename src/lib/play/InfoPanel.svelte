<script lang="ts">
  import PlayingCard from "../PlayingCard.svelte";
  import { classCombos, comboClass } from "../solver/range";
  import PlayGrid from "./PlayGrid.svelte";
  import { GRADE, choiceColors, choiceLabel, num, signedBB } from "./labels";
  import { worst, type SessionHand } from "./session.svelte";
  import type { HandView, Lesson } from "./types";

  let {
    view,
    lesson,
    hands,
    reviewing,
    onOpen,
  }: { view: HandView | null; lesson: Lesson | null; hands: SessionHand[]; reviewing: number | null; onOpen: (h: SessionHand) => void } = $props();

  let tab = $state<"info" | "hands">("info");

  const colors = $derived(lesson ? choiceColors(lesson.choices) : []);
  const cells = $derived.by(() => {
    if (!lesson) return null;
    if (lesson.grid) return { strategy: lesson.grid.strategy, reach: lesson.grid.reach };
    if (!lesson.node) return null;
    const strategy: (number[] | null)[] = new Array(169).fill(null);
    const reach = new Array(169).fill(0);
    for (const c of lesson.node.classes) {
      strategy[c.class] = c.strategy;
      reach[c.class] = Math.min(1, c.combos / classCombos(c.class));
    }
    return { strategy, reach };
  });
  /** Share of the whole range taking each choice. */
  const mix = $derived.by(() => {
    if (!cells || !lesson) return [];
    const out = lesson.choices.map(() => 0);
    let total = 0;
    cells.strategy.forEach((s, i) => {
      if (!s) return;
      const w = cells.reach[i] * classCombos(i);
      total += w;
      s.forEach((f, k) => (out[k] += f * w));
    });
    return out.map((x) => (total ? x / total : 0));
  });
  const hero = $derived(view ? comboClass(view.hero_cards[0], view.hero_cards[1]) : -1);
  const actor = $derived(view ? view.positions[view.hero] : "");
</script>

<section class="info">
  <div class="tabs" role="tablist" aria-label="Info">
    <button role="tab" aria-selected={tab === "info"} class:on={tab === "info"} onclick={() => (tab = "info")}>Strategie</button>
    <button role="tab" aria-selected={tab === "hands"} class:on={tab === "hands"} onclick={() => (tab = "hands")}>
      Hände <span class="num faint">{hands.length}</span>
    </button>
  </div>

  {#if tab === "info"}
    {#if lesson && cells}
      <p class="cap">
        Range von {actor} an dieser Stelle: wie {lesson.source === "model" ? "das Modell" : lesson.source === "equilibrium" ? "das Gleichgewicht" : "die Lösung"} jede Hand spielt. Deine Hand ist umrandet.
      </p>
      <PlayGrid strategy={cells.strategy} reach={cells.reach} {colors} {hero} />
      <ul class="legend">
        {#each lesson.choices as c, k (k)}
          {@const l = choiceLabel(c)}
          <li>
            <span class="sw" style:background={colors[k]}></span>
            <span>{l.label}{#if l.size} <small class="faint">{l.size}</small>{/if}</span>
            <b class="num">{Math.round((mix[k] ?? 0) * 100)} %</b>
          </li>
        {/each}
      </ul>
      <p class="faint small">Höhe einer Zelle: wie viel der Hand hier noch in der Range ist. Farben: wie oft sie welchen Zug spielt.</p>
    {:else}
      <div class="empty-state">
        <p>Strategie und Range erscheinen nach deinem Zug.</p>
        <p class="faint small">Vorher hilft „Tipp anzeigen“ (Taste H) – der Zug zählt dann nur halb.</p>
      </div>
    {/if}
  {:else if hands.length === 0}
    <div class="empty-state"><p class="faint">Gespielte Hände dieser Session erscheinen hier. Ein Klick öffnet die Hand noch einmal.</p></div>
  {:else}
    <ol class="hands">
      {#each [...hands].reverse() as h (h.n)}
        {@const g = worst(h.view)}
        {@const net = h.view.outcome?.net[h.view.hero] ?? 0}
        <li>
          <button class:on={reviewing === h.n} onclick={() => onOpen(h)}>
            <span class="dot" style:background={g ? GRADE[g].color : "var(--surface-3)"} title={g ? `Schwächster Zug: ${GRADE[g].label}` : "nicht bewertet"}></span>
            <span class="num faint">#{h.n}</span>
            <span class="cards"><PlayingCard card={h.view.hero_cards[0]} size={16} /><PlayingCard card={h.view.hero_cards[1]} size={16} /></span>
            <span class="pos">{h.view.positions[h.view.hero]}{h.repeat ? " · Wdh." : ""}</span>
            <span class="num net" class:pos-v={net > 0.04} class:neg-v={net < -0.04}>{signedBB(net)}</span>
          </button>
        </li>
      {/each}
    </ol>
  {/if}
</section>

<style>
  .info {
    display: grid;
    gap: 12px;
    align-content: start;
  }
  .tabs {
    display: flex;
    gap: 2px;
    padding: 2px;
    background: var(--surface-2);
    border-radius: var(--r-sm);
  }
  .tabs button {
    flex: 1;
    height: 28px;
    border: 0;
    border-radius: 5px;
    background: none;
    color: var(--text-2);
    font-size: 12.5px;
    cursor: pointer;
  }
  .tabs button.on {
    background: var(--surface-3);
    color: var(--text);
  }
  .cap {
    margin: 0;
    font-size: 12.5px;
    color: var(--text-2);
  }
  .legend {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 4px;
    font-size: 12.5px;
  }
  .legend li {
    display: grid;
    grid-template-columns: 12px 1fr auto;
    align-items: center;
    gap: 8px;
  }
  .legend small {
    margin-left: 4px;
  }
  .sw {
    width: 11px;
    height: 11px;
    border-radius: 3px;
  }
  .small {
    font-size: 11.5px;
    margin: 0;
  }
  .empty-state {
    padding: 18px 4px;
    display: grid;
    gap: 6px;
  }
  .empty-state p {
    margin: 0;
  }
  .hands {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 3px;
    max-height: 60vh;
    overflow-y: auto;
  }
  .hands button {
    width: 100%;
    display: grid;
    grid-template-columns: 10px 30px auto 1fr auto;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    border: 1px solid transparent;
    border-radius: var(--r-sm);
    background: none;
    font-size: 12.5px;
    cursor: pointer;
    text-align: left;
  }
  .hands button:hover {
    background: var(--surface-2);
  }
  .hands button.on {
    border-color: var(--focus);
  }
  .dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
  }
  .cards {
    display: inline-flex;
    gap: 2px;
  }
  .pos {
    color: var(--text-2);
  }
  .net {
    color: var(--text-2);
  }
  .pos-v {
    color: var(--pos);
  }
  .neg-v {
    color: var(--neg);
  }
</style>
