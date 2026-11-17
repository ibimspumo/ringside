<script lang="ts">
  import PlayingCard from "../PlayingCard.svelte";
  import { GRADE, choiceShort } from "./labels";
  import type { HandView, Street } from "./types";

  /**
   * The hand as one line: per street the moves, folds of others grouped, the board between.
   * The hero's moves carry their grade as a coloured notch; a click shows that moment.
   */
  let { view, at, onLook }: { view: HandView; at: number; onLook: (snapshot: number | null) => void } = $props();

  const STREET: Record<Street, string> = { preflop: "Preflop", flop: "Flop", turn: "Turn", river: "River" };

  type Item =
    | { type: "street"; street: Street; cards: string[]; snapshot: number }
    | { type: "move"; seats: number[]; text: string; hero: boolean; grade: string | null; label: string | null; snapshot: number; fold: boolean };

  const items = $derived.by(() => {
    const out: Item[] = [{ type: "street", street: "preflop", cards: [], snapshot: 0 }];
    for (const e of view.events) {
      if (e.type === "board") {
        out.push({ type: "street", street: e.street, cards: e.cards, snapshot: e.snapshot });
        continue;
      }
      const hero = e.seat === view.hero;
      const fold = e.choice.kind === "fold" && !hero;
      const prev = out[out.length - 1];
      if (fold && prev.type === "move" && prev.fold) {
        prev.seats.push(e.seat);
        prev.snapshot = e.snapshot;
        continue;
      }
      const v = e.verdict !== null ? view.verdicts[e.verdict] : null;
      const g = v?.judgement?.grade ?? null;
      out.push({
        type: "move",
        seats: [e.seat],
        text: choiceShort(e.choice),
        hero,
        grade: g ? GRADE[g].color : null,
        label: g ? GRADE[g].label : v ? "nicht bewertet" : null,
        snapshot: e.snapshot,
        fold,
      });
    }
    return out;
  });

  const live = $derived(view.snapshots.length - 1);
  let strip = $state<HTMLElement>();
  $effect(() => {
    void items.length;
    strip?.scrollTo({ left: strip.scrollWidth, behavior: "smooth" });
  });
</script>

<nav class="timeline" aria-label="Verlauf der Hand" bind:this={strip}>
  <ol>
    {#each items as it, i (i)}
      {#if it.type === "street"}
        <li class="street">
          <button class:on={at === it.snapshot} onclick={() => onLook(it.snapshot === live ? null : it.snapshot)}>
            <span class="name">{STREET[it.street]}</span>
            {#each it.cards as c (c)}<PlayingCard card={c} size={18} />{/each}
          </button>
        </li>
      {:else}
        <li class="move" class:hero={it.hero} class:fold={it.fold}>
          <button
            class:on={at === it.snapshot}
            style:--g={it.grade ?? "transparent"}
            onclick={() => onLook(it.snapshot === live ? null : it.snapshot)}
            title={it.label ? `Dein Zug: ${it.label}` : undefined}
            aria-label="{it.seats.map((s) => view.positions[s]).join(', ')} {it.text}{it.label ? `, ${it.label}` : ''}"
          >
            <b>{it.seats.map((s) => view.positions[s]).join(" ")}</b>
            <span>{it.text}</span>
            {#if it.hero && it.grade}<i class="notch" aria-hidden="true"></i>{/if}
          </button>
        </li>
      {/if}
    {/each}
  </ol>
</nav>

<style>
  .timeline {
    overflow-x: auto;
    scrollbar-width: thin;
    padding: 2px 0 6px;
  }
  ol {
    display: flex;
    align-items: stretch;
    gap: 4px;
    margin: 0;
    padding: 0;
    list-style: none;
    min-width: max-content;
  }
  li {
    display: flex;
  }
  button {
    position: relative;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 34px;
    padding: 0 10px;
    border: 1px solid var(--line);
    border-radius: var(--r-sm);
    background: var(--surface);
    color: var(--text-2);
    font-size: 12.5px;
    cursor: pointer;
    white-space: nowrap;
  }
  button:hover {
    border-color: var(--line-2);
    color: var(--text);
  }
  button.on {
    border-color: var(--focus);
    color: var(--text);
  }
  .street {
    margin-left: 8px;
  }
  .street:first-child {
    margin-left: 0;
  }
  .street button {
    background: transparent;
    border-color: transparent;
    gap: 3px;
  }
  .street .name {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-3);
    margin-right: 4px;
  }
  .move b {
    font-weight: 600;
    color: var(--text);
  }
  .fold button {
    color: var(--text-3);
  }
  .fold b {
    color: var(--text-3);
    font-weight: 500;
  }
  .hero button {
    background: var(--surface-2);
    color: var(--text);
    padding-bottom: 3px;
  }
  .notch {
    position: absolute;
    left: 8px;
    right: 8px;
    bottom: 3px;
    height: 3px;
    border-radius: 2px;
    background: var(--g);
  }
</style>
