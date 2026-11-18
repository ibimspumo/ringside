<script lang="ts">
  import type { Snippet } from "svelte";
  import type { HTMLButtonAttributes } from "svelte/elements";
  import Kbd from "./Kbd.svelte";

  /**
   * One button for every view. `primary` is the single next step of a screen ("Nächste Hand"),
   * `secondary` the alternatives, `quiet` the incidental ones, `danger` anything that deletes;
   * `danger-quiet` is a destructive action that stays in the background until hovered.
   */
  let {
    variant = "secondary",
    size = "md",
    kbd,
    loading = false,
    full = false,
    icon,
    children,
    ref = $bindable(),
    disabled,
    type = "button",
    class: extra = "",
    ...rest
  }: {
    variant?: "primary" | "secondary" | "quiet" | "danger" | "danger-quiet";
    size?: "sm" | "md" | "lg";
    /** Shortcut shown inside the button, e.g. "Leertaste" or "Strg + K". */
    kbd?: string | string[];
    loading?: boolean;
    full?: boolean;
    icon?: Snippet;
    children?: Snippet;
    ref?: HTMLButtonElement;
  } & HTMLButtonAttributes = $props();
</script>

<button
  bind:this={ref}
  {type}
  class="ui-btn {variant} {size} {extra}"
  class:full
  class:loading
  disabled={disabled || loading}
  aria-busy={loading || undefined}
  {...rest}
>
  {#if loading}<span class="spin" aria-hidden="true"></span>{:else if icon}<span class="icon" aria-hidden="true">{@render icon()}</span>{/if}
  {#if children}<span class="label">{@render children()}</span>{/if}
  {#if kbd}<Kbd keys={kbd} />{/if}
</button>

<style>
  .ui-btn {
    --h: 32px;
    --kbd-h: 18px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: var(--sp-2);
    height: var(--h);
    padding: 0 var(--sp-3);
    border: 1px solid var(--line-2);
    border-radius: var(--r-sm);
    background: var(--secondary);
    color: var(--text);
    font: inherit;
    font-size: var(--fs-md);
    font-weight: var(--fw-medium);
    line-height: 1;
    white-space: nowrap;
    cursor: pointer;
    transition:
      background var(--dur-fast) var(--ease-standard),
      border-color var(--dur-fast) var(--ease-standard),
      color var(--dur-fast) var(--ease-standard),
      transform var(--dur-fast) var(--ease-standard);
  }
  .sm {
    --h: 28px;
    --kbd-h: 16px;
    padding: 0 10px;
    font-size: var(--fs-sm);
    gap: 6px;
  }
  .lg {
    --h: 40px;
    --kbd-h: 20px;
    padding: 0 var(--sp-4);
    font-size: var(--fs-base);
    border-radius: var(--r-md);
  }
  .full {
    width: 100%;
  }

  .secondary:is(:hover, [data-force="hover"]) {
    background: var(--secondary-hover);
  }
  .primary {
    background: var(--primary);
    border-color: var(--primary);
    color: var(--primary-text);
    font-weight: var(--fw-semibold);
  }
  .primary:is(:hover, [data-force="hover"]) {
    background: var(--primary-hover);
    border-color: var(--primary-hover);
  }
  .quiet {
    background: transparent;
    border-color: transparent;
    color: var(--text-2);
  }
  .quiet:is(:hover, [data-force="hover"]) {
    background: var(--surface-2);
    color: var(--text);
  }
  .danger {
    background: color-mix(in srgb, var(--danger) 12%, var(--surface-2));
    border-color: color-mix(in srgb, var(--danger) 40%, transparent);
    color: var(--danger-text);
  }
  .danger:is(:hover, [data-force="hover"]) {
    background: color-mix(in srgb, var(--danger) 22%, var(--surface-2));
  }
  .danger-quiet {
    background: transparent;
    border-color: transparent;
    color: var(--danger-text);
  }
  .danger-quiet:is(:hover, [data-force="hover"]) {
    background: color-mix(in srgb, var(--danger) 12%, var(--surface-2));
  }
  .ui-btn:active:not(:disabled) {
    transform: translateY(1px);
  }
  .ui-btn:is(:focus-visible, [data-force="focus"]) {
    outline: 2px solid var(--focus);
    outline-offset: 2px;
  }
  .ui-btn:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .ui-btn.loading {
    opacity: 0.8;
    cursor: progress;
  }

  .ui-btn :global(kbd) {
    opacity: 0.72;
  }
  .ui-btn > :global(:is(kbd, .combo):last-child) {
    margin-right: -4px;
  }
  .icon {
    display: inline-grid;
    margin-left: -2px;
  }
  .icon :global(svg) {
    width: 16px;
    height: 16px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.7;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .spin {
    width: 14px;
    height: 14px;
    margin-left: -2px;
    border: 2px solid currentColor;
    border-right-color: transparent;
    border-radius: 50%;
    animation: spin var(--dur-loop) var(--ease-spin) infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .spin {
      animation: none;
      border-right-color: currentColor;
      opacity: 0.6;
    }
  }
</style>
