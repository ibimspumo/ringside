<script lang="ts">
  import type { HTMLButtonAttributes } from "svelte/elements";

  /**
   * One entry of the app's navigation. `top` is a tab of the top bar (with `chevron` it opens a
   * menu), `sub` a tab of the row below it, `tile` a menu entry with a sentence. `status` is a
   * short live value ("4 Schwachstellen"), `locked` marks what is closed while PokerStars runs.
   * `icon` is an SVG path on a 24 × 24 grid; without it the entry is text only. Nothing in an entry
   * wraps; whoever places it makes room or shortens the value.
   */
  let {
    label,
    icon,
    size,
    active = false,
    current,
    status,
    tone = "neutral",
    text,
    locked = false,
    chevron,
    el = $bindable(),
    ...rest
  }: {
    label: string;
    icon?: string;
    size: "top" | "sub" | "tile";
    /** The page on screen. */
    active?: boolean;
    /** The area of the page on screen, for an entry that opens a menu. */
    current?: boolean;
    status?: string;
    tone?: "neutral" | "good" | "warn" | "bad";
    text?: string;
    locked?: boolean;
    chevron?: "down";
    el?: HTMLButtonElement;
  } & Omit<HTMLButtonAttributes, "children"> = $props();
</script>

<button
  bind:this={el}
  type="button"
  class="ui-nav {size}"
  class:on={active}
  class:cur={current && !active}
  aria-current={active ? "page" : current ? "true" : undefined}
  {...rest}
>
  {#if icon}
    <span class="ico">
      <svg viewBox="0 0 24 24" aria-hidden="true"><path d={icon} /></svg>
    </span>
  {/if}
  <span class="body">
    <span class="label">{label}</span>
    {#if size === "tile"}
      {#if text}<span class="text">{text}</span>{/if}
      {#if status || locked}<span class="status {tone}">{@render lock()}{status ?? ""}</span>{/if}
    {/if}
  </span>
  {#if size !== "tile" && (status || locked)}<span class="value {tone}">{@render lock()}{status ?? ""}</span>{/if}
  {#if chevron}
    <svg class="chev" viewBox="0 0 16 16" aria-hidden="true"><path d="M5 6.5 8 9.5l3-3" /></svg>
  {/if}
</button>

{#snippet lock()}
  {#if locked}<svg class="lock" viewBox="0 0 16 16" aria-label="gesperrt, solange PokerStars läuft" role="img"
      ><rect x="3.5" y="7" width="9" height="6.5" rx="1.5" /><path d="M5.5 7V5.2a2.5 2.5 0 0 1 5 0V7" /></svg
    >{/if}
{/snippet}

<style>
  .ui-nav {
    position: relative;
    display: flex;
    align-items: center;
    flex-shrink: 0;
    min-width: 0;
    border: 0;
    border-radius: var(--r-sm);
    background: transparent;
    color: var(--text-2);
    font: inherit;
    font-size: var(--fs-base);
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
  .on,
  .cur {
    color: var(--text);
  }
  /* The page or area on screen is underlined, so it reads without colour. */
  .on::before,
  .cur::before {
    content: "";
    position: absolute;
    left: var(--sp-3);
    right: var(--sp-3);
    height: 2px;
    border-radius: var(--r-pill);
    background: var(--text);
  }

  .ico {
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
  .body {
    display: grid;
    min-width: 0;
    line-height: var(--lh-tight);
  }
  .value,
  .status {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-1);
    font-family: var(--font-num);
    font-size: var(--fs-md);
    white-space: nowrap;
  }
  .value {
    flex-shrink: 0;
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

  /* Tab of the top bar; an open menu keeps the hover surface, the mark sits on the bar's edge. */
  .top {
    height: 36px;
    padding: 0 10px 0 var(--sp-3);
    gap: var(--sp-2);
    font-weight: var(--fw-medium);
  }
  .top[aria-expanded="true"] {
    color: var(--text);
    background: var(--surface-2);
  }
  .top::before {
    bottom: -10px;
  }

  /* Tab of the row under the top bar: quieter; the page on screen is a filled pill, so this level
     reads apart from the bar above and the page's own sections below. */
  .sub {
    height: 32px;
    padding: 0 10px;
    gap: var(--sp-2);
    font-size: var(--fs-md);
  }
  .sub .ico svg {
    width: 16px;
    height: 16px;
  }
  .sub.on {
    background: var(--surface-2);
  }
  .sub.on::before {
    content: none;
  }

  /* Menu entry: name, a sentence and the live value below. */
  .tile {
    align-items: flex-start;
    gap: var(--sp-3);
    width: 100%;
    padding: 10px var(--sp-3) 10px 10px;
    border-radius: var(--r-md);
    white-space: normal;
  }
  .tile .ico {
    width: 32px;
    height: 32px;
    border-radius: var(--r-md);
    background: var(--surface-2);
    color: var(--text);
    transition: background-color var(--dur-fast) var(--ease-standard);
  }
  .tile:hover .ico {
    background: var(--surface-3);
  }
  .tile.on {
    background: var(--surface-2);
  }
  .tile.on .ico {
    background: var(--text);
    color: var(--bg);
  }
  .tile.on::before {
    left: 0;
    right: auto;
    top: 14px;
    width: 3px;
    height: 24px;
  }
  .tile .label {
    font-weight: var(--fw-semibold);
    color: var(--text);
  }
  .tile .text {
    margin-top: 3px;
    font-size: var(--fs-base);
    line-height: var(--lh-base);
    color: var(--text-2);
    display: -webkit-box;
    overflow: hidden;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
  }
  .tile .status {
    margin-top: 5px;
  }
  .tile .status.neutral {
    color: var(--text-2);
  }

  .chev {
    width: 14px;
    height: 14px;
    flex-shrink: 0;
    color: var(--text-3);
    transition: transform var(--dur) var(--ease-standard);
  }
  [aria-expanded="true"] > .chev {
    transform: rotate(180deg);
  }
  @media (prefers-reduced-motion: reduce) {
    .ui-nav,
    .ico,
    .chev {
      transition: none;
    }
  }
</style>
