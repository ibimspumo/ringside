<script lang="ts">
  import type { HTMLButtonAttributes } from "svelte/elements";

  /**
   * One entry of the app's navigation. `lg` is a highlighted entry with a status line, `md` the
   * normal row with a short value on the right, `sm` a quiet row for tools, `rail` only the icon,
   * `top` a tab of a top bar (with `chevron` it opens a menu), `sub` a tab of the row below it,
   * `tile` a menu or start-page entry
   * with a sentence, `group` the small heading that folds a group. `icon` is an SVG path on a
   * 24 × 24 grid. `locked`: closed while PokerStars runs.
   */
  let {
    label,
    icon,
    size = "md",
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
    size?: "lg" | "md" | "sm" | "rail" | "top" | "sub" | "tile" | "group";
    /** The page on screen. */
    active?: boolean;
    /** The area of the page on screen, for group or menu entries; marked quieter than `active`. */
    current?: boolean;
    status?: string;
    tone?: "neutral" | "good" | "warn" | "bad";
    text?: string;
    locked?: boolean;
    /** Opens a group or menu: the arrow shows which way. */
    chevron?: "down" | "right";
    el?: HTMLButtonElement;
  } & Omit<HTMLButtonAttributes, "children"> = $props();
</script>

<button
  bind:this={el}
  type="button"
  class="ui-nav {size}"
  class:on={active}
  class:cur={current && !active}
  class:locked
  aria-current={active ? "page" : current ? "true" : undefined}
  aria-label={size === "rail" ? label : undefined}
  {...rest}
>
  {#if icon}
    <span class="ico">
      <svg viewBox="0 0 24 24" aria-hidden="true"><path d={icon} /></svg>
    </span>
  {/if}
  {#if size !== "rail"}
    <span class="body">
      <span class="label">{label}</span>
      {#if text && size === "tile"}<span class="text">{text}</span>{/if}
      {#if status && (size === "lg" || size === "tile")}<span class="status {tone}">{@render lock()}{status}</span>{/if}
    </span>
    {#if status && size !== "lg" && size !== "tile"}<span class="value {tone}">{@render lock()}{status}</span>
    {:else if locked && size !== "lg" && size !== "tile"}<span class="value">{@render lock()}</span>{/if}
    {#if chevron}
      <svg class="chev {chevron}" viewBox="0 0 16 16" aria-hidden="true"><path d="M5 6.5 8 9.5l3-3" /></svg>
    {/if}
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
    gap: var(--sp-3);
    width: 100%;
    min-width: 0;
    border: 0;
    border-radius: var(--r-sm);
    background: transparent;
    color: var(--text-2);
    font: inherit;
    font-size: var(--fs-base);
    text-align: left;
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
  .ui-nav.on {
    color: var(--text);
    background: var(--surface-2);
  }
  /* The page on screen also gets a mark at the edge, so it reads without colour. */
  .ui-nav.on::before {
    content: "";
    position: absolute;
    left: 0;
    top: 50%;
    width: 3px;
    height: 16px;
    margin-top: -8px;
    border-radius: var(--r-pill);
    background: var(--text);
  }
  .ui-nav.cur {
    color: var(--text);
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
    flex: 1;
    min-width: 0;
    line-height: var(--lh-tight);
  }
  .label,
  .text,
  .status {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .value,
  .status {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-1);
    font-family: var(--font-num);
  }
  .value {
    flex-shrink: 0;
    font-size: var(--fs-sm);
    color: var(--text-3);
    white-space: nowrap;
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

  .md {
    height: 34px;
    padding: 0 var(--sp-3) 0 10px;
  }

  /* Highlighted: icon on a plate, status line below the name. */
  .lg {
    min-height: 46px;
    padding: var(--sp-2) 10px var(--sp-2) 6px;
    gap: 10px;
  }
  .lg .ico {
    width: 32px;
    height: 32px;
    border-radius: var(--r-md);
    background: var(--surface-2);
    color: var(--text);
    transition:
      background-color var(--dur-fast) var(--ease-standard),
      color var(--dur-fast) var(--ease-standard);
  }
  .lg .label {
    font-weight: var(--fw-semibold);
    color: var(--text);
  }
  .lg .status {
    margin-top: 3px;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .lg:hover .ico {
    background: var(--surface-3);
  }
  .lg.on .ico {
    background: var(--text);
    color: var(--bg);
  }

  /* Quiet: tools that step back. */
  .sm {
    height: 28px;
    padding: 0 var(--sp-3) 0 10px;
    font-size: var(--fs-md);
    color: var(--text-2);
    gap: 10px;
  }
  .sm .ico svg {
    width: 16px;
    height: 16px;
  }

  .rail {
    justify-content: center;
    width: 40px;
    height: 40px;
    padding: 0;
  }
  .rail .ico svg {
    width: 20px;
    height: 20px;
  }
  .rail.on::before {
    left: -6px;
  }

  /* Tab of a top bar: the open menu keeps the hover surface. */
  .top {
    flex-shrink: 0;
    width: auto;
    height: 36px;
    padding: 0 10px 0 var(--sp-3);
    gap: var(--sp-2);
    font-weight: var(--fw-medium);
  }
  .top[aria-expanded="true"] {
    color: var(--text);
    background: var(--surface-2);
  }
  .top.cur::before,
  .top.on::before {
    content: "";
    position: absolute;
    left: var(--sp-3);
    right: var(--sp-3);
    bottom: -11px;
    top: auto;
    width: auto;
    height: 2px;
    margin: 0;
    border-radius: var(--r-pill);
    background: var(--text);
  }
  .top.on {
    background: transparent;
  }

  /* Tab of the row under a top bar: quiet, the open page underlined like the bar above. */
  .sub {
    flex-shrink: 0;
    width: auto;
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
    background: transparent;
  }
  .sub.on::before {
    left: 10px;
    right: 10px;
    top: auto;
    bottom: -4px;
    width: auto;
    height: 2px;
    margin: 0;
  }

  /* Menu and start-page entry: name, a sentence, the live value at the right. */
  .tile {
    align-items: flex-start;
    padding: 10px var(--sp-3) 10px 10px;
    border-radius: var(--r-md);
  }
  .tile .ico {
    width: 32px;
    height: 32px;
    border-radius: var(--r-md);
    background: var(--surface-2);
    color: var(--text);
  }
  .tile:hover .ico {
    background: var(--surface-3);
  }
  .tile.on .ico {
    background: var(--text);
    color: var(--bg);
  }
  .tile .label {
    font-weight: var(--fw-semibold);
    color: var(--text);
  }
  .tile .text {
    margin-top: 3px;
    font-size: var(--fs-sm);
    line-height: var(--lh-base);
    color: var(--text-2);
    white-space: normal;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
  }
  .tile .status {
    margin-top: 5px;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .tile.on::before {
    top: 14px;
    margin-top: 0;
    height: 24px;
  }

  /* Group heading that folds its entries. */
  .group {
    height: 24px;
    padding: 0 10px;
    gap: 6px;
    font-size: var(--fs-xs);
    font-weight: var(--fw-semibold);
    letter-spacing: 0.02em;
    color: var(--text-3);
  }
  .group:hover {
    background: transparent;
    color: var(--text-2);
  }
  .group .body {
    flex: 0 1 auto;
  }
  .group .value {
    margin-left: auto;
    font-size: var(--fs-xs);
  }

  .chev {
    width: 14px;
    height: 14px;
    flex-shrink: 0;
    color: var(--text-3);
    transition: transform var(--dur) var(--ease-standard);
  }
  .chev.right {
    transform: rotate(-90deg);
  }
  [aria-expanded="true"] > .chev.down {
    transform: rotate(180deg);
  }
  [aria-expanded="true"] > .chev.right {
    transform: rotate(0deg);
  }
  /* Low windows (700 px at the minimum size): the whole side bar stays visible. */
  @media (max-height: 760px) {
    .md {
      height: 28px;
    }
    .sm {
      height: 26px;
    }
    .lg {
      min-height: 36px;
      padding-block: 2px;
    }
    .lg .ico {
      width: 28px;
      height: 28px;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .ui-nav,
    .ico,
    .chev {
      transition: none;
    }
  }
</style>
