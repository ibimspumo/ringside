<script lang="ts">
  import CardRow from "../blocks/CardRow.svelte";
  import { int, num, pct } from "../math";
  import { flopHit, pairs, rangeCombos, run } from "../poker";

  // Two example ranges, written for this page. Assumptions, not solver output.
  const RAISER = [
    ...pairs("2", "A"),
    ...run("A", "2", "K", "s"),
    ...run("K", "9", "Q", "s"),
    ...run("Q", "9", "J", "s"),
    "J9s",
    "JTs",
    "T9s",
    "98s",
    "87s",
    "76s",
    ...run("A", "T", "K", "o"),
    "KJo",
    "KQo",
    "QJo",
  ];
  const CALLER = [
    ...pairs("2", "9"),
    ...run("A", "2", "J", "s"),
    ...run("K", "5", "J", "s"),
    ...run("Q", "7", "J", "s"),
    ...run("J", "7", "T", "s"),
    ...run("T", "7", "9", "s"),
    "96s",
    "97s",
    "98s",
    "85s",
    "86s",
    "87s",
    "74s",
    "75s",
    "76s",
    "63s",
    "64s",
    "65s",
    "53s",
    "54s",
    "43s",
    ...run("A", "8", "J", "o"),
    ...run("K", "9", "J", "o"),
    "QTo",
    "QJo",
    "JTo",
    "T9o",
    "98o",
    "87o",
    "76o",
  ];

  const FLOPS = [
    { label: "Hoch, trocken", cards: ["As", "7d", "2c"] },
    { label: "Mittel", cards: ["Kh", "8s", "3d"] },
    { label: "Tief, verbunden", cards: ["8h", "7c", "6d"] },
  ];
  let fi = $state(0);
  const flop = $derived(FLOPS[fi].cards);

  function profile(range: string[], board: string[]) {
    const combos = rangeCombos(range, board);
    const n = { strong: 0, top: 0, weak: 0 };
    for (const c of combos) n[flopHit(c, board)]++;
    const total = combos.length;
    return { total, strong: n.strong / total, top: n.top / total, weak: n.weak / total };
  }

  const rows = $derived([
    { who: "Raiser", sub: "Open aus dem Cutoff", ...profile(RAISER, flop) },
    { who: "Caller", sub: "Verteidigt im Big Blind", ...profile(CALLER, flop) },
  ]);
  const lead = $derived(rows[0].strong + rows[0].top - (rows[1].strong + rows[1].top));
</script>

<div class="cb">
  <div class="flops" role="group" aria-label="Flop wählen">
    {#each FLOPS as f, i (f.label)}
      <button class="flop" class:on={fi === i} onclick={() => (fi = i)} aria-pressed={fi === i}>
        <CardRow cards={f.cards} size={26} />
        <span>{f.label}</span>
      </button>
    {/each}
  </div>

  <div class="rows">
    {#each rows as r (r.who)}
      <div class="row">
        <div class="who">
          <b>{r.who}</b>
          <span>{r.sub} · {int(r.total)} Kombos</span>
        </div>
        <div class="bar" role="img" aria-label="{r.who}: {pct(r.strong + r.top, 0)} Top Pair oder besser">
          <span class="s" style="width:{r.strong * 100}%"></span>
          <span class="t" style="width:{r.top * 100}%"></span>
        </div>
        <b class="num big">{pct(r.strong + r.top, 0)}</b>
      </div>
    {/each}
  </div>

  <div class="legend">
    <span><i class="s"></i>Zwei Paare oder besser</span>
    <span><i class="t"></i>Top Pair oder Overpair</span>
    <span><i class="w"></i>Weniger</span>
  </div>

  <p class="verdict">
    {#if lead > 0.03}
      Der Raiser trifft hier <b class="num">{num(lead * 100, 0)}</b> Prozentpunkte öfter: Eine C-Bet macht Druck auf eine Range, die oft nichts hat.
    {:else if lead < -0.03}
      Der Caller trifft hier <b class="num">{num(-lead * 100, 0)}</b> Prozentpunkte öfter: Der Raiser checkt hier häufiger.
    {:else}
      Beide treffen ähnlich oft. Kein klarer Vorteil.
    {/if}
  </p>
  <p class="faint fine">Beispiel-Ranges, auf jedem Flop exakt ausgezählt. Keine Solver-Strategie.</p>
</div>

<style>
  .cb {
    display: grid;
    gap: 14px;
  }
  .flops {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 8px;
  }
  .flop {
    display: grid;
    justify-items: center;
    gap: 6px;
    padding: 10px 6px 8px;
    border-radius: var(--r-sm);
    border: 1px solid var(--line);
    background: var(--bg);
    color: var(--text-2);
    font-size: 12.5px;
    cursor: pointer;
  }
  .flop:hover {
    border-color: var(--line-2);
  }
  .flop.on {
    border-color: var(--text);
    background: var(--surface-2);
    color: var(--text);
  }
  .rows {
    display: grid;
    gap: 14px;
  }
  .row {
    display: grid;
    grid-template-columns: minmax(130px, 32%) minmax(0, 1fr) 60px;
    align-items: center;
    gap: 14px;
  }
  .who {
    display: grid;
  }
  .who b {
    font-size: 15px;
  }
  .who span {
    color: var(--text-3);
    font-size: 12px;
  }
  .bar {
    display: flex;
    height: 26px;
    border-radius: 5px;
    overflow: hidden;
    background: var(--surface-3);
  }
  .bar span {
    transition: width 0.2s;
  }
  .s {
    background: var(--text);
  }
  .t {
    background: color-mix(in srgb, var(--text) 50%, var(--surface-3));
  }
  .w {
    background: var(--surface-3);
  }
  .big {
    font-size: 22px;
    font-weight: 600;
    text-align: right;
  }
  .legend {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 16px;
    color: var(--text-2);
    font-size: 12.5px;
  }
  .legend i {
    display: inline-block;
    width: 10px;
    height: 10px;
    margin-right: 6px;
    border-radius: 3px;
  }
  .verdict {
    margin: 0;
    padding: 10px 12px;
    border-radius: var(--r-sm);
    background: var(--bg);
    font-size: 13.5px;
  }
  .fine {
    margin: -6px 0 0;
    font-size: 12px;
  }
</style>
