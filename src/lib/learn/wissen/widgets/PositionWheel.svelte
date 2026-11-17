<script lang="ts">
  import TableDiagram from "../blocks/TableDiagram.svelte";
  import { POSITION_INFO, POSITIONS, postflopOrder, type Seats } from "../seats";

  let seats = $state<Seats>(8);
  let street = $state<"pre" | "post">("pre");
  let picked = $state<string | null>("BTN");

  const pre = $derived(POSITIONS[seats]);
  const post = $derived(postflopOrder(seats));
  const seq = $derived(street === "pre" ? pre : post);
  const order = $derived(Object.fromEntries(seq.map((p, i) => [p, i + 1])));
  const sel = $derived(picked && pre.includes(picked) ? picked : null);
</script>

<div class="wheel">
  <div class="ctrl">
    <div class="seg" role="group" aria-label="Spieler am Tisch">
      {#each [6, 8, 9] as const as n (n)}
        <button class:on={seats === n} onclick={() => (seats = n)}>{n} Spieler</button>
      {/each}
    </div>
    <div class="seg" role="group" aria-label="Straße">
      <button class:on={street === "pre"} onclick={() => (street = "pre")}>Preflop</button>
      <button class:on={street === "post"} onclick={() => (street = "post")}>Ab dem Flop</button>
    </div>
  </div>

  <TableDiagram {seats} {order} selected={sel} onpick={(p) => (picked = p)} />

  <div class="info" aria-live="polite">
    {#if sel}
      <div class="who">
        <b class="num">{sel}</b>
        <span>{POSITION_INFO[sel].name}</span>
      </div>
      <p>{POSITION_INFO[sel].text}</p>
      <div class="facts">
        <span>Preflop <b class="num">{pre.indexOf(sel) + 1}.</b> von {seats}</span>
        <span>Ab dem Flop <b class="num">{post.indexOf(sel) + 1}.</b> von {seats}</span>
      </div>
    {:else}
      <p class="dim">Tipp einen Sitz an.</p>
    {/if}
  </div>
</div>

<style>
  .wheel {
    display: grid;
    gap: 10px;
  }
  .ctrl {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    justify-content: space-between;
  }
  .info {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    gap: 4px 18px;
    align-items: center;
    padding: 12px 14px;
    border-radius: var(--r-sm);
    background: var(--bg);
    min-height: 76px;
  }
  .who {
    display: grid;
    grid-row: 1 / 3;
  }
  .who b {
    font-size: 30px;
    font-weight: 600;
    line-height: 1;
  }
  .who span {
    color: var(--text-2);
    font-size: 12px;
  }
  p {
    margin: 0;
    font-size: 13.5px;
  }
  .facts {
    display: flex;
    gap: 18px;
    color: var(--text-2);
    font-size: 13px;
  }
  .facts b {
    color: var(--text);
    font-size: 15px;
  }
</style>
