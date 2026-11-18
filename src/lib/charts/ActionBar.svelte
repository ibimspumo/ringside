<script lang="ts">
  import Tag from "../ui/Tag.svelte";
  /**
   * How often the whole range takes each action, as tiles over one stacked bar. A click on a tile
   * shows only the hands that take that action; a second click shows all again.
   */
  let {
    actions,
    selected,
    onSelect,
  }: {
    actions: { name: string; pct: number; color: string; fold: boolean }[];
    selected: number | null;
    onSelect: (i: number | null) => void;
  } = $props();

  const pct = (x: number) => x.toLocaleString("de-DE", { minimumFractionDigits: 1, maximumFractionDigits: 1 });
</script>

<div class="action-bar">
  <div class="acts" role="group" aria-label="Aktionen – Klick zeigt nur diese Hände">
    {#each actions as a, i (i)}
      <Tag selected={selected === i} onclick={() => onSelect(selected === i ? null : i)}>
        <span class="act" class:off={selected !== null && selected !== i}>
          <i class="sw" style="background:{a.color}"></i>
          <span>{a.name}</span>
          <b class="num">{pct(a.pct)} %</b>
        </span>
      </Tag>
    {/each}
  </div>
  <div class="stack" aria-hidden="true">
    {#each actions as a, i (i)}
      {#if a.pct > 0}<i style="flex:{a.pct} 1 0;background:{a.color}"></i>{/if}
    {/each}
  </div>
  <p class="help">
    {#if selected === null}Klick auf eine Aktion zeigt nur ihre Hände.{:else}Nur „{actions[selected].name}“ hervorgehoben – erneut klicken für alle.{/if}
  </p>
</div>

<style>
  .action-bar {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }
  .acts {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-2);
  }
  .act {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-2);
    transition: opacity var(--dur) var(--ease-standard);
  }
  .act.off {
    opacity: 0.55;
  }
  .act b {
    font-weight: 600;
  }
  .sw {
    width: 10px;
    height: 10px;
    border-radius: 2px;
  }
  .stack {
    display: flex;
    gap: 2px;
    height: 6px;
    border-radius: 3px;
    overflow: hidden;
  }
  .stack i {
    transition: flex-grow var(--dur-slow) var(--ease-standard);
  }
  .help {
    margin: 0;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
</style>
