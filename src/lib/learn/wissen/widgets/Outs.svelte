<script lang="ts">
  import PlayingCard from "../../../ui/PlayingCard.svelte";
  import CardRow from "../blocks/CardRow.svelte";
  import { hitChance, pct } from "../math";
  import { CATEGORY, RANKS, SUITS, outsTo } from "../poker";

  // Category index each scenario aims for: 3 Drilling, 4 Straße, 5 Flush.
  const SCENARIOS = [
    { name: "Flush-Draw", hole: ["Jh", "Th"], board: ["Ah", "8h", "4c"], target: 5 },
    { name: "Beidseitiger Straßen-Draw", hole: ["9c", "8d"], board: ["7h", "6s", "2c"], target: 4 },
    { name: "Gutshot", hole: ["Kd", "Jc"], board: ["Qh", "9s", "3d"], target: 4 },
    { name: "Flush- und Straßen-Draw", hole: ["9h", "8h"], board: ["Th", "7h", "2c"], target: 4 },
    { name: "Set suchen", hole: ["5s", "5d"], board: ["Kc", "9h", "2d"], target: 3 },
  ];

  let idx = $state(0);
  let picked = $state<string[]>([]);
  let revealed = $state(false);

  const sc = $derived(SCENARIOS[idx]);
  const outs = $derived(outsTo(sc.hole, sc.board, sc.target));
  const known = $derived(new Set([...sc.hole, ...sc.board]));
  const ROWS = SUITS.split("").map((s) => [...RANKS].reverse().map((r) => r + s));

  const hits = $derived(picked.filter((c) => outs.includes(c)).length);
  const wrong = $derived(picked.length - hits);

  function toggle(c: string) {
    if (revealed || known.has(c)) return;
    picked = picked.includes(c) ? picked.filter((x) => x !== c) : [...picked, c];
  }
  function next() {
    idx = (idx + 1) % SCENARIOS.length;
    picked = [];
    revealed = false;
  }
</script>

<div class="outs">
  <div class="spot">
    <div class="hole">
      <span class="lbl">Du</span>
      <CardRow cards={sc.hole} size={40} />
    </div>
    <div class="board">
      <CardRow cards={sc.board} size={40} />
    </div>
    <div class="goal">
      <span class="lbl">Ziel</span>
      <b>{CATEGORY[sc.target]} oder besser</b>
    </div>
  </div>

  <div class="deck" role="group" aria-label="Restliche Karten">
    {#each ROWS as row, ri (ri)}
      {#each row as c (c)}
        {#if known.has(c)}
          <span class="gap"></span>
        {:else}
          {@const isOut = outs.includes(c)}
          {@const isPicked = picked.includes(c)}
          <button
            class="cell"
            class:picked={isPicked}
            class:good={revealed && isPicked && isOut}
            class:bad={revealed && isPicked && !isOut}
            class:missed={revealed && !isPicked && isOut}
            class:dimmed={revealed && !isOut && !isPicked}
            aria-pressed={isPicked}
            onclick={() => toggle(c)}
          >
            <PlayingCard card={c} size={26} />
          </button>
        {/if}
      {/each}
    {/each}
  </div>

  <div class="foot">
    {#if revealed}
      <div class="score">
        <span><b class="num pos">{hits}</b> von <b class="num">{outs.length}</b> Outs gefunden</span>
        {#if wrong}<span><b class="num neg">{wrong}</b> falsch</span>{/if}
        <span class="legend"><i class="g"></i>richtig <i class="r"></i>falsch <i class="m"></i>übersehen</span>
        <span class="dim">Turn <b class="num">{pct(hitChance(outs.length, 1, 47))}</b> · bis River <b class="num">{pct(hitChance(outs.length, 2, 47))}</b></span>
      </div>
      <button class="btn" onclick={next}>Nächstes Beispiel</button>
    {:else}
      <span class="dim">{sc.name} · <b class="num">{picked.length}</b> markiert</span>
      <button class="btn primary" onclick={() => (revealed = true)}>Auflösen</button>
    {/if}
  </div>
</div>

<style>
  .outs {
    display: grid;
    gap: 14px;
  }
  .spot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    flex-wrap: wrap;
  }
  .hole,
  .goal {
    display: grid;
    gap: 5px;
  }
  .goal {
    text-align: right;
    font-size: 14px;
  }
  .lbl {
    color: var(--text-3);
    font-size: 12px;
    font-weight: 600;
  }
  .board {
    padding: 12px 22px;
    border-radius: 999px;
    background: radial-gradient(ellipse at 50% 40%, #23312d 0%, #1b2623 62%, #16201d 100%);
    box-shadow: 0 0 0 5px #1c1f24;
  }
  .deck {
    display: grid;
    grid-template-columns: repeat(13, minmax(0, 1fr));
    gap: 5px 4px;
    justify-items: center;
    padding: 12px;
    border-radius: var(--r);
    background: var(--bg);
  }
  .gap {
    width: 26px;
    height: 36px;
    border-radius: 4px;
    border: 1px dashed var(--line-2);
  }
  .cell {
    padding: 0;
    border: 0;
    background: none;
    border-radius: 4px;
    cursor: pointer;
    transition:
      transform 0.1s,
      opacity 0.15s;
    line-height: 0;
  }
  .cell:hover {
    transform: translateY(-2px);
  }
  .cell.picked {
    transform: translateY(-3px);
    box-shadow: 0 0 0 2px var(--text);
  }
  .cell.good {
    box-shadow: 0 0 0 2.5px var(--pos);
  }
  .cell.bad {
    box-shadow: 0 0 0 2.5px var(--neg);
    opacity: 0.7;
  }
  .cell.missed {
    box-shadow: 0 0 0 2px var(--amber);
    outline: 2px dashed var(--amber);
    outline-offset: 2px;
  }
  .cell.dimmed {
    opacity: 0.22;
  }
  .foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    min-height: 34px;
    font-size: 13.5px;
  }
  .score {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 16px;
  }
  .score b {
    font-size: 16px;
  }
  .legend {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: var(--text-3);
    font-size: 12px;
  }
  .legend i {
    width: 10px;
    height: 10px;
    margin-left: 4px;
    border-radius: 3px;
    border: 2px solid var(--pos);
  }
  .legend i.r {
    border-color: var(--neg);
  }
  .legend i.m {
    border: 2px dashed var(--amber);
  }
</style>
