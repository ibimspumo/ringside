<script lang="ts">
  import PlayingCard from "../PlayingCard.svelte";
  import MiniCard from "../ui/MiniCard.svelte";
  import MixBar from "../ui/MixBar.svelte";
  import { classCombos, comboClass } from "../solver/range";
  import PlayGrid from "./PlayGrid.svelte";
  import { GRADE, choiceColors, choiceLabel, num, signedBB } from "./labels";
  import { worst, type SessionHand } from "./session.svelte";
  import type { HandView, Lesson, Verdict } from "./types";

  let {
    view,
    lesson,
    hands,
    reviewing,
    onOpen,
    past = false,
  }: { view: HandView | null; lesson: Lesson | null; hands: SessionHand[]; reviewing: number | null; onOpen: (h: SessionHand) => void; past?: boolean } = $props();

  let tab = $state<"info" | "hands">("info");

  const colors = $derived(lesson ? choiceColors(lesson.choices) : []);
  /** Action names, with the size where two choices share a name (two bet sizes). */
  const names = $derived.by(() => {
    const ls = lesson ? lesson.choices.map((c) => choiceLabel(c)) : [];
    return ls.map((l) => (ls.filter((x) => x.label === l.label).length > 1 && l.size ? `${l.label} ${l.size}` : l.label));
  });
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
  /** "< 1 %" rather than "0 %" for a choice the range still takes now and then. */
  const share = (x: number) => (x > 0 && x < 0.005 ? "< 1 %" : `${Math.round(x * 100)} %`);
  const hero = $derived(view ? comboClass(view.hero_cards[0], view.hero_cards[1]) : -1);
  const actor = $derived(view ? view.positions[view.hero] : "");
  const STREET: Record<string, string> = { preflop: "Preflop", flop: "Flop", turn: "Turn", river: "River" };
  /** Which decision the range belongs to: your move, or the one in front of you when you asked for a tip. */
  const about = $derived.by(() => {
    if (!lesson) return null;
    const street = STREET[lesson.street] ?? "";
    if ("chosen" in lesson && typeof lesson.chosen === "number") {
      const l = choiceLabel(lesson.choices[lesson.chosen], lesson.street !== "preflop");
      return { street: `Rückblick · ${street}`, text: `Dein Zug: ${l.label}${l.size ? ` ${l.size}` : ""}` };
    }
    return { street, text: "Tipp für deine Entscheidung" };
  });
  /** A move the solution also plays that still costs: why it is graded so, in the move's own numbers. */
  const mixedWhy = $derived.by(() => {
    if (!lesson || !("judgement" in lesson)) return null;
    const v = lesson as Verdict;
    const j = v.judgement;
    if (!j || !v.strategy) return null;
    const f = v.strategy[v.chosen] ?? 0;
    if (f < 0.05 || j.grade === "best" || j.grade === "mix" || j.best === v.chosen) return null;
    const mine = choiceLabel(v.choices[v.chosen]).label;
    const best = choiceLabel(v.choices[j.best]).label;
    const cost = j.loss_bb !== null ? ` im Schnitt ${num(j.loss_bb, 2)} BB` : "";
    return `${mine} spielt die Lösung in ${Math.round(f * 100)}\u00a0% – trotzdem bringt er hier${cost} weniger als ${best}. Deshalb „${GRADE[j.grade].label}“.`;
  });
</script>

<section class="info">
  <div class="tabs" role="tablist" aria-label="Info">
    <button role="tab" aria-selected={tab === "info"} class:on={tab === "info"} onclick={() => (tab = "info")}>Strategie</button>
    <button role="tab" aria-selected={tab === "hands"} class:on={tab === "hands"} onclick={() => (tab = "hands")}>
      Hände <span class="num faint">{hands.length}</span>
    </button>
  </div>

  {#if tab === "info"}
    {#if past}
      <p class="nowline">Die Strategie für diese Entscheidung erscheint nach deinem Zug · Tipp vorher: <b>H</b> (zählt dann halb)</p>
    {/if}
    {#if lesson && (past || cells)}
      <div class="retro" class:back={past}>
      {#if about}<p class="about"><span>{about.street}</span> <b>{about.text}</b></p>{/if}
      {#if lesson.strategy && view}
        {@const own = lesson.strategy}
        <div class="mine">
          <p>Deine Hand {#each view.hero_cards as c (c)}<MiniCard card={c} />{/each} spielt die Lösung so:</p>
          <MixBar label="So oft spielt die Lösung deine Hand" parts={lesson.choices.map((c, k) => ({ label: names[k], share: Math.round((own[k] ?? 0) * 100), color: colors[k] }))} />
          <p class="key">
            {#each lesson.choices as c, k (k)}
              {#if (own[k] ?? 0) >= 0.005}<span><i style:background={colors[k]}></i>{names[k]} {Math.round((own[k] ?? 0) * 100)} %</span>{/if}
            {/each}
          </p>
          <span class="note" class:why={!!mixedWhy}>{mixedWhy ?? "Prozent: wie oft die Lösung jeden Zug wählt. Bewertet wird, was dein Zug gegenüber dem besten kostet."}</span>
        </div>
      {/if}
      </div>
    {/if}
    <!-- While you decide, the last move's range would read like advice for this one: it waits for your move. -->
    {#if !past && lesson && cells}
      <p class="cap">
        Range von {actor} an dieser Stelle: wie {lesson.source === "model" ? "das Modell" : lesson.source === "equilibrium" ? "das Gleichgewicht" : "die Lösung"} jede Hand spielt. Deine Hand ist umrandet.
      </p>
      <PlayGrid strategy={cells.strategy} reach={cells.reach} {colors} {hero} />
      <ul class="legend" aria-label="Wie oft die Range welchen Zug spielt">
        <li class="lh" aria-hidden="true"><span></span><span>Zug</span><span>Anteil aller Hände</span></li>
        {#each lesson.choices as c, k (k)}
          {@const l = choiceLabel(c, lesson.street !== "preflop")}
          <li>
            <span class="sw" style:background={colors[k]}></span>
            <span>{l.label}{#if l.size} <small class="faint">{l.size}</small>{/if}</span>
            <b class="num">{share(mix[k] ?? 0)}</b>
          </li>
        {/each}
      </ul>
      <p class="faint small">Höhe einer Zelle: wie viel der Hand hier noch in der Range ist. Farben: wie oft sie welchen Zug spielt.</p>
    {:else if !past}
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
  .about {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 4px 8px;
    margin: 0 0 6px;
    font-size: var(--fs-md);
  }
  .about span {
    font-size: var(--fs-xs);
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-3);
  }
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
  .retro.back {
    opacity: 0.78;
  }
  .nowline + .retro {
    padding-top: 12px;
    border-top: 1px solid var(--line);
  }
  .nowline {
    margin: 0 0 12px;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .nowline b {
    color: var(--text);
  }
  .key {
    display: flex;
    flex-wrap: wrap;
    gap: 2px 12px;
    font-size: var(--fs-xs);
  }
  .key span {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }
  .key i {
    width: 8px;
    height: 8px;
    border-radius: 2px;
  }
  .mine {
    display: grid;
    gap: 6px;
    margin-bottom: 14px;
    padding-bottom: 14px;
    border-bottom: 1px solid var(--line);
  }
  .mine p {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px;
    margin: 0;
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .mine .note {
    font-size: var(--fs-xs);
    line-height: 1.45;
    color: var(--text-2);
  }
  .mine .note.why {
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .legend .lh {
    font-size: var(--fs-xs);
    color: var(--text-3);
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
