<script lang="ts">
  /**
   * How often a hand takes each action, as one labelled bar: the width is the share, the colour
   * the action (never a grade). Shares are whole percents; parts under 1 % are left out.
   */
  let {
    parts,
    label = "So oft spielt die Lösung jede Aktion",
  }: {
    parts: { label: string; share: number; color: string }[];
    label?: string;
  } = $props();

  const shown = $derived(parts.filter((p) => p.share >= 1));
</script>

<div class="ui-mix" role="img" aria-label="{label}: {shown.map((p) => `${p.label} ${p.share} %`).join(', ')}">
  <!-- Two sizes of one action may share a label: the position is the key. -->
  {#each shown as p, i (i)}
    <!-- Widths follow the shares exactly; a narrow part keeps only its number, a tiny one only its colour. -->
    <i style:flex="{p.share} 1 0" style:--c={p.color} class:narrow={p.share < 24} class:tiny={p.share < 8} title="{p.label}: in {p.share} % der Fälle">
      <span>{p.label}</span><b class="num">{p.share}&nbsp;%</b>
    </i>
  {/each}
</div>

<style>
  .ui-mix {
    display: flex;
    gap: 2px;
    height: 28px;
    border-radius: var(--r-xs);
    overflow: hidden;
  }
  i {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-2);
    min-width: 0;
    padding: 0 var(--sp-2);
    background: color-mix(in srgb, var(--c) 24%, var(--surface-2));
    box-shadow: inset 0 -3px 0 var(--c);
    color: var(--text);
    font-size: var(--fs-xs);
    font-style: normal;
    white-space: nowrap;
    overflow: hidden;
  }
  .narrow span,
  .tiny b {
    display: none;
  }
  .narrow {
    justify-content: center;
    padding: 0 2px;
  }
  span {
    overflow: hidden;
    text-overflow: ellipsis;
    font-weight: var(--fw-medium);
  }
  b {
    font-weight: var(--fw-bold);
  }
</style>
