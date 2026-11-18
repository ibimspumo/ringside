<script lang="ts">
  import { untrack, type Snippet } from "svelte";
  import { REFERENCE_ASPECT } from "../hud/geometry";
  import OvalFelt from "../table/OvalFelt.svelte";
  import { clear, layout, placeOf } from "../table/seats";
  import PlayingCard from "../PlayingCard.svelte";
  import { openInSolver } from "../solver/handoff.svelte";
  import { spotFromHand } from "../solver/presets";
  import { money, moneyShort } from "../stats";
  import type { Action, Hand, Street } from "../types";

  /**
   * `children` is the hand's title line, laid out flush with the table. With `upTo` the hand ends
   * before that action (an index into `actions`): no later cards, no showdown, no winner.
   * `marks` colours actions (by index into `actions`) on the log and the timeline; clicking one
   * calls `onMark`. A new `jumpTo` moves the replay to just before its action. `aside` sits
   * above the action log. `quiz` is for practising the hand: while the answer is open, the
   * solver hand-off is hidden and the play button stays quiet. `inBigBlinds` starts the amounts
   * in big blinds instead of chips. `quietPlay` keeps the play button in outline, for screens
   * that have their own main action.
   */
  let {
    hand: full,
    upTo = null,
    marks = null,
    onMark,
    jumpTo = null,
    children,
    aside,
    quiz = false,
    inBigBlinds = false,
    quietPlay = false,
  }: {
    hand: Hand;
    upTo?: number | null;
    marks?: Map<number, { color: string; label: string }> | null;
    onMark?: (action: number) => void;
    jumpTo?: { action: number } | null;
    children?: Snippet;
    aside?: Snippet;
    quiz?: boolean;
    inBigBlinds?: boolean;
    quietPlay?: boolean;
  } = $props();

  type Event =
    | { type: "action"; action: Action; index: number }
    | { type: "street"; street: Street }
    | { type: "showdown" }
    | { type: "collect" };

  const STREETS: Street[] = ["preflop", "flop", "turn", "river"];
  const BOARD_AT: Record<Street, number> = { preflop: 0, flop: 3, turn: 4, river: 5 };

  const hand = $derived.by((): Hand => {
    if (upTo === null) return full;
    const street = full.actions[upTo]?.street ?? full.actions.at(-1)?.street ?? "preflop";
    return {
      ...full,
      actions: full.actions.slice(0, upTo),
      board: full.board.slice(0, BOARD_AT[street]),
      shown: [],
      collected: [],
      returned: [],
      bounties_won: [],
    };
  });

  const events = $derived.by(() => {
    const out: Event[] = [];
    let street: Street = "preflop";
    hand.actions.forEach((action, index) => {
      while (action.street !== street) {
        street = STREETS[STREETS.indexOf(street) + 1];
        out.push({ type: "street", street });
      }
      out.push({ type: "action", action, index });
    });
    while (STREETS.indexOf(street) < 3 && hand.board.length > BOARD_AT[street]) {
      street = STREETS[STREETS.indexOf(street) + 1];
      out.push({ type: "street", street });
    }
    if (hand.shown.length) out.push({ type: "showdown" });
    if (upTo === null) out.push({ type: "collect" });
    return out;
  });

  let step = $state(0);
  let playing = $state(false);
  let inBB = $state(untrack(() => inBigBlinds));
  let stageWidth = $state(860);
  let logEl = $state<HTMLElement>();
  /** The stage keeps its shape, so seats, board and bets simply scale with its width. */
  const k = $derived(Math.min(1.15, Math.max(0.8, stageWidth / 880)));

  // A new hand starts at the beginning, a stopped one at its stop; lifting the stop plays on.
  let shownId: number | null = null;
  $effect(() => {
    const id = full.id;
    const stop = upTo;
    untrack(() => {
      if (id !== shownId) {
        shownId = id;
        step = stop === null ? 0 : events.length;
        playing = false;
      } else if (stop === null) {
        playing = true;
      }
    });
  });

  $effect(() => {
    const target = jumpTo;
    if (!target) return;
    untrack(() => {
      const at = events.findIndex((e) => e.type === "action" && e.index === target.action);
      if (at >= 0) {
        step = at;
        playing = false;
      }
    });
  });

  /** Marked actions with their place on the timeline. */
  const pips = $derived(
    marks
      ? events.flatMap((e, i) => (e.type === "action" && marks.has(e.index) ? [{ i, index: e.index, ...marks.get(e.index)! }] : []))
      : [],
  );

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
  /** The heads-up spot at the start of the street on screen, for the solver. */
  const solveSpot = $derived(table.street === "preflop" ? null : spotFromHand(hand, table.street, positions));

  /** Actions grouped by street, each with its index in `events` so the log can follow the replay. */
  const log = $derived.by(() => {
    const out: { street: Street; items: { i: number; a: Action; index: number }[] }[] = [];
    events.forEach((e, i) => {
      if (e.type !== "action") return;
      let group = out[out.length - 1];
      if (!group || group.street !== e.action.street) out.push((group = { street: e.action.street, items: [] }));
      group.items.push({ i, a: e.action, index: e.index });
    });
    return out;
  });

  // You sit at the bottom, the others clockwise by seat number (see `table/seats.ts`).
  const stageHeight = $derived(stageWidth / REFERENCE_ASPECT);
  // Smaller tables (e.g. beside the hand list) shrink seats and chips until nothing overlaps.
  const sizesAt = (kk: number) => ({ seat: { w: 124 * kk, h: 92 * kk }, badge: { w: 1, h: 1 }, bet: { w: 90 * kk, h: 24 * kk }, gap: 3 * kk });
  const middleAt = (kk: number) => [{ box: { w: 236 * kk, h: 100 * kk }, dy: 0 }];
  const fitted = $derived.by(() => {
    const n = Math.max(hand.max_seats, 2);
    for (let kk = k; kk > 0.4; kk -= 0.02) {
      const g = layout(n, stageWidth, stageHeight, sizesAt(kk));
      if (clear(g, sizesAt(kk), middleAt(kk))) return { geo: g, kk };
    }
    return { geo: layout(n, stageWidth, stageHeight, sizesAt(0.4)), kk: 0.4 };
  });
  const geo = $derived(fitted.geo);
  /** Seat number `seat` (1 … max) in fractions of the stage, with its chips. */
  function place(seat: number) {
    const n = Math.max(hand.max_seats, 2);
    const p = geo.places[placeOf(seat - 1, heroSeat - 1, n)];
    return { x: p.seat.x / stageWidth, y: p.seat.y / stageHeight, bx: p.bet.x / stageWidth, by: p.bet.y / stageHeight };
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
  <div class="layout" class:has-aside={!!aside} style="--ar:{REFERENCE_ASPECT}">
    <div class="main">
      {@render children?.()}
      <div class="stage" bind:clientWidth={stageWidth} style="--k:{fitted.kk}">
        <div class="felt">
          <OvalFelt felt={geo.felt} />
          <div class="center" data-part="middle" style="left:{geo.center.x}px; top:{geo.center.y}px">
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
              <div class="bet num" data-part="bet" data-seat={s.seat - 1} style="left:{p.bx * 100}%; top:{p.by * 100}%">
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
              data-part="seat"
              data-seat={s.seat - 1}
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
        </div>
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
            class="btn play"
            class:primary={!quiz && !quietPlay}
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
        <div class="track">
          <input type="range" min="0" max={events.length} bind:value={step} aria-label="Schritt" />
          {#each pips as p (p.index)}
            <button
              class="pip"
              class:at={step === p.i}
              style="--at:{p.i / Math.max(events.length, 1)}; background:{p.color}"
              title={p.label}
              aria-label={p.label}
              onclick={() => (onMark ? onMark(p.index) : (step = p.i))}
            ></button>
          {/each}
        </div>
        <span class="steps num">{step}/{events.length}</span>
        <div class="seg" role="group" aria-label="Beträge anzeigen in">
          <button class:on={!inBB} onclick={() => (inBB = false)}>{hand.game === "tournament" ? "Chips" : "Geld"}</button>
          <button class:on={inBB} onclick={() => (inBB = true)}>BB</button>
        </div>
        {#if solveSpot && !quiz}
          <button
            class="btn solve"
            disabled={"reason" in solveSpot}
            title={"reason" in solveSpot ? solveSpot.reason : "Board, Pot und Stack dieser Straße im Solver öffnen; die Ranges sind ein Vorschlag."}
            onclick={() => "prefill" in solveSpot && openInSolver(solveSpot.prefill)}
          >
            Im Solver nachrechnen
          </button>
        {/if}
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
      {#if aside}<div class="aside">{@render aside()}</div>{/if}
      <div class="log" bind:this={logEl}>
        {#each log as g (g.street)}
          <div class="street-log">
            <h4 class:on={table.street === g.street && step > 0}>{STREET_NAME[g.street]}</h4>
            <ol>
              {#each g.items as { i, a, index } (i)}
                {@const mark = marks?.get(index)}
                <li class:future={i >= step} class:now={i === step - 1}>
                  <button onclick={() => (mark && onMark ? onMark(index) : (step = i + 1))} title={mark?.label}>
                    {#if mark}<span class="mark" style="background:{mark.color}"></span>{/if}
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
  .felt {
    position: absolute;
    inset: 0;
    overflow: hidden;
    background: var(--bg);
  }
  .center {
    position: absolute;
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
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 10px;
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
  .track {
    position: relative;
    flex: 1;
    min-width: 60px;
    display: flex;
    align-items: center;
    margin: 0 4px;
  }
  input[type="range"] {
    flex: 1;
    min-width: 0;
    margin: 0;
  }
  /* The thumb is about 16px wide, so its centre runs from 8px to the width minus 8px. */
  .pip {
    position: absolute;
    top: -9px;
    left: calc(8px + (100% - 16px) * var(--at));
    width: 10px;
    height: 10px;
    padding: 0;
    border: 2px solid var(--bg);
    border-radius: 50%;
    transform: translateX(-50%);
    cursor: pointer;
  }
  .pip:hover,
  .pip.at {
    box-shadow: 0 0 0 2px var(--text);
  }
  .mark {
    flex-shrink: 0;
    align-self: center;
    width: 8px;
    height: 8px;
    border-radius: 50%;
  }
  /* Below the table the analysis needs room; the table gives up some height for it. */
  .layout.has-aside {
    --log-min: clamp(220px, 46cqh, 460px);
  }
  .aside {
    flex: 0 1 auto;
    min-height: 0;
    overflow: auto;
  }
  .has-aside .log {
    min-height: 72px;
  }
  .steps {
    font-size: 12px;
    min-width: 36px;
    text-align: right;
    color: var(--text-2);
  }
  .solve {
    height: 28px;
    padding: 0 10px;
    font-size: 12.5px;
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
