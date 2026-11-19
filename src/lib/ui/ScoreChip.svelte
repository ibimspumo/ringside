<script lang="ts">
  /**
   * A score in the top bar that opens its page: the number with a trend mark, a quiet second
   * line, and a lock instead of the number where the value may not be shown. `compact` drops the
   * second line; the label (`title`) always says everything.
   */
  let {
    value,
    trend = null,
    sub,
    subTone = "neutral",
    locked = false,
    active = false,
    compact = false,
    title,
    onclick,
    ...rest
  }: {
    /** The number, or a word where there is none yet. */
    value: string;
    /** Direction of the last change; null hides the mark. */
    trend?: number | null;
    sub?: string;
    /** Colours the first word of `sub`, e.g. the change. */
    subTone?: "neutral" | "good" | "bad";
    locked?: boolean;
    active?: boolean;
    compact?: boolean;
    title: string;
    onclick: () => void;
    [key: `data-${string}`]: unknown;
  } = $props();

  const [lead, ...restSub] = $derived((sub ?? "").split(" · "));
</script>

<button type="button" class="ui-score-chip" class:active class:compact class:locked aria-current={active ? "page" : undefined} {title} aria-label={title} {onclick} {...rest}>
  <svg class="ic" viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
    {#if locked}
      <rect x="5" y="10.5" width="14" height="9.5" rx="2" /><path d="M8.5 10.5V8a3.5 3.5 0 0 1 7 0v2.5" />
    {:else}
      <path d="M4 19.5h16M5 15l4.5-4.5 3 3L19 7" />
    {/if}
  </svg>
  <span class="txt">
    <b class="num">
      {value}
      {#if trend !== null && !locked}
        <svg class="tr" class:up={trend > 0} class:down={trend < 0} viewBox="0 0 16 16" width="11" height="11" aria-hidden="true">
          {#if trend > 0}<path d="M8 3.5 13 9.5H3z" />{:else if trend < 0}<path d="M8 12.5 3 6.5h10z" />{:else}<path d="M3 7h10v2H3z" />{/if}
        </svg>
      {/if}
    </b>
    {#if sub && !compact}<small><span class:up={subTone === "good"} class:down={subTone === "bad"}>{lead}</span>{#if restSub.length}{` · ${restSub.join(" · ")}`}{/if}</small>{/if}
  </span>
</button>

<style>
  .ui-score-chip {
    display: inline-flex;
    gap: var(--sp-2);
    align-items: center;
    height: 40px;
    padding: 0 var(--sp-3) 0 var(--sp-2);
    border: 1px solid var(--line-2);
    border-radius: var(--r-md);
    background: var(--surface);
    color: var(--text);
    font: inherit;
    cursor: pointer;
    transition:
      background var(--dur-fast) var(--ease-standard),
      border-color var(--dur-fast) var(--ease-standard);
  }
  .ui-score-chip:hover {
    background: var(--surface-2);
  }
  .ui-score-chip.active {
    border-color: var(--sd);
    background: var(--surface-2);
  }
  .ui-score-chip:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: 2px;
  }
  .ui-score-chip.compact {
    padding: 0 var(--sp-2);
  }
  .ic {
    flex: none;
    fill: none;
    stroke: var(--sd);
    stroke-width: 1.7;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .locked .ic {
    stroke: var(--text-2);
  }
  .txt {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    line-height: 1.15;
    white-space: nowrap;
  }
  b {
    display: inline-flex;
    gap: 4px;
    align-items: center;
    font-size: var(--fs-lg);
    font-weight: var(--fw-bold);
  }
  .locked b {
    color: var(--text-2);
    font-size: var(--fs-md);
  }
  small {
    color: var(--text-2);
    font-size: var(--fs-xs);
  }
  .tr {
    fill: var(--text-3);
  }
  .tr.up {
    fill: var(--pos);
  }
  .tr.down {
    fill: var(--neg);
  }
  small .up {
    color: var(--pos);
  }
  small .down {
    color: var(--neg);
  }
</style>
