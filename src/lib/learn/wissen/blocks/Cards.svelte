<script lang="ts">
  import CardRow from "./CardRow.svelte";

  let {
    rows,
    board = [],
    highlight = null,
  }: { rows: { label: string; cards: string[]; tag?: string }[]; board?: string[]; highlight?: string[] | null } = $props();
</script>

<div class="cards">
  {#if board.length}
    <div class="board">
      <span class="lbl">Board</span>
      <CardRow cards={board} size={44} {highlight} />
    </div>
  {/if}
  <div class="hands">
    {#each rows as r (r.label)}
      <div class="hand">
        <span class="lbl">{r.label}</span>
        <CardRow cards={r.cards} size={38} {highlight} />
        {#if r.tag}<span class="tag">{r.tag}</span>{/if}
      </div>
    {/each}
  </div>
</div>

<style>
  .cards {
    display: grid;
    gap: 16px;
  }
  .board {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    padding: 18px 12px 20px;
    border-radius: 999px;
    background: radial-gradient(ellipse at 50% 40%, #23312d 0%, #1b2623 62%, #16201d 100%);
    box-shadow:
      inset 0 0 0 1px rgba(0, 0, 0, 0.4),
      0 0 0 6px #1c1f24;
    margin: 6px;
  }
  .hands {
    display: grid;
    gap: 10px;
  }
  .hand {
    display: grid;
    grid-template-columns: 72px auto minmax(0, 1fr);
    align-items: center;
    gap: 14px;
  }
  .lbl {
    color: var(--text-3);
    font-size: 12px;
    font-weight: 600;
  }
  .tag {
    color: var(--text-2);
    font-size: 13px;
  }
</style>
