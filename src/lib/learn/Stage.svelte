<script lang="ts">
  import PlayingCard from "../PlayingCard.svelte";
  import type { ExercisePrompt } from "../types";
  import { explain, fmtBB, fmtPct, parseRange } from "./catalog";
  import SeatRing from "./SeatRing.svelte";
  import Matrix from "./wissen/blocks/Matrix.svelte";

  /** The picture of an exercise. Showdown hands and seats double as the answer input. */
  let {
    prompt,
    choice = null,
    truthChoice = null,
    onChoose,
    seatPickable = false,
    seat = null,
    truthSeat = null,
    onSeat,
  }: {
    prompt: ExercisePrompt;
    choice?: string | null;
    truthChoice?: string | null;
    onChoose?: (id: string) => void;
    seatPickable?: boolean;
    seat?: number | null;
    truthSeat?: number | null;
    onSeat?: (seat: number) => void;
  } = $props();

  let width = $state(700);
  const big = $derived(width < 460 ? 38 : 50);

  /** Hole cards in the order their name is read: "K5o" shows the king first. */
  const highFirst = (cards: string[]) => [...cards].sort((a, b) => "23456789TJQKA".indexOf(b[0]) - "23456789TJQKA".indexOf(a[0]));

  /** "65s" → what the s or o means; pairs need no hint. */
  function handHint(cls: string): string {
    return cls.endsWith("s") ? "s: gleiche Farbe" : cls.endsWith("o") ? "o: verschiedene Farben" : "Paar";
  }
  const mid = $derived(width < 460 ? 32 : 40);
</script>

<div class="stage" bind:clientWidth={width}>
  {#if prompt.type === "outs" || prompt.type === "equity_hand"}
    {@const villain = prompt.villain}
    <div class="felt">
      {#if villain}
        <div class="hand">
          <span class="who">Gegner</span>
          <span class="cards">{#each villain as c (c)}<PlayingCard card={c} size={mid} />{/each}</span>
        </div>
      {/if}
      <div class="board" aria-label="Board">
        {#each prompt.board as c (c)}<PlayingCard card={c} size={big} />{/each}
        {#each Array(Math.max(0, (prompt.type === "outs" ? prompt.board.length + 1 : 5) - prompt.board.length)) as _, i (i)}
          <span class="ghost" style="--w:{big}px" aria-hidden="true">?</span>
        {/each}
      </div>
      <div class="hand">
        <span class="who hero">Du</span>
        <span class="cards">{#each prompt.hero as c (c)}<PlayingCard card={c} size={big} />{/each}</span>
      </div>
    </div>
  {:else if prompt.type === "all_in"}
    <div class="felt">
      {#each prompt.opponents as opp, i (i)}
        <div class="hand">
          <span class="who">{prompt.opponents.length > 1 ? `Gegner ${i + 1}` : "Gegner"}</span>
          <span class="cards">{#each opp as c (c)}<PlayingCard card={c} size={mid} />{/each}</span>
        </div>
      {/each}
      <div class="board" aria-label="Board">
        {#each prompt.board as c (c)}<PlayingCard card={c} size={big} />{/each}
        {#if !prompt.board.length}<span class="dim small">Preflop all-in, noch kein Board</span>{/if}
      </div>
      <div class="hand">
        <span class="who hero">Du</span>
        <span class="cards">{#each prompt.hero as c (c)}<PlayingCard card={c} size={big} />{/each}</span>
      </div>
      <div class="pots">
        {#each prompt.pots as p, i (i)}
          <span class="pot"><span class="faint">{i === 0 ? "Hauptpot" : `Sidepot ${i}`}</span> <b class="num">{fmtBB(p.amount_bb)}</b></span>
        {/each}
      </div>
    </div>
  {:else if prompt.type === "pot_odds"}
    {@const total = prompt.pot + prompt.to_call}
    <div class="potodds">
      <div class="bar" role="img" aria-label="Pot {fmtBB(prompt.pot)}, Call {fmtBB(prompt.to_call)}">
        <span class="seg pot" style="flex:{prompt.pot}"><b class="num">{fmtBB(prompt.pot)}</b><small>im Pot</small></span>
        <span class="seg call" style="flex:{prompt.to_call}"><b class="num">{fmtBB(prompt.to_call)}</b><small>dein Call</small></span>
      </div>
      <div class="scale num"><span>0</span><span>Pot nach deinem Call: {fmtBB(total)}</span></div>
      {#if prompt.equity_pct !== null}
        <div class="given">
          <span class="faint">Deine Equity</span>
          <b class="num">{fmtPct(prompt.equity_pct, 1)}</b>
        </div>
      {/if}
    </div>
  {:else if prompt.type === "probability"}
    <div class="tiles">
      <div class="tile"><b class="num">{prompt.outs}</b><span>Outs</span></div>
      <div class="tile"><b class="num">{prompt.unseen}</b><span>unbekannte Karten</span></div>
      <div class="tile"><b class="num">{prompt.cards_to_come}</b><span>{prompt.cards_to_come === 1 ? "Karte kommt" : "Karten kommen"}</span></div>
      <div class="tile method">
        <b>{prompt.method === "rule" ? "Regel 2/4" : "Exakt"}</b>
        <span>{prompt.method === "rule" ? "Schätzen mit Outs × 2 oder × 4" : "Auf eine Nachkommastelle"}</span>
      </div>
    </div>
  {:else if prompt.type === "combos"}
    <div class="combos">
      {#if prompt.class}
        <span class="cls num">{prompt.class}</span>
      {:else}
        <span class="range num">{prompt.range}</span>
      {/if}
      {#if prompt.dead.length}
        <div class="hand">
          <span class="who">Bekannte Karten</span>
          <span class="cards">{#each prompt.dead as c (c)}<PlayingCard card={c} size={mid} />{/each}</span>
        </div>
      {/if}
    </div>
  {:else if prompt.type === "position"}
    <SeatRing
      size={prompt.table_size}
      occupied={prompt.occupied}
      button={prompt.button}
      smallBlind={prompt.small_blind}
      bigBlind={prompt.big_blind}
      target={prompt.ask.type === "label_of" ? prompt.ask.seat : null}
      pickable={seatPickable}
      picked={seat}
      truth={truthSeat}
      onPick={onSeat}
    />
    {#if prompt.dead_button}<p class="note dim">Toter Button: Der Button liegt auf einem leeren Sitz.</p>{/if}
  {:else if prompt.type === "showdown"}
    <div class="felt">
      <div class="board" aria-label="Board">
        {#each prompt.board as c (c)}<PlayingCard card={c} size={big} />{/each}
      </div>
      <div class="players" role="group" aria-label="Wer gewinnt?">
        {#each prompt.players as hand, i (i)}
          {@const id = `p${i}`}
          <button
            class="player"
            class:on={choice === id}
            class:win={truthChoice === id}
            class:miss={truthChoice !== null && choice === id && truthChoice !== id}
            disabled={!onChoose}
            onclick={() => onChoose?.(id)}
          >
            <span class="who">Spieler {i + 1} <kbd>{i + 1}</kbd></span>
            <span class="cards">{#each hand as c (c)}<PlayingCard card={c} size={mid} />{/each}</span>
          </button>
        {/each}
      </div>
    </div>
  {:else if prompt.type === "equity_range"}
    <div class="vs">
      <div class="hand">
        <span class="who hero">Du</span>
        <span class="cards">{#each prompt.hero as c (c)}<PlayingCard card={c} size={big} />{/each}</span>
        {#if prompt.board.length}
          <span class="who">Flop</span>
          <span class="cards">{#each prompt.board as c (c)}<PlayingCard card={c} size={mid} />{/each}</span>
        {/if}
      </div>
      <div class="range-box">
        <span class="who">Range „{prompt.range_name}“ · {fmtPct(prompt.range_pct, 1)} aller Hände</span>
        <Matrix range={parseRange(prompt.range)} />
      </div>
    </div>
  {:else if prompt.type === "push_fold"}
    {@const sb = prompt.hero_position === "sb"}
    <div class="felt hu">
      <div class="hu-seat">
        <span class="hu-pos">{sb ? "Big Blind" : "Small Blind"}</span>
        <span class="num hu-stack">{fmtBB(prompt.stack_bb)}</span>
        <span class="hu-act" class:allin={!sb}>{sb ? "wartet auf dich" : "geht all-in"}</span>
      </div>
      <div class="hu-mid">
        <b>{sb ? "Alle anderen haben gefoldet" : "Der Small Blind schiebt all-in"}</b>
        <span class="dim small">
          Blinds {fmtBB(prompt.small_blind_bb)} / 1 bb{#if prompt.ante_bb > 0}{` · Ante ${fmtBB(prompt.ante_bb)}`}{/if}{#if prompt.dead_bb > 0}{` · `}<span
              class="term"
              title={explain("Totes Geld")}>Totes Geld</span
            >{` ${fmtBB(prompt.dead_bb)}`}{/if} · {prompt.players === 2 ? "Heads-up: nur ihr beide" : `${prompt.players} Spieler am Tisch`}
        </span>
      </div>
      <div class="hu-seat hero">
        <span class="cards">{#each highFirst(prompt.hand) as c (c)}<PlayingCard card={c} size={big + 6} />{/each}</span>
        <span class="hu-pos">Du · {sb ? "Small Blind" : "Big Blind"} · <span class="num">{fmtBB(prompt.stack_bb)}</span></span>
        <span class="cls-line"><b class="num cls-sm">{prompt.class}</b> <span class="dim">{handHint(prompt.class)}</span></span>
      </div>
    </div>
  {:else if prompt.type === "push_fold_table"}
    {@const p = prompt}
    {@const pos = p.positions[p.hero]}
    <div class="pf">
      <div class="hand">
        <span class="cards">{#each highFirst(p.hand) as c (c)}<PlayingCard card={c} size={big + 6} />{/each}</span>
        <span class="cls num">{p.class}</span>
      </div>
      <dl class="spot">
        <div><dt>Du sitzt</dt><dd>{pos} <span class="faint">von {p.positions.length}</span></dd></div>
        <div>
          <dt>Situation</dt>
          <dd>
            {#if p.shover === null}Alle vor dir folden{:else}{p.positions[p.shover]} all-in{#each p.callers as c (c)}, {p.positions[c]} callt{/each}{/if}
          </dd>
        </div>
        <div><dt>Effektiver Stack</dt><dd class="num">{fmtBB(p.effective_bb)}</dd></div>
        <div><dt>Blinds</dt><dd class="num">{fmtBB(p.small_blind_bb)} / 1 bb</dd></div>
        {#if p.ante_bb > 0}<div><dt>Ante</dt><dd class="num">{fmtBB(p.ante_bb)}</dd></div>{/if}
        {#if p.bb_ante_bb > 0}<div><dt>Big-Blind-Ante</dt><dd class="num">{fmtBB(p.bb_ante_bb)}</dd></div>{/if}
        {#if p.dead_bb > 0}<div><dt>Totes Geld</dt><dd class="num">{fmtBB(p.dead_bb)}</dd></div>{/if}
      </dl>
    </div>
    <ol class="seats" aria-label="Tisch in Reihenfolge der Aktion">
      {#each p.positions as name, i (name)}
        {@const act =
          i === p.hero ? "Du" : i === p.shover ? "All-in" : p.callers.includes(i) ? "Call" : i < p.hero ? "Fold" : "danach"}
        <li class:hero={i === p.hero} class:in={i === p.shover || p.callers.includes(i)} class:out={act === "Fold"}>
          <span class="seat-pos">{name}</span>
          <span class="num">{fmtBB(p.stacks_bb[i])}</span>
          <span class="seat-act">{act}</span>
        </li>
      {/each}
    </ol>
  {/if}
</div>

<style>
  .stage {
    min-width: 0;
    container-type: inline-size;
  }
  .felt {
    display: grid;
    justify-items: center;
    gap: 14px;
    padding: 18px 14px;
    border-radius: var(--r);
    background: radial-gradient(ellipse at 50% 40%, #23312d 0%, #1b2623 65%, #16201d 100%);
    box-shadow:
      inset 0 0 0 1px rgba(0, 0, 0, 0.4),
      inset 0 0 30px rgba(0, 0, 0, 0.35);
  }
  .hand {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: center;
    gap: 10px;
  }
  .cards {
    display: inline-flex;
    gap: 5px;
  }
  .who {
    font-size: 12.5px;
    color: var(--text-2);
  }
  .who.hero {
    color: var(--text);
    font-weight: 600;
  }
  .board {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 6px;
    min-height: 40px;
    align-items: center;
  }
  .ghost {
    display: grid;
    place-items: center;
    width: var(--w);
    height: calc(var(--w) * 1.4);
    border-radius: max(3px, calc(var(--w) * 0.12));
    border: 1.5px dashed rgba(236, 235, 231, 0.22);
    color: rgba(236, 235, 231, 0.35);
    font-family: var(--font-num);
    font-size: 18px;
  }
  .small {
    font-size: 12.5px;
  }
  .pots {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 6px 16px;
    font-size: 13px;
  }

  .potodds {
    display: grid;
    gap: 8px;
  }
  .bar {
    display: flex;
    height: 64px;
    border-radius: var(--r);
    overflow: hidden;
    gap: 2px;
  }
  .seg {
    display: flex;
    flex-direction: column;
    justify-content: center;
    min-width: 76px;
    padding: 0 12px;
    animation: grow 0.5s cubic-bezier(0.2, 0.8, 0.2, 1) both;
    transform-origin: left;
  }
  .seg.pot {
    background: var(--surface-3);
  }
  .seg.call {
    background: var(--text);
    color: var(--bg);
    animation-delay: 0.12s;
  }
  .seg b {
    font-size: 18px;
    font-weight: 600;
  }
  .seg small {
    font-size: 12px;
    opacity: 0.75;
  }
  @keyframes grow {
    from {
      transform: scaleX(0.3);
      opacity: 0;
    }
  }
  .scale {
    display: flex;
    justify-content: space-between;
    font-size: 12.5px;
    color: var(--text-3);
  }
  .given {
    display: flex;
    align-items: baseline;
    gap: 10px;
    margin-top: 4px;
  }
  .given b {
    font-size: 22px;
  }

  .tiles {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(130px, 1fr));
    gap: 10px;
  }
  .tile {
    display: grid;
    gap: 2px;
    padding: 14px;
    border-radius: var(--r);
    background: var(--surface-2);
    border: 1px solid var(--line);
  }
  .tile b {
    font-family: var(--font-num);
    font-size: 30px;
    font-weight: 600;
    line-height: 1.1;
  }
  .tile span {
    color: var(--text-2);
    font-size: 12.5px;
  }
  .tile.method b {
    font-size: 18px;
  }

  .combos {
    display: grid;
    justify-items: center;
    gap: 14px;
    padding: 12px 0;
  }
  .cls {
    font-size: 44px;
    font-weight: 600;
    letter-spacing: 0.02em;
    line-height: 1;
  }
  .range {
    font-size: 24px;
    font-weight: 600;
    text-align: center;
  }

  .players {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 10px;
  }
  .player {
    display: grid;
    justify-items: center;
    gap: 6px;
    padding: 10px 12px;
    border-radius: var(--r);
    border: 1.5px solid var(--line-2);
    background: rgba(0, 0, 0, 0.25);
    cursor: pointer;
    transition:
      border-color 0.12s,
      transform 0.12s;
  }
  .player:disabled {
    cursor: default;
  }
  .player:hover:not(:disabled) {
    border-color: var(--text-2);
    transform: translateY(-2px);
  }
  .player.on {
    border-color: var(--text);
    background: rgba(236, 235, 231, 0.08);
  }
  .player.win {
    border-color: var(--pos);
    box-shadow: 0 0 0 3px rgba(120, 193, 66, 0.25);
  }
  .player.miss {
    border-color: var(--neg);
  }
  kbd {
    margin-left: 4px;
    padding: 0 5px;
    border-radius: 4px;
    border: 1px solid var(--line-2);
    font-family: var(--font-num);
    font-size: 11px;
    color: var(--text-3);
  }

  .vs {
    display: grid;
    grid-template-columns: minmax(0, 200px) minmax(0, 1fr);
    gap: 20px;
    align-items: start;
  }
  .vs .hand {
    flex-direction: column;
    align-items: flex-start;
  }
  .range-box {
    display: grid;
    gap: 8px;
    min-width: 0;
  }

  .hu {
    gap: 10px;
    padding: 14px;
  }
  @container (min-width: 640px) {
    .hu {
      grid-template-columns: auto auto auto;
      justify-content: center;
      align-items: center;
      column-gap: 40px;
      padding: 16px 24px;
    }
    .hu .hu-mid {
      order: 0;
      max-width: 260px;
    }
  }
  .hu-seat {
    display: grid;
    justify-items: center;
    gap: 4px;
    padding: 8px 18px;
    border-radius: var(--r);
    background: rgba(18, 20, 24, 0.55);
    border: 1px solid var(--line-2);
  }
  .hu-seat.hero {
    gap: 6px;
    padding: 12px 22px 10px;
    border-color: rgba(236, 235, 231, 0.3);
  }
  .hu-pos {
    font-size: 13px;
    font-weight: 600;
    white-space: nowrap;
  }
  .hu-stack {
    font-size: 15px;
  }
  .hu-act {
    font-size: 12px;
    color: var(--text-2);
  }
  .hu-act.allin {
    color: var(--neg);
    font-weight: 600;
  }
  .hu-mid {
    display: grid;
    justify-items: center;
    gap: 2px;
    text-align: center;
  }
  .hu-mid b {
    font-size: 14px;
  }
  .cls-line {
    font-size: 12.5px;
  }
  .cls-sm {
    font-size: 18px;
    font-weight: 600;
    margin-right: 4px;
  }
  .term {
    text-decoration: underline dotted;
    text-decoration-color: var(--text-3);
    text-underline-offset: 3px;
    cursor: help;
  }
  .pf {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    gap: 24px;
    align-items: center;
  }
  .pf .hand {
    flex-direction: column;
  }
  .spot {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
    gap: 8px;
    margin: 0;
  }
  .spot div {
    padding: 8px 10px;
    border-radius: var(--r-sm);
    background: var(--surface-2);
  }
  dt {
    font-size: 12px;
    color: var(--text-2);
  }
  dd {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
  }
  .seats {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(68px, 1fr));
    gap: 6px;
    margin: 14px 0 0;
    padding: 0;
    list-style: none;
  }
  .seats li {
    display: grid;
    justify-items: center;
    gap: 1px;
    padding: 7px 4px;
    border-radius: var(--r-sm);
    border: 1px solid var(--line);
    background: var(--surface-2);
    font-size: 13px;
  }
  .seats .seat-pos {
    font-weight: 600;
  }
  .seats .seat-act {
    font-size: 11.5px;
    color: var(--text-3);
  }
  .seats li.out {
    opacity: 0.5;
  }
  .seats li.in {
    border-color: var(--neg);
  }
  .seats li.in .seat-act {
    color: var(--neg);
    font-weight: 600;
  }
  .seats li.hero {
    border-color: var(--text);
    background: rgba(236, 235, 231, 0.08);
  }
  .seats li.hero .seat-act {
    color: var(--text);
    font-weight: 600;
  }
  .note {
    margin: 8px 0 0;
    text-align: center;
    font-size: 12.5px;
  }
  @container (max-width: 560px) {
    .vs,
    .pf {
      grid-template-columns: 1fr;
    }
  }
</style>
