<script lang="ts">
  import MiniCard from "../ui/MiniCard.svelte";
  import { GRADE, choiceShort } from "./labels";
  import type { HandView, Street } from "./types";

  /**
   * The hand as text, street by street: every action of every player in order, folds
   * included, your own moves marked with their grade. Shows the steps up to `upto`; `at`
   * is the step on the table, and a click on an action goes there.
   */
  let { view, at, upto, onLook }: { view: HandView; at: number; upto: number; onLook: (snapshot: number) => void } = $props();

  // On a narrow table the history is one line that you can slide; it keeps the newest step in sight.

  const STREET: Record<Street, string> = { preflop: "Preflop", flop: "Flop", turn: "Turn", river: "River" };

  type Move = { seat: number; text: string; snapshot: number; hero: boolean; fold: boolean; grade: string | null; mark: string; label: string | null };
  type Group = { street: Street; cards: string[]; snapshot: number; moves: Move[] };

  const groups = $derived.by(() => {
    const out: Group[] = [{ street: "preflop", cards: [], snapshot: 0, moves: [] }];
    for (const e of view.events) {
      if (e.snapshot > upto) break;
      if (e.type === "board") {
        out.push({ street: e.street, cards: e.cards, snapshot: e.snapshot, moves: [] });
        continue;
      }
      const v = e.verdict !== null ? view.verdicts[e.verdict] : null;
      const g = v?.judgement?.grade ?? null;
      const amount = e.choice.kind === "bet" || e.choice.kind === "raise" || e.choice.kind === "all_in" ? " BB" : "";
      out[out.length - 1].moves.push({
        seat: e.seat,
        text: `${choiceShort(e.choice)}${amount}`,
        snapshot: e.snapshot,
        hero: e.seat === view.hero,
        fold: e.choice.kind === "fold",
        grade: g ? GRADE[g].color : null,
        mark: g ? GRADE[g].mark : "",
        label: g ? GRADE[g].label : v ? "nicht bewertet" : null,
      });
    }
    return out;
  });
</script>

<nav class="timeline" aria-label="Verlauf der Hand">
  {#each groups as g (g.snapshot)}
    <section class="street" class:now={g.snapshot <= at && !groups.some((x) => x.snapshot > g.snapshot && x.snapshot <= at)}>
      <button class="head" class:on={at === g.snapshot} onclick={() => onLook(g.snapshot)} aria-label="{STREET[g.street]} ansehen">
        <span class="name">{STREET[g.street]}</span>
        {#each g.cards as c (c)}<MiniCard card={c} />{/each}
      </button>
      <ol>
        {#each g.moves as m (m.snapshot)}
          <li>
            <button
              class="move"
              data-pos={view.positions[m.seat]}
              class:hero={m.hero}
              class:fold={m.fold}
              class:on={at === m.snapshot}
              style:--g={m.grade ?? "var(--line-2)"}
              onclick={() => onLook(m.snapshot)}
              title={m.label ? `Dein Zug: ${m.label}` : undefined}
              aria-label="{m.hero ? 'Du' : view.positions[m.seat]}: {m.text}{m.label ? `, ${m.label}` : ''}"
            >
              <b>{m.hero ? `Du (${view.positions[m.seat]})` : view.positions[m.seat]}</b>
              <span>{m.text}</span>
              {#if m.hero && m.mark}<i class="mark" aria-hidden="true">{m.mark}</i>{/if}
            </button>
          </li>
        {/each}
      </ol>
    </section>
  {/each}
</nav>

<style>
  .timeline {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 14px;
    align-items: flex-start;
  }
  /* On a narrow table the chips get a little smaller, so the history stays in few lines. */
  @container history (width < 700px) {
    .timeline {
      --chip-h: 26px;
      --chip-px: 7px;
      --chip-fs: var(--fs-xs);
      gap: 4px 10px;
    }
  }
  .street {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px;
    min-width: 0;
    padding-left: 12px;
    border-left: 2px solid var(--line-2);
  }
  .street:first-child {
    padding-left: 0;
    border-left: 0;
  }
  .street.now {
    border-left-color: var(--focus);
  }
  .street:first-child.now .head .name {
    color: var(--text);
  }
  ol {
    display: contents;
    list-style: none;
  }
  li {
    display: flex;
  }
  button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: var(--chip-h, 30px);
    padding: 0 var(--chip-px, 9px);
    border: 1px solid var(--line);
    border-radius: var(--r-sm);
    background: var(--surface);
    color: var(--text-2);
    font-size: var(--chip-fs, var(--fs-sm));
    cursor: pointer;
    white-space: nowrap;
  }
  button:hover {
    border-color: var(--line-2);
    color: var(--text);
  }
  button:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: 1px;
  }
  button.on {
    border-color: var(--focus);
    color: var(--text);
    box-shadow: 0 0 0 1px var(--focus);
  }
  .head {
    background: transparent;
    border-color: transparent;
    gap: 3px;
    padding: 0 6px 0 0;
  }
  .head .name {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-3);
    margin-right: 4px;
  }
  .move b {
    font-weight: 600;
    color: var(--text);
  }
  .move.fold {
    color: var(--text-3);
  }
  .move.fold b {
    color: var(--text-2);
    font-weight: 500;
  }
  .move.hero {
    background: var(--surface-3);
    border-color: var(--g);
    color: var(--text);
  }
  .mark {
    display: inline-grid;
    place-items: center;
    min-width: 18px;
    height: 18px;
    padding: 0 3px;
    border-radius: 999px;
    background: var(--g);
    color: var(--bg);
    font-style: normal;
    font-size: 11px;
    font-weight: 700;
  }
</style>
