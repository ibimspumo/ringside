<script lang="ts">
  /**
   * How often a hand takes each action, as one labelled bar: the width is the share, the colour
   * the action (never a grade). Shares are whole percents; parts under 1 % are left out. `mine`
   * (index into `parts`) marks your own choice with an outline and "du" under it, in `mineColor`.
   */
  let {
    parts,
    label = "So oft spielt die Lösung jede Aktion",
    mine = null,
    mineColor = "var(--text)",
  }: {
    parts: { label: string; share: number; color: string }[];
    label?: string;
    mine?: number | null;
    mineColor?: string;
  } = $props();

  const shown = $derived(parts.map((p, i) => ({ ...p, i })).filter((p) => p.share >= 1));
  const total = $derived(shown.reduce((a, p) => a + p.share, 0) || 1);
  /** The middle of your part in % of the bar; null when the solution never plays it. */
  const mineAt = $derived.by(() => {
    let before = 0;
    for (const p of shown) {
      if (p.i === mine) return Math.min(97, Math.max(3, ((before + p.share / 2) / total) * 100));
      before += p.share;
    }
    return null;
  });
</script>

<div class="ui-mix" role="img" aria-label="{label}: {shown.map((p) => `${p.label} ${p.share} %`).join(', ')}">
  <!-- Two sizes of one action may share a label: the position is the key. -->
  {#each shown as p (p.i)}
    <!-- Widths follow the shares exactly; a narrow part keeps only its number, a tiny one only its colour. -->
    <i style:flex="{p.share} 1 0" style:--c={p.color} style:--mine={mineColor} class:mine={p.i === mine} class:narrow={p.share < 24} class:tiny={p.share < 8} title="{p.label}: in {p.share} % der Fälle">
      <span>{p.label}</span><b class="num">{p.share}&nbsp;%</b>
    </i>
  {/each}
</div>
{#if mineAt !== null}<span class="ui-mix-mine" aria-hidden="true"><span style:left="{mineAt}%" style:color={mineColor}>du</span></span>{/if}

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
  i.mine {
    box-shadow:
      inset 0 -3px 0 var(--c),
      inset 0 0 0 2px var(--mine);
  }
  .ui-mix-mine {
    position: relative;
    display: block;
    height: 16px;
  }
  .ui-mix-mine span {
    position: absolute;
    top: 0;
    transform: translateX(-50%);
    font-size: var(--fs-xs);
    font-weight: var(--fw-semibold);
    line-height: 1;
  }
  .ui-mix-mine span::before {
    content: "";
    display: block;
    width: 0;
    margin: 0 auto 2px;
    border: 4px solid transparent;
    border-top: 0;
    border-bottom-color: currentColor;
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
