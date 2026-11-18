<script lang="ts" module>
  import type { ActionKind } from "./poker";

  export type ChainMove = {
    /** "CO". */
    pos: string;
    /** The player's name, shown on hover. */
    name?: string;
    kind: ActionKind;
    /** In big blinds, what the player has in on this street after the move. */
    amount?: number;
    hero?: boolean;
  };
  export type ChainStreet = { street: "preflop" | "flop" | "turn" | "river"; cards?: string[]; moves: ChainMove[] };
</script>

<script lang="ts">
  import ActionLabel from "./ActionLabel.svelte";
  import MiniCard from "./MiniCard.svelte";

  /**
   * A hand so far as one line: every move with seat, word and amount, street by street with its
   * cards. `turn` ends the line with the seat that is to act, e.g. your own decision.
   */
  let { streets, turn, label = "Bisher in der Hand" }: { streets: ChainStreet[]; turn?: { pos: string; text: string }; label?: string } = $props();

  const NAME = { preflop: "Preflop", flop: "Flop", turn: "Turn", river: "River" } as const;
</script>

<ol class="ui-chain" aria-label={label}>
  {#each streets as s, i (s.street)}
    <li class="street" class:first={i === 0}>
      <span class="name">{NAME[s.street]}</span>
      {#if s.cards?.length}<span class="cards">{#each s.cards as c (c)}<MiniCard card={c} />{/each}</span>{/if}
      {#each s.moves as m, j (j)}
        <span class="move" class:fold={m.kind === "fold"} class:hero={m.hero} title={m.name ? `${m.hero ? "Du" : m.name} (${m.pos})` : undefined}>
          <b class="seat">{m.hero ? `Du · ${m.pos}` : m.pos}</b>
          <ActionLabel kind={m.kind} amount={m.amount} variant="text" size="sm" />
        </span>
      {/each}
      {#if turn && i === streets.length - 1}
        <span class="move turn"><b class="seat">{turn.pos}</b><span class="turn-text">{turn.text}</span></span>
      {/if}
    </li>
  {/each}
</ol>

<style>
  .ui-chain {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-2) var(--sp-3);
    align-items: center;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .street {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-1);
    min-width: 0;
    padding-left: var(--sp-3);
    border-left: 1px solid var(--line-2);
  }
  .street.first {
    padding-left: 0;
    border-left: 0;
  }
  .name {
    margin-right: var(--sp-1);
    color: var(--text-3);
    font-size: var(--fs-xs);
    font-weight: var(--fw-semibold);
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }
  .cards {
    display: inline-flex;
    gap: 2px;
    margin-right: var(--sp-1);
    font-size: var(--fs-base);
  }
  .move {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 28px;
    padding: 0 var(--sp-2);
    border: 1px solid var(--line);
    border-radius: var(--r-sm);
    background: var(--surface-2);
    white-space: nowrap;
  }
  .seat {
    color: var(--text-2);
    font-size: var(--fs-sm);
    font-weight: var(--fw-semibold);
  }
  .move.fold {
    background: transparent;
  }
  .move.fold :global(.ui-action) {
    color: var(--text-3);
  }
  .move.hero {
    border-color: var(--line-2);
    background: var(--surface-3);
  }
  .move.hero .seat {
    color: var(--text);
  }
  .move.turn {
    border-color: var(--focus);
    background: color-mix(in srgb, var(--focus) 12%, var(--surface-2));
    box-shadow: 0 0 0 1px var(--focus);
  }
  .move.turn .seat {
    color: var(--text);
  }
  .turn-text {
    color: var(--text);
    font-size: var(--fs-sm);
    font-weight: var(--fw-medium);
  }
</style>
