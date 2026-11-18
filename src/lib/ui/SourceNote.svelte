<script lang="ts">
  import type { Snippet } from "svelte";
  import { SOURCE, type Source } from "./poker";

  /**
   * Says where a number comes from ("Modell", "Solver", …) or that a feature is locked while
   * PokerStars runs. `inline` is the small mark next to a figure (explanation on hover), `block`
   * the line under a result with the explanation written out. The caller may add specifics.
   */
  let {
    kind,
    variant = "inline",
    children,
  }: { kind: Source; variant?: "inline" | "block"; children?: Snippet } = $props();

  const s = $derived(SOURCE[kind]);
</script>

{#if variant === "inline"}
  <span class="ui-source inline" class:locked={kind === "locked"} class:fact={kind === "computed"} title={s.text}>
    {@render mark()}{s.label}
  </span>
{:else}
  <div class="ui-source block" class:locked={kind === "locked"} class:fact={kind === "computed"} role="note">
    {@render mark()}
    <p><b>{s.label}.</b> {#if children}{@render children()}{:else}{s.text}{/if}</p>
  </div>
{/if}

{#snippet mark()}
  <svg viewBox="0 0 16 16" width="13" height="13" aria-hidden="true">
    {#if kind === "locked"}
      <rect x="3" y="7" width="10" height="7" rx="1.6" /><path d="M5.5 7V5a2.5 2.5 0 0 1 5 0v2" />
    {:else if kind === "computed"}
      <path d="M3.5 8.5l3 3 6-7" />
    {:else}
      <circle cx="8" cy="8" r="6" /><path d="M8 7.2v4" /><circle cx="8" cy="4.9" r=".4" class="pt" />
    {/if}
  </svg>
{/snippet}

<style>
  .ui-source {
    --c: var(--source);
    color: color-mix(in srgb, var(--c) 75%, var(--text));
  }
  .locked {
    --c: var(--amber);
  }
  /* A computed figure is a fact: solid frame and text colour instead of the dashed "where from". */
  .fact {
    --c: var(--text-2);
  }
  .inline.fact {
    border-style: solid;
    border-color: var(--line-2);
    background: var(--surface-2);
    color: var(--text);
  }
  svg {
    flex-shrink: 0;
    fill: none;
    stroke: var(--c);
    stroke-width: 1.5;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .pt {
    fill: var(--c);
  }
  .inline {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    height: 20px;
    padding: 0 7px 0 5px;
    border-radius: var(--r-pill);
    border: 1px dashed color-mix(in srgb, var(--c) 55%, transparent);
    font-size: var(--fs-xs);
    font-weight: var(--fw-medium);
    line-height: 1;
    white-space: nowrap;
    vertical-align: middle;
    cursor: help;
  }
  .block {
    display: flex;
    align-items: flex-start;
    gap: var(--sp-2);
    padding: var(--sp-2) var(--sp-3);
    border-radius: var(--r-sm);
    border: 1px dashed color-mix(in srgb, var(--c) 40%, transparent);
    background: color-mix(in srgb, var(--c) 6%, transparent);
    font-size: var(--fs-sm);
    line-height: 1.45;
  }
  .block svg {
    margin-top: 2px;
  }
  .block p {
    margin: 0;
    color: var(--text-2);
  }
  .block b {
    color: color-mix(in srgb, var(--c) 75%, var(--text));
    font-weight: var(--fw-semibold);
  }
</style>
