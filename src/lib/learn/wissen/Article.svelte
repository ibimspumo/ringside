<script lang="ts">
  import type { HudConfig } from "../../types";
  import { ARTICLE_BY_ID, LEVELS, UNITS, type Article, type Block } from "./articles";
  import Bars from "./blocks/Bars.svelte";
  import Cards from "./blocks/Cards.svelte";
  import Compare from "./blocks/Compare.svelte";
  import Formula from "./blocks/Formula.svelte";
  import Matchup from "./blocks/Matchup.svelte";
  import Matrix from "./blocks/Matrix.svelte";
  import Sample from "./blocks/Sample.svelte";
  import Scale from "./blocks/Scale.svelte";
  import TableDiagram from "./blocks/TableDiagram.svelte";
  import Tree from "./blocks/Tree.svelte";
  import CbetRange from "./widgets/CbetRange.svelte";
  import Combos from "./widgets/Combos.svelte";
  import Outs from "./widgets/Outs.svelte";
  import PositionWheel from "./widgets/PositionWheel.svelte";
  import PotOdds from "./widgets/PotOdds.svelte";
  import RuleTwoFour from "./widgets/RuleTwoFour.svelte";
  import StackRuler from "./widgets/StackRuler.svelte";

  let {
    article,
    read,
    onToggleRead,
    onOpen,
    hud = null,
  }: { article: Article; read: boolean; onToggleRead: () => void; onOpen: (id: string) => void; hud?: HudConfig | null } = $props();

  const TEXT_KINDS = new Set<Block["kind"]>(["points", "avoid", "note", "formula"]);
  const visuals = $derived(article.blocks.filter((b) => !TEXT_KINDS.has(b.kind)));
  const texts = $derived(article.blocks.filter((b) => TEXT_KINDS.has(b.kind)));
  const unit = $derived(UNITS.find((u) => u.id === article.unit)?.title ?? "");
  const related = $derived(article.related.map((id) => ARTICLE_BY_ID[id]).filter(Boolean));

</script>

<article class="art">
  <header>
    <div class="meta">
      <span>{unit}</span>
      <span class="dot"></span>
      <span>{LEVELS[article.level]}</span>
      {#if article.heuristic}<span class="badge">Faustregel</span>{/if}
    </div>
    <h2 class="lead-title">{article.title}</h2>
    <p class="one">{article.oneLiner}</p>
  </header>

  <div class="body">
    <div class="stage">
      {#each visuals as b, i (i)}
        <figure class="panel">
          {#if b.kind === "widget"}
            {@const o = b.props ?? {}}
            {#if b.widget === "pot-odds"}
              <PotOdds mode={o.mode as "call" | "mdf" | "alpha" | undefined} />
            {:else if b.widget === "position"}
              <PositionWheel />
            {:else if b.widget === "outs"}
              <Outs />
            {:else if b.widget === "rule-24"}
              <RuleTwoFour />
            {:else if b.widget === "combos"}
              <Combos hand={o.hand} blocked={o.blocked} />
            {:else if b.widget === "stack"}
              <StackRuler mode={o.mode as "bb" | "m" | undefined} />
            {:else if b.widget === "cbet"}
              <CbetRange />
            {/if}
          {:else if b.kind === "table"}
            <TableDiagram seats={b.seats} marks={b.marks} />
          {:else if b.kind === "cards"}
            <Cards rows={b.rows} board={b.board} highlight={b.highlight ?? null} />
          {:else if b.kind === "matchup"}
            <Matchup hero={b.hero} villain={b.villain} board={b.board} />
          {:else if b.kind === "scale"}
            {@const t = hud?.thresholds[b.stat]}
            <Scale stat={b.stat} max={b.max} low={t?.low ?? b.low} high={t?.high ?? b.high} labels={b.labels} ratio={b.ratio} palette={hud?.palette} />
          {:else if b.kind === "bars"}
            <Bars rows={b.rows} max={b.max} schematic={b.schematic} />
          {:else if b.kind === "compare"}
            <Compare legend={b.legend} rows={b.rows} />
          {:else if b.kind === "tree"}
            <Tree root={b.root} branches={b.branches} unit={b.unit} />
          {:else if b.kind === "matrix"}
            <Matrix range={b.range} />
          {:else if b.kind === "sample"}
            <Sample rows={b.rows} />
          {/if}
          {#if "caption" in b && b.caption}<figcaption>{b.caption}</figcaption>{/if}
        </figure>
      {/each}
    </div>

    <div class="notes">
      {#each texts as b, i (i)}
        {#if b.kind === "points"}
          <section class="points">
            <h2>Das Wichtigste</h2>
            <ol>
              {#each b.items as item (item)}<li>{item}</li>{/each}
            </ol>
          </section>
        {:else if b.kind === "formula"}
          <div class="panel tight">
            <Formula name={b.name} num={b.num} den={b.den} after={b.after} example={b.example} heuristic={b.heuristic} />
          </div>
        {:else if b.kind === "avoid"}
          <section class="avoid">
            <h2>Wann lieber nicht</h2>
            <ul>
              {#each b.items as item (item)}<li>{item}</li>{/each}
            </ul>
          </section>
        {:else if b.kind === "note"}
          <p class="note">{b.text}</p>
        {/if}
      {/each}
    </div>
  </div>

  <footer>
    {#if related.length}
      <div class="related">
        <span class="faint">Weiter mit</span>
        {#each related as r (r.id)}
          <button class="rel" onclick={() => onOpen(r.id)}>{r.title}<span aria-hidden="true">→</span></button>
        {/each}
      </div>
    {/if}
    <button class="btn read" class:done={read} onclick={onToggleRead} aria-pressed={read}>
      <svg width="15" height="15" viewBox="0 0 24 24" aria-hidden="true"><path d="M5 12.5l4.5 4.5L19 7.5" /></svg>
      {read ? "Gelesen" : "Als gelesen markieren"}
    </button>
  </footer>
</article>

<style>
  .art {
    display: grid;
    gap: 22px;
  }
  header {
    display: grid;
    gap: 6px;
  }
  .meta {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--text-3);
    font-size: 12.5px;
  }
  .meta .dot {
    width: 3px;
    height: 3px;
    border-radius: 50%;
    background: var(--text-3);
  }
  .badge {
    margin-left: 4px;
    padding: 1px 8px;
    border-radius: 99px;
    border: 1px solid var(--line-2);
    color: var(--text-2);
  }
  .lead-title {
    margin: 0;
    font-family: var(--font-num);
    font-size: var(--fs-3xl);
    font-weight: 600;
    line-height: 1.15;
  }
  .one {
    margin: 0;
    max-width: 62ch;
    color: var(--text-2);
    font-size: 15.5px;
  }
  .body {
    display: grid;
    gap: 16px;
    align-items: start;
  }
  @container wissen (min-width: 780px) {
    .body {
      grid-template-columns: minmax(0, 1.45fr) minmax(250px, 1fr);
      gap: 20px;
    }
  }
  .stage,
  .notes {
    display: grid;
    gap: 16px;
    min-width: 0;
  }
  .panel {
    margin: 0;
    padding: 18px;
    border-radius: var(--r);
    background: var(--surface);
    border: 1px solid var(--line);
    min-width: 0;
  }
  .panel.tight {
    padding: 14px 16px;
  }
  figcaption {
    margin-top: 12px;
    color: var(--text-2);
    font-size: 13px;
  }
  h2 {
    margin: 0 0 8px;
    color: var(--text-3);
    font-size: 12px;
    font-weight: 600;
  }
  .points ol {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 10px;
    counter-reset: p;
  }
  .points li {
    counter-increment: p;
    display: grid;
    grid-template-columns: 24px minmax(0, 1fr);
    gap: 10px;
    font-size: 14.5px;
    line-height: 1.45;
  }
  .points li::before {
    content: counter(p);
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    background: var(--surface-3);
    font-family: var(--font-num);
    font-size: 13px;
    font-weight: 600;
  }
  .avoid {
    padding: 12px 14px;
    border-radius: var(--r);
    border: 1px dashed var(--line-2);
  }
  .avoid ul {
    margin: 0;
    padding: 0;
    list-style: none;
    display: grid;
    gap: 6px;
  }
  .avoid li {
    position: relative;
    padding-left: 18px;
    color: var(--text-2);
    font-size: 13.5px;
  }
  .avoid li::before {
    content: "";
    position: absolute;
    left: 2px;
    top: 0.62em;
    width: 8px;
    border-top: 1.5px solid var(--text-3);
  }
  .note {
    margin: 0;
    color: var(--text-2);
  }
  footer {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding-top: 16px;
    border-top: 1px solid var(--line);
  }
  .related {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    font-size: 13px;
  }
  .rel {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 30px;
    padding: 0 12px;
    border-radius: 99px;
    border: 1px solid var(--line-2);
    background: var(--surface);
    font-size: 13px;
    cursor: pointer;
  }
  .rel span {
    color: var(--text-3);
  }
  .rel:hover {
    background: var(--surface-2);
  }
  .read svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 2.2;
  }
  .read.done {
    color: var(--text-2);
  }
</style>
