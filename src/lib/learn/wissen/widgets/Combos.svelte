<script lang="ts">
  import CardRow from "../blocks/CardRow.svelte";
  import { combosOf } from "../poker";

  let { hand = "AA", blocked = false }: { hand?: "AA" | "AK"; blocked?: boolean } = $props();

  const TYPES = [
    { id: "AA", label: "Paar AA", classes: ["AA"] },
    { id: "AKs", label: "Suited AKs", classes: ["AKs"] },
    { id: "AKo", label: "Offsuit AKo", classes: ["AKo"] },
    { id: "AK", label: "AK gesamt", classes: ["AKs", "AKo"] },
  ];
  const BLOCKER = "As";

  let type = $derived(hand);
  let block = $derived(blocked);

  const t = $derived(TYPES.find((x) => x.id === type) ?? TYPES[0]);
  const all = $derived(t.classes.flatMap(combosOf));
  const live = $derived(all.filter((c) => !block || !c.includes(BLOCKER)));
</script>

<div class="combos">
  <div class="ctrl">
    <div class="seg" role="group" aria-label="Handtyp">
      {#each TYPES as x (x.id)}
        <button class:on={type === x.id} onclick={() => (type = x.id as typeof type)}>{x.label}</button>
      {/each}
    </div>
    <label class="check">
      <input type="checkbox" bind:checked={block} />
      <span>Du hältst <CardRow cards={[BLOCKER]} size={18} /></span>
    </label>
  </div>

  <div class="count">
    <b class="num">{live.length}</b>
    {#if block && live.length !== all.length}<span class="num was">von {all.length}</span>{/if}
    <span class="dim">Kombos</span>
  </div>

  <div class="list">
    {#each all as c (c.join(""))}
      {@const gone = block && c.includes(BLOCKER)}
      <span class="combo" class:gone>
        <CardRow cards={c} size={28} />
      </span>
    {/each}
  </div>
</div>

<style>
  .combos {
    display: grid;
    gap: 14px;
  }
  .ctrl {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }
  .check {
    align-items: center;
    font-size: 13px;
    color: var(--text-2);
  }
  .check input {
    margin: 0;
  }
  .check span {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .count {
    display: flex;
    align-items: baseline;
    gap: 10px;
  }
  .count b {
    font-size: 48px;
    font-weight: 600;
    line-height: 1;
  }
  .was {
    color: var(--text-3);
    font-size: 20px;
    text-decoration: line-through;
  }
  .list {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(76px, 1fr));
    gap: 10px;
    padding: 14px;
    border-radius: var(--r);
    background: var(--bg);
    min-height: 90px;
    align-content: start;
  }
  .combo {
    position: relative;
    display: flex;
    justify-content: center;
    padding: 6px 0;
    border-radius: 6px;
    background: var(--surface-2);
    transition: opacity 0.15s;
  }
  .combo.gone {
    opacity: 0.25;
  }
  .combo.gone::after {
    content: "";
    position: absolute;
    left: 10%;
    right: 10%;
    top: 50%;
    border-top: 2px solid var(--neg);
    transform: rotate(-14deg);
  }
</style>
