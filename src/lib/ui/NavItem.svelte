<script lang="ts">
  import type { HTMLButtonAttributes } from "svelte/elements";

  /**
   * One entry of the side bar. `side` is a row with icon, name and a short live value at the
   * right ("4", "Analyse läuft"); `rail` is the icon alone for the narrow bar, its value a small
   * mark on the icon and its name in the tooltip. `locked` marks what is closed while a poker client
   * runs. `icon` is an SVG path on a 24 × 24 grid.
   */
  let {
    label,
    icon,
    size = "side",
    active = false,
    status,
    tone = "neutral",
    locked = false,
    title,
    el = $bindable(),
    ...rest
  }: {
    label: string;
    icon: string;
    size?: "side" | "rail";
    /** The page on screen. */
    active?: boolean;
    status?: string;
    tone?: "neutral" | "good" | "warn" | "bad";
    locked?: boolean;
    /** The tooltip; the rail always names the entry in it. */
    title?: string;
    el?: HTMLButtonElement;
  } & Omit<HTMLButtonAttributes, "children" | "title"> = $props();

  const tip = $derived(size === "rail" ? [label, locked ? "gesperrt" : status, title].filter(Boolean).join(" · ") : title);
  /** A rail mark shows a short count; longer values only colour the dot. */
  const mark = $derived(status && status.length <= 3 ? status : "");
</script>

<button
  bind:this={el}
  type="button"
  class="ui-nav {size}"
  class:on={active}
  aria-current={active ? "page" : undefined}
  aria-label={size === "rail" ? tip : undefined}
  title={tip}
  {...rest}
>
  <span class="ico">
    <svg viewBox="0 0 24 24" aria-hidden="true"><path d={icon} /></svg>
    {#if size === "rail" && locked}
      <span class="mark neutral"
        ><svg class="lock" viewBox="0 0 16 16" aria-hidden="true"><rect x="3.5" y="7" width="9" height="6.5" rx="1.5" /><path d="M5.5 7V5.2a2.5 2.5 0 0 1 5 0V7" /></svg></span
      >
    {:else if size === "rail" && status}
      <span class="mark {tone}" class:dot={!mark}>{mark}</span>
    {/if}
  </span>
  {#if size === "side"}
    <span class="label">{label}</span>
    {#if status || locked}
      <span class="value {tone}">
        {#if locked}<svg class="lock" viewBox="0 0 16 16" aria-label="gesperrt, solange ein Poker-Client läuft" role="img"
            ><rect x="3.5" y="7" width="9" height="6.5" rx="1.5" /><path d="M5.5 7V5.2a2.5 2.5 0 0 1 5 0V7" /></svg
          >{/if}{status ?? ""}
      </span>
    {/if}
  {/if}
</button>

<style>
  .ui-nav {
    position: relative;
    display: flex;
    align-items: center;
    min-width: 0;
    border: 0;
    border-radius: var(--r-sm);
    background: transparent;
    color: var(--text-2);
    font: inherit;
    text-align: left;
    white-space: nowrap;
    cursor: pointer;
    transition:
      background-color var(--dur-fast) var(--ease-standard),
      color var(--dur-fast) var(--ease-standard);
  }
  .ui-nav:hover {
    color: var(--text);
    background: var(--surface);
  }
  .ui-nav:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: -2px;
  }
  /* The page on screen: filled, and a bar at its left edge, so it reads without colour. */
  .on,
  .on:hover {
    color: var(--text);
    background: var(--surface-2);
  }
  .on::before {
    content: "";
    position: absolute;
    left: 0;
    top: 7px;
    bottom: 7px;
    width: 3px;
    border-radius: var(--r-pill);
    background: var(--text);
  }

  .ico {
    position: relative;
    display: grid;
    place-items: center;
    flex-shrink: 0;
  }
  svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 1.6;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .ico svg {
    width: 18px;
    height: 18px;
  }

  .side {
    width: 100%;
    height: 32px;
    gap: var(--sp-3);
    padding: 0 10px 0 var(--sp-3);
    font-size: var(--fs-base);
  }
  .label {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .value {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-1);
    flex-shrink: 0;
    font-family: var(--font-num);
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .lock {
    width: 12px;
    height: 12px;
    flex-shrink: 0;
    stroke-width: 1.5;
  }
  .good {
    color: var(--pos);
  }
  .warn {
    color: var(--amber);
  }
  .bad {
    color: var(--danger-text);
  }

  .rail {
    justify-content: center;
    width: 44px;
    height: 40px;
  }
  .rail .ico svg {
    width: 20px;
    height: 20px;
  }
  .rail.on::before {
    top: 10px;
    bottom: 10px;
  }
  .mark {
    position: absolute;
    top: -7px;
    left: 12px;
    min-width: 16px;
    height: 16px;
    padding: 0 4px;
    border: 2px solid var(--surface-sunken);
    border-radius: var(--r-pill);
    background: var(--surface-3);
    font-family: var(--font-num);
    font-size: var(--fs-xs);
    line-height: 12px;
    text-align: center;
    color: var(--text);
  }
  .mark .lock {
    display: block;
    width: 10px;
    height: 12px;
    margin: 0 -2px;
  }
  .mark.dot {
    top: -3px;
    left: 14px;
    min-width: 0;
    width: 10px;
    height: 10px;
    padding: 0;
  }
  .mark.good {
    background: var(--pos);
    color: var(--bg);
  }
  .mark.warn {
    background: var(--amber);
    color: var(--bg);
  }
  .mark.bad {
    background: var(--neg);
    color: var(--text);
  }
  @media (prefers-reduced-motion: reduce) {
    .ui-nav {
      transition: none;
    }
  }
</style>
