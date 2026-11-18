<script lang="ts">
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

  const pct = (x: number) => x.toLocaleString("de-DE", { maximumFractionDigits: 1 });
</script>

<div class="action-bar">
  <div class="tiles" role="group" aria-label="Aktionen – Klick zeigt nur diese Hände">
    {#each actions as a, i (i)}
      <button
        type="button"
        class="tile"
        class:fold={a.fold}
        class:on={selected === i}
        class:off={selected !== null && selected !== i}
        style="--c:{a.color}"
        aria-pressed={selected === i}
        onclick={() => onSelect(selected === i ? null : i)}
      >
        <span class="name">{a.name}</span>
        <b class="num">{pct(a.pct)} %</b>
      </button>
    {/each}
  </div>
  <div class="stack" aria-hidden="true">
    {#each actions as a, i (i)}
      {#if a.pct > 0}<i style="flex:{a.pct} 1 0;background:{a.fold ? 'var(--surface-3)' : a.color}"></i>{/if}
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
    gap: 6px;
  }
  .tiles {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .tile {
    flex: 1 1 110px;
    max-width: 220px;
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 10px;
    height: 34px;
    padding: 0 10px 0 12px;
    border: 1px solid transparent;
    border-radius: var(--r-sm);
    background: color-mix(in srgb, var(--c) 22%, var(--surface));
    box-shadow: inset 3px 0 0 var(--c);
    color: var(--text);
    font-size: 13px;
    line-height: 32px;
    cursor: pointer;
    transition:
      opacity var(--dur) var(--ease-out),
      border-color var(--dur-fast) var(--ease-out);
  }
  .tile.fold {
    background: var(--surface-2);
    box-shadow: inset 3px 0 0 var(--line-2);
  }
  .tile:hover {
    border-color: var(--line-2);
  }
  .tile.on {
    border-color: var(--text-2);
  }
  .tile.off {
    opacity: 0.5;
  }
  .tile:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: 2px;
  }
  .name {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .tile b {
    font-size: 14px;
    font-weight: 600;
  }
  .stack i {
    transition: flex-grow var(--dur-slow) var(--ease-out);
  }
  .stack {
    display: flex;
    gap: 2px;
    height: 6px;
    border-radius: 3px;
    overflow: hidden;
  }
  .help {
    margin: 0;
    font-size: 12px;
    color: var(--text-3);
  }
</style>
