<script lang="ts">
  import type { Snippet } from "svelte";
  import Felt from "../Felt.svelte";
  import { REFERENCE_ASPECT } from "../hud/geometry";
  import PlayingCard from "../PlayingCard.svelte";
  import { money, moneyShort } from "../stats";
  import type { Action, Hand, Street } from "../types";

  /** `children` is the hand's title line, laid out flush with the table. */
  let { hand, children }: { hand: Hand; children?: Snippet } = $props();

  type Event =
    | { type: "action"; action: Action }
    | { type: "street"; street: Street }
    | { type: "showdown" }
    | { type: "collect" };

  const STREETS: Street[] = ["preflop", "flop", "turn", "river"];
  const BOARD_AT: Record<Street, number> = { preflop: 0, flop: 3, turn: 4, river: 5 };

  const events = $derived.by(() => {
    const out: Event[] = [];
    let street: Street = "preflop";
    for (const action of hand.actions) {
      while (action.street !== street) {
        street = STREETS[STREETS.indexOf(street) + 1];
        out.push({ type: "street", street });
      }
      out.push({ type: "action", action });
    }
    while (STREETS.indexOf(street) < 3 && hand.board.length > BOARD_AT[street]) {
      street = STREETS[STREETS.indexOf(street) + 1];
      out.push({ type: "street", street });
    }
    if (hand.shown.length) out.push({ type: "showdown" });
    out.push({ type: "collect" });
    return out;
  });

  let step = $state(0);
  let playing = $state(false);
  let inBB = $state(false);
  let stageWidth = $state(860);
  let logEl = $state<HTMLElement>();
  /** The stage keeps its shape, so seats, board and bets simply scale with its width. */
  const k = $derived(Math.min(1.15, Math.max(0.8, stageWidth / 880)));

  $effect(() => {
    void hand.id;
    step = 0;
    playing = false;
  });

  $effect(() => {
    if (!playing) return;
    const t = setInterval(() => {
      if (step >= events.length) playing = false;
      else step++;
    }, 850);
    return () => clearInterval(t);
  });

  // Keep the current log row in view without moving the page around it.
  $effect(() => {
    void step;
    const row = logEl?.querySelector<HTMLElement>("li.now");
    if (!logEl || !row) return;
    const box = logEl.getBoundingClientRect();
    const r = row.getBoundingClientRect();
    if (r.top < box.top + 30) logEl.scrollTop -= box.top + 30 - r.top;
    else if (r.bottom > box.bottom) logEl.scrollTop += r.bottom - box.bottom + 4;
  });

  const LABEL: Record<string, string> = {
    post_small_blind: "SB",
    post_big_blind: "BB",
    post_ante: "Ante",
    post_both: "SB+BB",
    fold: "Fold",
    check: "Check",
    call: "Call",
    bet: "Bet",
    raise: "Raise",
  };

  /**
   * Position names as the stats engine assigns them: the big blind is whoever posted it, the small
   * blind only counts when posted from the seat right before, and the button sits before that.
   */
  const positions = $derived.by(() => {
    const active = hand.seats.filter((s) => !s.sitting_out);
    const n = active.length;
    const out = new Map<string, string>();
    if (n < 2) return out;
    const posted = (kind: Action["kind"]) => hand.actions.find((a) => a.street === "preflop" && a.kind === kind)?.player;
    let big = active.findIndex((s) => s.name === posted("post_big_blind"));
    if (big < 0) {
      const after = active.findIndex((s) => s.seat >= hand.button_seat);
      big = ((after < 0 ? 0 : after) + 2) % n;
    }
    const order = Array.from({ length: n }, (_, i) => active[(big + 1 + i) % n].name);
    if (n === 2) {
      out.set(order[0], "BTN");
      out.set(order[1], "BB");
      return out;
    }
    const tail = ["BTN"];
    if (posted("post_small_blind") === order[n - 2]) tail.push("SB");
    tail.push("BB");
    const LATE = ["LJ", "HJ", "CO"];
    const EARLY = ["UTG", "UTG+1", "UTG+2", "UTG+3", "UTG+4"];
    const rest = n - tail.length;
    const early = Math.max(0, rest - LATE.length);
    const labels = [...EARLY.slice(0, early), ...LATE.slice(LATE.length - (rest - early)), ...tail];
    order.forEach((name, i) => out.set(name, labels[i]));
    return out;
  });

  const table = $derived.by(() => {
    const stacks = new Map(hand.seats.map((s) => [s.name, s.stack]));
    const put = new Map<string, number>();
    const label = new Map<string, string>();
    const folded = new Set<string>();
    let pot = 0;
    let board = 0;
    let revealed = false;
    let winners = new Map<string, number>();
    let street: Street = "preflop";
    let last: string | null = null;

    const sweep = () => {
      for (const v of put.values()) pot += v;
      put.clear();
    };

    for (const e of events.slice(0, step)) {
      last = null;
      if (e.type === "action") {
        const a = e.action;
        stacks.set(a.player, (stacks.get(a.player) ?? 0) - a.amount);
        if (a.kind === "post_ante") pot += a.amount;
        else put.set(a.player, (put.get(a.player) ?? 0) + a.amount);
        if (a.kind === "fold") folded.add(a.player);
        label.set(a.player, a.all_in ? "All-in" : LABEL[a.kind]);
        last = a.player;
      } else if (e.type === "street") {
        sweep();
        street = e.street;
        board = BOARD_AT[e.street];
        for (const name of label.keys()) if (!folded.has(name)) label.delete(name);
      } else if (e.type === "showdown") {
        sweep();
        revealed = true;
      } else {
        sweep();
        for (const [name, amount] of hand.returned) {
          stacks.set(name, (stacks.get(name) ?? 0) + amount);
          pot -= amount;
        }
        winners = new Map(hand.collected);
        for (const [name, amount] of hand.collected) stacks.set(name, (stacks.get(name) ?? 0) + amount);
        pot = 0;
      }
    }
    let inFront = 0;
    for (const v of put.values()) inFront += v;
    return { stacks, put, label, folded, pot, total: pot + inFront, board, revealed, winners, street, last };
  });

  const heroSeat = $derived(hand.seats.find((s) => s.name === hand.hero)?.seat ?? 1);
  const done = $derived(step >= events.length);

  /** Actions grouped by street, each with its index in `events` so the log can follow the replay. */
  const log = $derived.by(() => {
    const out: { street: Street; items: { i: number; a: Action }[] }[] = [];
    events.forEach((e, i) => {
      if (e.type !== "action") return;
      let group = out[out.length - 1];
      if (!group || group.street !== e.action.street) out.push((group = { street: e.action.street, items: [] }));
      group.items.push({ i, a: e.action });
    });
    return out;
  });

  /** Middle of the rail drawn by Felt; seats sit on an ellipse just outside it. */
  const CENTER = { x: 0.5, y: 0.44 };

  function place(seat: number) {
    const n = Math.max(hand.max_seats, 2);
    const visual = (((seat - heroSeat) % n) + n) % n;
    const a = Math.PI / 2 + (visual * 2 * Math.PI) / n;
    const x = CENTER.x + 0.41 * Math.cos(a);
    const y = CENTER.y + 0.335 * Math.sin(a);
    // Bets sit on the cloth between the seat and the pot.
    return { x, y, bx: x + (CENTER.x - x) * 0.46, by: y + (CENTER.y - y) * 0.5 };
  }

  function amount(v: number) {
    if (inBB && hand.big_blind > 0) return `${(v / hand.big_blind).toLocaleString("de-DE", { maximumFractionDigits: 1 })} BB`;
    return money(v, hand.currency, hand.game);
  }

  function bbs(v: number) {
    if (hand.big_blind <= 0) return "";
    const n = v / hand.big_blind;
    return n.toLocaleString("de-DE", { maximumFractionDigits: n < 10 ? 1 : 0 });
  }

  function cardsOf(name: string): string[] | null {
    if (name === hand.hero) return hand.hero_cards;
    if (table.revealed) return hand.shown.find(([n]) => n === name)?.[1] ?? null;
    return null;
  }

  function key(e: KeyboardEvent) {
    const t = e.target as HTMLElement | null;
    if (t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.tagName === "SELECT")) return;
    if (e.key === "ArrowRight") step = Math.min(step + 1, events.length);
    else if (e.key === "ArrowLeft") step = Math.max(step - 1, 0);
    else if (e.key === " ") playing = !playing;
    else return;
    e.preventDefault();
  }

  const STREET_NAME: Record<Street, string> = { preflop: "Preflop", flop: "Flop", turn: "Turn", river: "River" };
  const ICON = {
    first: "M6 5v14M18 6l-8 6 8 6z",
    prev: "M16 6l-8 6 8 6z",
    next: "M8 6l8 6-8 6z",
    last: "M18 5v14M6 6l8 6-8 6z",
    play: "M8 5.5v13l10-6.5z",
    pause: "M8 5.5v13M16 5.5v13",
  };
</script>

<svelte:window onkeydown={key} />

<div class="replayer">
  <div class="layout" style="--ar:{REFERENCE_ASPECT}">
    <div class="main">
      {@render children?.()}
      <div class="stage" bind:clientWidth={stageWidth} style="--k:{k}">
        <Felt>
          <div class="center">
            <div class="board">
              {#each hand.board.slice(0, table.board) as c (c)}
                <PlayingCard card={c} size={44} />
              {/each}
            </div>
            <div class="pot" class:hidden={table.total <= 0}>
              <span>Pot</span>
              <b class="num">{amount(Math.max(table.total, 0))}</b>
            </div>
            <div class="street">{STREET_NAME[table.street]}</div>
          </div>

          {#each hand.seats.filter((s) => !s.sitting_out) as s (s.seat)}
            {@const p = place(s.seat)}
            {@const bet = table.put.get(s.name) ?? 0}
            {#if bet > 0}
              <div class="bet num" style="left:{p.bx * 100}%; top:{p.by * 100}%">
                <i></i>{amount(bet)}
              </div>
            {/if}
          {/each}

          {#each hand.seats.filter((s) => !s.sitting_out) as s (s.seat)}
            {@const p = place(s.seat)}
            {@const cards = cardsOf(s.name)}
            {@const won = table.winners.get(s.name)}
            {@const stack = table.stacks.get(s.name) ?? 0}
            {@const pos = positions.get(s.name)}
            <div
              class="seat"
              class:folded={table.folded.has(s.name)}
              class:acting={table.last === s.name}
              class:winner={won !== undefined}
              class:hero={s.name === hand.hero}
              style="left:{p.x * 100}%; top:{p.y * 100}%"
            >
              <div class="cards">
                {#if cards}
                  {#each cards as c (c)}<PlayingCard card={c} size={30} />{/each}
                {:else if !table.folded.has(s.name)}
                  <PlayingCard hidden size={30} /><PlayingCard hidden size={30} />
                {/if}
              </div>
              <div class="pod">
                <div class="who">
                  {#if pos}<span class="tag">{pos}</span>{/if}
                  <span class="name" title={s.name}>{s.name}</span>
                </div>
                <div class="num stack">
                  {#if inBB}
                    <b>{bbs(stack)}</b> <span class="bbs">BB</span>
                  {:else}
                    <b>{amount(stack)}</b>{#if hand.big_blind > 0}<span class="sep">·</span><span class="bbs">{bbs(stack)} BB</span>{/if}
                  {/if}
                </div>
                {#if won !== undefined}
                  <span class="act win num">+{amount(won)}</span>
                {:else if table.label.get(s.name)}
                  <span class="act" class:allin={table.label.get(s.name) === "All-in"}>{table.label.get(s.name)}</span>
                {/if}
              </div>
              {#if s.seat === hand.button_seat}<span class="dealer" title="Dealer-Button">D</span>{/if}
            </div>
          {/each}
        </Felt>
      </div>

      <div class="controls">
        <div class="buttons">
          <button class="icon-btn" onclick={() => (step = 0)} title="Zum Anfang" aria-label="Zum Anfang">
            <svg viewBox="0 0 24 24" width="16" height="16"><path d={ICON.first} /></svg>
          </button>
          <button class="icon-btn" onclick={() => (step = Math.max(step - 1, 0))} title="Zurück (←)" aria-label="Zurück">
            <svg viewBox="0 0 24 24" width="16" height="16"><path d={ICON.prev} /></svg>
          </button>
          <button
            class="btn primary play"
            onclick={() => {
              if (done) step = 0;
              playing = !playing;
            }}
            title="Abspielen oder anhalten (Leertaste)"
          >
            <svg viewBox="0 0 24 24" width="14" height="14"><path d={playing ? ICON.pause : ICON.play} /></svg>
            {playing ? "Anhalten" : done ? "Nochmal" : "Abspielen"}
          </button>
          <button class="icon-btn" onclick={() => (step = Math.min(step + 1, events.length))} title="Weiter (→)" aria-label="Weiter">
            <svg viewBox="0 0 24 24" width="16" height="16"><path d={ICON.next} /></svg>
          </button>
          <button class="icon-btn" onclick={() => (step = events.length)} title="Zum Ende" aria-label="Zum Ende">
            <svg viewBox="0 0 24 24" width="16" height="16"><path d={ICON.last} /></svg>
          </button>
        </div>
        <input type="range" min="0" max={events.length} bind:value={step} aria-label="Schritt" />
        <span class="steps num">{step}/{events.length}</span>
        <div class="seg" role="group" aria-label="Beträge anzeigen in">
          <button class:on={!inBB} onclick={() => (inBB = false)}>{hand.game === "tournament" ? "Chips" : "Geld"}</button>
          <button class:on={inBB} onclick={() => (inBB = true)}>BB</button>
        </div>
      </div>

    </div>

    <div class="side">
      {#if done && hand.bounties_won.length}
        <div class="bounties">
          <span class="dim">Bounty gewonnen</span>
          {#each hand.bounties_won as [name, value] (name)}
            <span><b>{name}</b> <span class="num pos">{moneyShort(value, hand.bounty_currency ?? "USD")}</span></span>
          {/each}
        </div>
      {/if}
      <div class="log" bind:this={logEl}>
        {#each log as g (g.street)}
          <div class="street-log">
            <h4 class:on={table.street === g.street && step > 0}>{STREET_NAME[g.street]}</h4>
            <ol>
              {#each g.items as { i, a } (i)}
                <li class:future={i >= step} class:now={i === step - 1}>
                  <button onclick={() => (step = i + 1)}>
                    <span class="actor" title={a.player}>{a.player}</span>
                    <span class="what">{a.all_in ? "All-in" : LABEL[a.kind]}</span>
                    {#if a.amount > 0}<span class="num amt">{amount(a.to ?? a.amount)}</span>{/if}
                  </button>
                </li>
              {/each}
            </ol>
          </div>
        {/each}
      </div>
    </div>
  </div>
</div>

<style>
  /*
   * The table keeps the client's window shape at any size: its width is the smallest of the free
   * width, a cap, and what the free height allows. The action log sits below it, or beside it once
   * there is room for both.
   */
  .replayer {
    flex: 1;
    min-width: 0;
    min-height: 0;
    container: replayer / size;
  }
  .layout {
    --chrome: 80px;
    --log-min: clamp(120px, 20cqh, 200px);
    --w: min(100cqw, 1100px, (100cqh - var(--chrome) - var(--log-min) - 14px) * var(--ar));
    display: grid;
    grid-template-columns: var(--w);
    grid-template-rows: auto minmax(var(--log-min), 1fr);
    justify-content: center;
    gap: 14px;
    height: 100%;
  }
  .main {
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-width: 0;
  }
  .stage {
    position: relative;
    aspect-ratio: var(--ar);
    border-radius: var(--r-lg);
    overflow: hidden;
    border: 1px solid var(--line);
  }
  .center {
    position: absolute;
    left: 50%;
    top: 44%;
    transform: translate(-50%, -50%) scale(var(--k));
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 7px;
  }
  .board {
    display: flex;
    gap: 5px;
    min-height: 62px;
  }
  .pot {
    display: flex;
    align-items: baseline;
    gap: 7px;
    padding: 3px 12px;
    border-radius: 99px;
    background: rgba(0, 0, 0, 0.42);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.06);
    font-size: 12px;
    color: var(--text-2);
  }
  .pot b {
    font-size: 15px;
    font-weight: 600;
    color: var(--text);
  }
  .pot.hidden {
    visibility: hidden;
  }
  .street {
    font-size: 11.5px;
    font-weight: 600;
    color: rgba(236, 235, 231, 0.4);
  }

  .seat {
    position: absolute;
    z-index: 2;
    transform: translate(-50%, -50%) scale(var(--k));
    display: flex;
    flex-direction: column;
    align-items: center;
    transition: opacity 0.2s;
  }
  .seat.folded {
    opacity: 0.55;
  }
  .cards {
    display: flex;
    gap: 3px;
    height: 42px;
    margin-bottom: -10px;
    z-index: 1;
  }
  .pod {
    position: relative;
    min-width: 116px;
    padding: 12px 12px 9px;
    border-radius: 8px;
    background: var(--plate);
    box-shadow:
      inset 0 0 0 1px var(--plate-edge),
      0 6px 16px rgba(0, 0, 0, 0.45);
    text-align: center;
    transition: box-shadow 0.15s;
  }
  .acting .pod {
    box-shadow:
      inset 0 0 0 1.5px var(--text),
      0 6px 16px rgba(0, 0, 0, 0.45);
  }
  .winner .pod {
    box-shadow:
      inset 0 0 0 1.5px var(--pos),
      0 6px 16px rgba(0, 0, 0, 0.45);
  }
  .who {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    max-width: 150px;
  }
  .tag {
    flex-shrink: 0;
    padding: 0 4px;
    border-radius: 3px;
    background: rgba(255, 255, 255, 0.08);
    font-family: var(--font-num);
    font-size: 10.5px;
    font-weight: 600;
    line-height: 15px;
    color: var(--text-2);
  }
  .name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12.5px;
    color: var(--text-2);
  }
  .hero .name {
    color: var(--text);
  }
  .stack {
    margin-top: 1px;
    font-size: 13px;
    line-height: 1.3;
    white-space: nowrap;
  }
  .stack b {
    font-size: 15px;
    font-weight: 700;
    color: #fff;
  }
  .sep {
    margin: 0 5px;
    color: var(--text-3);
  }
  .bbs {
    color: var(--pos);
    font-weight: 600;
  }
  .act {
    position: absolute;
    left: 50%;
    bottom: -10px;
    transform: translateX(-50%);
    padding: 0 8px;
    border-radius: 99px;
    background: var(--surface-3);
    border: 1px solid var(--line-2);
    color: var(--text);
    font-size: 11px;
    font-weight: 600;
    line-height: 17px;
    white-space: nowrap;
  }
  .act.allin {
    background: var(--text);
    color: var(--bg);
    border-color: var(--text);
  }
  .act.win {
    background: #1d2a1a;
    border-color: color-mix(in srgb, var(--pos) 55%, transparent);
    color: var(--pos);
  }
  .dealer {
    position: absolute;
    right: -9px;
    top: 30px;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: var(--text);
    color: var(--bg);
    font-family: var(--font-num);
    font-size: 10.5px;
    font-weight: 700;
    display: grid;
    place-items: center;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.5);
  }
  .bet {
    position: absolute;
    z-index: 1;
    transform: translate(-50%, -50%) scale(var(--k));
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 2px 9px 2px 3px;
    border-radius: 99px;
    background: rgba(0, 0, 0, 0.5);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.06);
    font-size: 13px;
    font-weight: 600;
    white-space: nowrap;
  }
  .bet i {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: var(--text);
    box-shadow:
      inset 0 0 0 2px #8f9198,
      inset 0 0 0 4px var(--text);
  }

  .controls {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .buttons {
    display: flex;
    align-items: center;
    gap: 2px;
  }
  .buttons svg path {
    fill: currentColor;
  }
  .play {
    min-width: 116px;
    margin: 0 4px;
  }
  .play svg path {
    fill: currentColor;
    stroke: currentColor;
  }
  input[type="range"] {
    flex: 1;
    min-width: 60px;
    margin: 0 4px;
  }
  .steps {
    font-size: 12px;
    min-width: 36px;
    text-align: right;
    color: var(--text-2);
  }

  .bounties {
    flex-shrink: 0;
    max-height: 40%;
    overflow: auto;
    display: flex;
    flex-wrap: wrap;
    gap: 6px 16px;
    padding: 8px 12px;
    border-radius: var(--r-sm);
    background: var(--surface);
    border: 1px solid var(--line);
    font-size: 13px;
  }
  .bounties b {
    font-weight: 500;
  }

  .side {
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-height: 0;
  }
  .log {
    flex: 1;
    min-height: 0;
    overflow: auto;
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    border: 1px solid var(--line);
    border-radius: var(--r-lg);
    background: var(--surface);
  }
  .street-log {
    min-width: 0;
    border-left: 1px solid var(--line);
  }
  .street-log:first-child {
    border-left: 0;
  }
  h4 {
    position: sticky;
    top: 0;
    margin: 0;
    padding: 9px 10px 6px;
    background: var(--surface);
    font-size: 12.5px;
    font-weight: 600;
    color: var(--text-2);
  }
  h4.on {
    color: var(--text);
  }
  ol {
    margin: 0;
    padding: 0 0 6px;
    list-style: none;
  }
  ol button {
    display: flex;
    align-items: baseline;
    gap: 6px;
    width: 100%;
    padding: 3px 10px;
    border: 0;
    border-left: 2px solid transparent;
    background: transparent;
    font-size: 12.5px;
    text-align: left;
    color: var(--text-2);
    cursor: pointer;
  }
  ol button:hover {
    background: var(--surface-2);
  }
  .future button {
    color: color-mix(in srgb, var(--text-2) 70%, transparent);
  }
  .now button {
    color: var(--text);
    background: var(--surface-3);
    border-left-color: var(--text);
  }
  .actor {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .amt {
    font-size: 12.5px;
  }
  @container replayer (min-width: 1150px) {
    .layout {
      --log-w: clamp(280px, 24cqw, 360px);
      --w: min(100cqw - var(--log-w) - 18px, 1100px, (100cqh - var(--chrome)) * var(--ar));
      grid-template-columns: var(--w) var(--log-w);
      grid-template-rows: auto;
      align-content: start;
      gap: 18px;
      height: auto;
    }
    /* Beside the table the log only takes the table's height and scrolls within it. */
    .side {
      contain: size;
    }
    .log {
      display: block;
    }
    .street-log {
      border-left: 0;
      border-top: 1px solid var(--line);
    }
    .street-log:first-child {
      border-top: 0;
    }
  }
</style>
