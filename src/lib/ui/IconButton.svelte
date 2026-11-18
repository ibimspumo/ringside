<script lang="ts">
  import type { Snippet } from "svelte";
  import type { HTMLButtonAttributes } from "svelte/elements";

  /** Square button with only an icon. `label` is required: it names the button for screen readers and on hover. */
  let {
    label,
    size = "md",
    variant = "quiet",
    pressed,
    children,
    type = "button",
    ...rest
  }: {
    label: string;
    size?: "sm" | "md" | "lg";
    variant?: "quiet" | "secondary";
    /** Toggle state; omitted for plain buttons. */
    pressed?: boolean;
    children: Snippet;
  } & HTMLButtonAttributes = $props();
</script>

<button {type} class="ui-icon-btn {size} {variant}" aria-label={label} title={label} aria-pressed={pressed} {...rest}>
  {@render children()}
</button>

<style>
  .ui-icon-btn {
    --s: 32px;
    display: inline-grid;
    place-items: center;
    width: var(--s);
    height: var(--s);
    padding: 0;
    border: 1px solid transparent;
    border-radius: var(--r-sm);
    background: transparent;
    color: var(--text-2);
    cursor: pointer;
    transition:
      background var(--dur-fast) var(--ease-standard),
      color var(--dur-fast) var(--ease-standard);
  }
  .sm {
    --s: 26px;
  }
  .lg {
    --s: 40px;
  }
  .secondary {
    background: var(--secondary);
    border-color: var(--line-2);
  }
  .ui-icon-btn:is(:hover, [data-force="hover"]) {
    background: var(--surface-2);
    color: var(--text);
  }
  .secondary:is(:hover, [data-force="hover"]) {
    background: var(--secondary-hover);
  }
  .ui-icon-btn[aria-pressed="true"] {
    background: var(--surface-3);
    color: var(--text);
  }
  .ui-icon-btn:is(:focus-visible, [data-force="focus"]) {
    outline: 2px solid var(--focus);
    outline-offset: 2px;
  }
  .ui-icon-btn:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .ui-icon-btn :global(svg) {
    width: 18px;
    height: 18px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.7;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .sm :global(svg) {
    width: 15px;
    height: 15px;
  }
</style>
