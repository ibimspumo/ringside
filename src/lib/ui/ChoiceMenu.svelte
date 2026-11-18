<script lang="ts" generics="T extends string | null">
  import { floating, rectOf, type Anchor } from "./floating";

  /**
   * One choice out of a few for tight headers: the current option on a compact button, the list
   * in a card below it. Same choices as a SegmentedControl, in a fraction of the width.
   */
  let {
    options,
    value,
    label,
    onchange,
    dim = false,
    title,
  }: {
    options: [T, string][];
    /** Undefined while the choice does not apply: the button shows the label instead. */
    value: T | undefined;
    /** Names the choice, e.g. "Zeitraum"; read with the current option. */
    label: string;
    onchange: (value: T) => void;
    /** The choice does not apply right now (it stays usable). */
    dim?: boolean;
    title?: string;
  } = $props();

  let button = $state<HTMLButtonElement>();
  let anchor = $state<Anchor | null>(null);
  const current = $derived(options.find(([id]) => id === value)?.[1] ?? "–");

  function toggle() {
    anchor = anchor || !button ? null : rectOf(button);
  }
  function pick(id: T) {
    anchor = null;
    onchange(id);
    button?.focus();
  }
  function outside(e: PointerEvent) {
    if (anchor && !(e.target instanceof Element && e.target.closest(".ui-choice, .ui-choice-list"))) anchor = null;
  }
</script>

<svelte:window onpointerdown={outside} onkeydown={(e) => e.key === "Escape" && anchor && ((anchor = null), button?.focus())} />

<button
  bind:this={button}
  type="button"
  class="ui-choice"
  class:dim
  aria-haspopup="listbox"
  aria-expanded={!!anchor}
  aria-label="{label}: {current}"
  {title}
  onclick={toggle}
>
  <span class="what">{label}</span><span>{current}</span>
  <svg viewBox="0 0 12 12" width="10" height="10" aria-hidden="true"><path d="M3 4.5l3 3 3-3" /></svg>
</button>
{#if anchor}
  <ul class="ui-choice-list" role="listbox" aria-label={label} use:floating={{ anchor, side: "bottom" }}>
    {#each options as [id, text] (text)}
      <li>
        <button type="button" role="option" aria-selected={id === value} class:on={id === value} onclick={() => pick(id)}>
          <svg viewBox="0 0 12 12" width="11" height="11" aria-hidden="true"><path d="M2.5 6.3l2.3 2.2 4.7-5" /></svg>
          {text}
        </button>
      </li>
    {/each}
  </ul>
{/if}

<style>
  .ui-choice {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-2);
    height: 32px;
    padding: 0 var(--sp-3);
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    background: var(--bg);
    color: var(--text);
    font: inherit;
    font-size: var(--fs-md);
    white-space: nowrap;
    cursor: pointer;
    transition: border-color var(--dur-fast) var(--ease-standard);
  }
  .ui-choice:is(:hover, [aria-expanded="true"]) {
    border-color: var(--line-2);
  }
  .ui-choice.dim {
    opacity: 0.45;
  }
  svg {
    flex: none;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.6;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .ui-choice svg,
  .what {
    color: var(--text-2);
  }
  .ui-choice-list {
    min-width: 150px;
    margin: 0;
    padding: var(--sp-1);
    list-style: none;
    border: 1px solid var(--line-2);
    border-radius: var(--r-md);
    background: var(--surface-3);
    box-shadow: var(--shadow-2);
    animation: drop var(--dur-fast) var(--ease-enter);
  }
  .ui-choice-list button {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    width: 100%;
    height: 30px;
    padding: 0 var(--sp-3) 0 var(--sp-2);
    border: 0;
    border-radius: var(--r-sm);
    background: transparent;
    color: var(--text-2);
    font: inherit;
    font-size: var(--fs-md);
    text-align: left;
    cursor: pointer;
  }
  .ui-choice-list button:is(:hover, :focus-visible) {
    background: var(--surface-2);
    color: var(--text);
  }
  .ui-choice-list button svg {
    visibility: hidden;
  }
  .ui-choice-list button.on {
    color: var(--text);
    font-weight: var(--fw-semibold);
  }
  .ui-choice-list button.on svg {
    visibility: visible;
  }
  @keyframes drop {
    from {
      opacity: 0;
      transform: translateY(-3px);
    }
  }
</style>
