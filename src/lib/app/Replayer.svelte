<script lang="ts">
  import { untrack, type Snippet } from "svelte";
  import { preview } from "../learn/preview";
  import { CATEGORY, categoryOf } from "../learn/wissen/poker";
  import { openInSolver } from "../solver/handoff.svelte";
  import { spotFromHand } from "../solver/presets";
  import { money, moneyShort } from "../stats";
  import type { Action, Hand, Street } from "../types";
  import BestHand from "../ui/BestHand.svelte";
  import Button from "../ui/Button.svelte";
  import GradeIcon from "../ui/GradeIcon.svelte";
  import IconButton from "../ui/IconButton.svelte";
  import PokerTable, { CLEAN_FROM, tableHeight, type TableSeat } from "../ui/PokerTable.svelte";
  import type { ActionKind, Grade } from "../ui/poker";
  import SegmentedControl from "../ui/SegmentedControl.svelte";

  /**
   * `children` is the hand's title line, laid out flush with the table. With `upTo` the hand ends
   * before that action (an index into `actions`): no later cards, no showdown, no winner.
   * `marks` grades actions (by index into `actions`) on the table, the log and the timeline;
   * clicking one calls `onMark`. A new `jumpTo` moves the replay to just before its action.
   * `aside` (e.g. the analysis) opens a column beside the table, above the log, once the
   * replayer is wide enough; callers show it elsewhere on narrow screens (see `asideFrom`).
   * `quiz` is for practising the hand: while the answer is open, the solver hand-off is hidden
   * and the play button stays quiet. `inBigBlinds` starts the amounts in big blinds instead of
   * chips. `quietPlay` keeps the play button in outline, for screens that have their own main
   * action.
   */
  let {
    hand: full,
    upTo = null,
    marks = null,
    onMark,
    onStep,
    jumpTo = null,
    children,
    aside,
    quiz = false,
    inBigBlinds = false,
    quietPlay = false,
  }: {
    hand: Hand;
    upTo?: number | null;
    marks?: Map<number, { grade: Grade | null; label: string }> | null;
    onMark?: (action: number) => void;
    /** The action the replay stands before (index into `actions`), or null once all are played. */
    onStep?: (next: number | null) => void;
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
  let logEl = $state<HTMLElement>();
  let sideEl = $state<HTMLElement>();

  // A new hand starts at the beginning, a stopped one at its stop; lifting the stop plays on.
  let shownId: number | null = null;
  $effect(() => {
    const id = full.id;
    const stop = upTo;
    untrack(() => {
      if (id !== shownId) {
        shownId = id;
        const at = preview("step");
        step = stop === null ? (at === "end" ? events.length : Number(at ?? 0) || 0) : events.length;
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

  $effect(() => {
    const next = events.slice(step).find((e) => e.type === "action");
    const at = next?.type === "action" ? next.index : null;
    untrack(() => onStep?.(at));
  });

  /** Graded actions with their place on the timeline. */
  const pips = $derived(
    marks
      ? events.flatMap((e, i) => {
          const m = e.type === "action" ? marks.get(e.index) : undefined;
          return m && e.type === "action" ? [{ i, index: e.index, ...m }] : [];
        })
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
    const scroller = logEl;
    const row = logEl?.querySelector<HTMLElement>("li.now");
    if (!scroller || !row) return;
    const box = scroller.getBoundingClientRect();
    const r = row.getBoundingClientRect();
    if (r.top < box.top + 30) scroller.scrollTop -= box.top + 30 - r.top;
    else if (r.bottom > box.bottom) scroller.scrollTop += r.bottom - box.bottom + 4;
  });

  const POST: Partial<Record<Action["kind"], string>> = {
    post_small_blind: "Small Blind",
    post_big_blind: "Big Blind",
    post_ante: "Ante",
    post_both: "SB + BB",
  };
  const KIND: Partial<Record<Action["kind"], ActionKind>> = { fold: "fold", check: "check", call: "call", bet: "bet", raise: "raise" };
  const WORD: Record<ActionKind, string> = { fold: "Fold", check: "Check", call: "Call", bet: "Bet", raise: "Raise", allin: "All-in", limp: "Limp" };
  const kindOf = (a: Action): ActionKind | undefined => (a.all_in ? "allin" : KIND[a.kind]);

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
    const label = new Map<string, { kind: ActionKind; index: number }>();
    const folded = new Set<string>();
    let pot = 0;
    let board = 0;
    let revealed = false;
    let winners = new Map<string, number>();
    let street: Street = "preflop";
    let latest = -1;

    const sweep = () => {
      for (const v of put.values()) pot += v;
      put.clear();
    };

    for (const e of events.slice(0, step)) {
      if (e.type === "action") {
        const a = e.action;
        stacks.set(a.player, (stacks.get(a.player) ?? 0) - a.amount);
        if (a.kind === "post_ante") pot += a.amount;
        else put.set(a.player, (put.get(a.player) ?? 0) + a.amount);
        if (a.kind === "fold") folded.add(a.player);
        const kind = kindOf(a);
        if (kind) label.set(a.player, { kind, index: e.index });
        latest = e.index;
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
    const next = events[step];
    const toAct = next?.type === "action" ? next.action.player : null;
    // Who folded is dimmed; the tag only shows in the step the fold happens.
    for (const [name, l] of label) if (l.kind === "fold" && l.index !== latest) label.delete(name);
    return { stacks, put, label, folded, pot, total: pot + inFront, board, revealed, winners, street, toAct };
  });

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

  const unit = $derived(hand.big_blind > 0 ? hand.big_blind : 1);
  const n = $derived(Math.max(hand.max_seats, 2));
  const heroSeat = $derived(hand.seats.find((s) => s.name === hand.hero)?.seat ?? 1);

  function cardsOf(name: string): string[] | null {
    if (name === hand.hero) return hand.hero_cards;
    if (table.revealed) return hand.shown.find(([p]) => p === name)?.[1] ?? null;
    return null;
  }

  // You sit at the bottom, the others clockwise by seat number (see `table/seats.ts`).
  const seats = $derived.by((): TableSeat[] =>
    Array.from({ length: n }, (_, k) => {
      const s = hand.seats.find((x) => x.seat === k + 1 && !x.sitting_out);
      if (!s) return { empty: true };
      const cards = cardsOf(s.name);
      const last = table.label.get(s.name);
      const won = table.winners.get(s.name);
      const bet = table.put.get(s.name) ?? 0;
      return {
        name: s.name,
        position: positions.get(s.name),
        stack: (table.stacks.get(s.name) ?? 0) / unit,
        cards: cards?.length ? cards : table.folded.has(s.name) ? undefined : "hidden",
        // Whoever is to act shows no older action: the plate's ring says "your turn".
        action: last && !(table.toAct === s.name && !done) ? { kind: last.kind } : undefined,
        grade: last && !(table.toAct === s.name && !done) ? (marks?.get(last.index)?.grade ?? undefined) : undefined,
        bet: bet > 0 ? bet / unit : undefined,
        folded: table.folded.has(s.name),
        toAct: !done && table.toAct === s.name,
        dealer: s.seat === hand.button_seat,
        won: won !== undefined ? won / unit : undefined,
      };
    }),
  );

  /** Writes an amount given in big blinds in the unit chosen below the table. */
  function format(v: number): string {
    if (inBB || hand.big_blind <= 0) return `${v.toLocaleString("de-DE", { maximumFractionDigits: v < 10 ? 1 : 0 })} BB`;
    return money(v * unit, hand.currency, hand.game);
  }

  /** What every shown hand made, winners first, for the result below the log. */
  const result = $derived.by(() => {
    if (!done || upTo !== null) return [];
    const names = new Set([...hand.collected.map(([p]) => p), ...hand.shown.map(([p]) => p)]);
    return [...names]
      .map((name) => ({
        name,
        position: positions.get(name),
        won: table.winners.get(name),
        cards: name === hand.hero && hand.shown.some(([p]) => p === name) ? hand.hero_cards : (hand.shown.find(([p]) => p === name)?.[1] ?? null),
      }))
      .sort((a, b) => (b.won ?? 0) - (a.won ?? 0));
  });

  // The table takes the largest width whose height still fits, beside the side column or above the log.
  let boxW = $state(0);
  let boxH = $state(0);
  let headH = $state(0);
  let ctrlH = $state(40);
  const LOG_MIN = 150;
  const besides = $derived(boxW >= (aside ? 860 : 1150));
  const sideW = $derived(besides ? Math.round(aside ? Math.min(420, Math.max(300, boxW * 0.28)) : Math.min(360, Math.max(280, boxW * 0.24))) : 0);
  const fit = $derived.by(() => {
    const room = Math.min(besides ? boxW - sideW - 20 : boxW, 1100);
    const free = boxH - headH - ctrlH - 20 - (besides ? 0 : LOG_MIN + 12);
    return fitTable(free, room);
  });
  const MIN_SCALE = 0.85;
  /**
   * The widest table whose height fits `free`. Below the width where chips and tags would
   * overlap it takes the flattest clean shape instead, shrunk by at most 15 % to fit; whatever
   * is still missing, the box scrolls. A room narrower than the clean width keeps its width.
   */
  function fitTable(free: number, room: number): { w: number; f: number } {
    const floor = Math.min(room, CLEAN_FROM);
    for (let w = room; w >= floor; w -= 10) if (tableHeight(w) <= free) return { w, f: 1 };
    let best = room;
    for (let w = room; w >= floor; w -= 10) if (tableHeight(w) < tableHeight(best)) best = w;
    return { w: best, f: Math.max(MIN_SCALE, Math.min(1, free / tableHeight(best))) };
  }

  /** Under the board at the end: who won and with what, in words. */
  const outcome = $derived.by(() => {
    if (!done || upTo !== null || !table.winners.size) return null;
    const [name] = [...table.winners.entries()].sort((a, b) => b[1] - a[1])[0];
    const who = name === hand.hero ? "Du gewinnst" : `${name} gewinnt`;
    const cards = name === hand.hero ? hand.hero_cards : hand.shown.find(([p]) => p === name)?.[1];
    if (!hand.shown.length || !cards?.length || hand.board.length < 3) return `${who} – alle anderen haben gefoldet`;
    return `${who}: ${CATEGORY[categoryOf([...cards, ...hand.board])]}`;
  });

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

<div class="replayer" class:besides bind:clientWidth={boxW} bind:clientHeight={boxH}>
  <div class="layout" class:besides style:--side="{sideW}px" style:--table="{fit.w * fit.f}px">
    <div class="main">
      {#if children}<div bind:clientHeight={headH}>{@render children()}</div>{/if}
      <div class="stage" style:height={fit.f < 1 ? `${tableHeight(fit.w) * fit.f}px` : undefined}>
        <div class="scaler" style:width={fit.f < 1 ? `${fit.w}px` : undefined} style:transform={fit.f < 1 ? `scale(${fit.f})` : undefined}>
        <PokerTable
          {seats}
          hero={heroSeat - 1}
          board={hand.board.slice(0, table.board)}
          pot={Math.max(table.total, 0) / unit}
          note={outcome ?? STREET_NAME[table.street]}
          noteWin={!!outcome}
          {format}
          label="Tisch der Hand"
        />
        </div>
      </div>

      <div class="controls" bind:clientHeight={ctrlH}>
        <div class="buttons">
          <IconButton label="Zum Anfang" onclick={() => (step = 0)}>
            <svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true"><path d={ICON.first} /></svg>
          </IconButton>
          <IconButton label="Zurück (←)" onclick={() => (step = Math.max(step - 1, 0))}>
            <svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true"><path d={ICON.prev} /></svg>
          </IconButton>
          <Button
            class="play"
            variant={quiz || quietPlay ? "secondary" : "primary"}
            onclick={() => {
              if (done) step = 0;
              playing = !playing;
            }}
            title="Abspielen oder anhalten (Leertaste)"
          >
            {#snippet icon()}<svg viewBox="0 0 24 24" width="14" height="14" aria-hidden="true"><path d={playing ? ICON.pause : ICON.play} /></svg>{/snippet}
            {playing ? "Anhalten" : done ? "Nochmal" : "Abspielen"}
          </Button>
          <IconButton label="Weiter (→)" onclick={() => (step = Math.min(step + 1, events.length))}>
            <svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true"><path d={ICON.next} /></svg>
          </IconButton>
          <IconButton label="Zum Ende" onclick={() => (step = events.length)}>
            <svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true"><path d={ICON.last} /></svg>
          </IconButton>
        </div>
        <div class="track">
          <input type="range" min="0" max={events.length} bind:value={step} aria-label="Schritt in der Hand" />
          {#each pips as p (p.index)}
            <button
              class="pip"
              class:at={step === p.i}
              style:--at={p.i / Math.max(events.length, 1)}
              style:--c={p.grade ? `var(--grade-${p.grade})` : "var(--text-3)"}
              class:open={!p.grade}
              title={p.label}
              aria-label={p.label}
              onclick={() => (onMark ? onMark(p.index) : (step = p.i))}
            ></button>
          {/each}
        </div>
        <span class="steps num" title="Schritt in der Hand">{step}/{events.length}</span>
        <SegmentedControl
          size="sm"
          label="Beträge anzeigen in"
          options={[
            { value: "money", label: hand.game === "tournament" ? "Chips" : "Geld" },
            { value: "bb", label: "BB", hint: "In Big Blinds: so lassen sich Hände aller Limits vergleichen." },
          ]}
          bind:value={() => (inBB ? "bb" : "money"), (v) => (inBB = v === "bb")}
        />
        {#if solveSpot && !quiz}
          <Button
            size="sm"
            variant="quiet"
            disabled={"reason" in solveSpot}
            title={"reason" in solveSpot ? solveSpot.reason : "Board, Pot und Stacks dieser Straße im Solver öffnen; die Ranges sind ein Vorschlag."}
            onclick={() => "prefill" in solveSpot && openInSolver(solveSpot.prefill)}
          >
            {STREET_NAME[table.street]} im Solver
          </Button>
        {/if}
      </div>
    </div>

    <div class="side" bind:this={sideEl}>
      {#if aside && besides}<div class="aside">{@render aside()}</div>{/if}
      {#if done && hand.bounties_won.length}
        <div class="bounties">
          <span class="dim">Bounty gewonnen</span>
          {#each hand.bounties_won as [name, value] (name)}
            <span><b>{name === hand.hero ? "Du" : name}</b> <span class="num pos">{moneyShort(value, hand.bounty_currency ?? "USD")}</span></span>
          {/each}
        </div>
      {/if}
      <div class="log" bind:this={logEl} aria-label="Zugverlauf">
        {#each log as g (g.street)}
          <div class="street-log">
            <h4 class:on={table.street === g.street && step > 0}>{STREET_NAME[g.street]}</h4>
            <ol>
              {#each g.items as { i, a, index } (i)}
                {@const mark = marks?.get(index)}
                {@const kind = kindOf(a)}
                {@const me = a.player === hand.hero}
                <li class:future={i >= step} class:now={i === step - 1}>
                  <button
                    class:marked={!!mark}
                    onclick={() => (mark && onMark ? onMark(index) : (step = i + 1))}
                    title="{me ? `Du (${a.player})` : a.player}{mark ? ` · ${mark.label}` : ''}"
                  >
                    <span class="pos num" class:me>{positions.get(a.player) ?? "–"}</span>
                    <span class="actor">{me ? "Du" : a.player}</span>
                    <span class="what" style:--c={kind ? `var(--act-${kind === "allin" ? "allin" : kind === "raise" || kind === "bet" ? "bet-3" : kind})` : undefined}
                      >{kind ? WORD[kind] : (POST[a.kind] ?? a.kind)}</span
                    >
                    {#if a.amount > 0}<span class="num amt">{format((a.to ?? a.amount) / unit)}</span>{/if}
                    {#if mark?.grade}<GradeIcon grade={mark.grade} size={14} />{:else if mark}<i class="open-mark" aria-hidden="true"></i>{/if}
                  </button>
                </li>
              {/each}
            </ol>
          </div>
        {/each}
        {#if result.length}
          <div class="result">
            <h4>Ergebnis</h4>
            {#each result as r (r.name)}
              <div class="who-won" class:winner={r.won !== undefined}>
                <span class="line">
                  <span class="pos num" class:me={r.name === hand.hero}>{r.position ?? "–"}</span>
                  <span class="actor" title={r.name}>{r.name === hand.hero ? "Du" : r.name}</span>
                  {#if r.won !== undefined}<b class="num gain">+{format(r.won / unit)}</b>{:else}<span class="lost">verliert</span>{/if}
                </span>
                {#if r.cards && hand.board.length >= 3}
                  <BestHand hole={r.cards} board={hand.board} winner={r.won !== undefined} size={24} />
                {:else if r.won !== undefined && !hand.shown.length}
                  <span class="faint">ohne Showdown – alle anderen haben gefoldet</span>
                {/if}
              </div>
            {/each}
          </div>
        {/if}
      </div>
    </div>
  </div>
</div>

<style>
  .replayer {
    flex: 1;
    min-width: 0;
    min-height: 0;
  }
  /* Beside a side column in a low window the replayer scrolls rather than cutting off your seat. */
  .replayer.besides {
    overflow-y: auto;
  }
  .layout {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    grid-template-rows: auto minmax(150px, 1fr);
    gap: var(--sp-3);
    height: 100%;
  }
  .main {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    min-width: 0;
  }
  .stage {
    width: var(--table);
    max-width: 100%;
    align-self: center;
  }
  .scaler {
    transform-origin: 0 0;
  }
  /* Beside the side column, controls line up with the table; stacked, they take the full width. */
  .besides .main {
    width: var(--table);
    max-width: 100%;
    justify-self: center;
  }


  .controls {
    flex-shrink: 0;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2) var(--sp-3);
  }
  .buttons {
    display: flex;
    align-items: center;
    gap: 2px;
  }
  .buttons svg path {
    fill: currentColor;
    stroke: currentColor;
  }
  .buttons :global(.play) {
    min-width: 116px;
    margin: 0 var(--sp-1);
  }
  .track {
    position: relative;
    flex: 1;
    min-width: 80px;
    display: flex;
    align-items: center;
    margin: 0 var(--sp-1);
  }
  input[type="range"] {
    flex: 1;
    min-width: 0;
    margin: 0;
  }
  /* The thumb is about 16px wide, so its centre runs from 8px to the width minus 8px. */
  .pip {
    position: absolute;
    top: -10px;
    left: calc(8px + (100% - 16px) * var(--at));
    width: 10px;
    height: 10px;
    padding: 0;
    border: 2px solid var(--bg);
    border-radius: 50%;
    background: var(--c);
    transform: translateX(-50%);
    cursor: pointer;
    transition: box-shadow var(--dur-fast) var(--ease-standard);
  }
  .pip.open {
    background: var(--bg);
    box-shadow: inset 0 0 0 1.5px var(--c);
  }
  .pip:hover,
  .pip.at {
    box-shadow: 0 0 0 2px var(--text);
  }
  .steps {
    min-width: 40px;
    color: var(--text-2);
    font-size: var(--fs-sm);
    text-align: right;
  }

  .bounties {
    flex-shrink: 0;
    max-height: 40%;
    overflow: auto;
    display: flex;
    flex-wrap: wrap;
    gap: 6px var(--sp-4);
    padding: var(--sp-2) var(--sp-3);
    border-radius: var(--r-md);
    background: var(--surface);
    border: 1px solid var(--line);
    font-size: var(--fs-md);
  }
  .bounties b {
    font-weight: var(--fw-medium);
  }

  .side {
    container: log / inline-size;
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    min-height: 0;
  }
  .log {
    flex: 1;
    min-height: 0;
    overflow: auto;
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    align-content: start;
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    background: var(--surface);
  }
  .street-log {
    min-width: 0;
    border-left: 1px solid var(--line);
    container: street / inline-size;
  }
  .street-log:first-child {
    border-left: 0;
  }
  h4 {
    position: sticky;
    top: 0;
    z-index: 1;
    margin: 0;
    padding: var(--sp-2) var(--sp-3) 6px;
    background: var(--surface);
    color: var(--text-2);
    font-size: var(--fs-sm);
    font-weight: var(--fw-semibold);
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
    align-items: center;
    gap: 6px;
    width: 100%;
    min-height: 26px;
    padding: 2px var(--sp-3) 2px 10px;
    border: 0;
    border-left: 2px solid transparent;
    background: transparent;
    color: var(--text-2);
    font: inherit;
    font-size: var(--fs-md);
    text-align: left;
    cursor: pointer;
    transition: background-color var(--dur-fast) var(--ease-standard);
  }
  ol button:hover {
    background: var(--surface-2);
  }
  ol button:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: -2px;
  }
  .future button {
    color: var(--text-3);
  }
  .now button {
    color: var(--text);
    background: var(--surface-3);
    border-left-color: var(--text);
  }
  .pos {
    flex-shrink: 0;
    min-width: 34px;
    padding: 0 4px;
    border-radius: var(--r-xs);
    background: var(--surface-3);
    color: var(--text-2);
    font-size: var(--fs-xs);
    font-weight: var(--fw-semibold);
    line-height: 17px;
    text-align: center;
  }
  .pos.me {
    background: var(--text);
    color: var(--bg);
  }
  .actor {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .what {
    flex-shrink: 0;
    color: inherit;
  }
  .what::before {
    content: "";
    display: inline-block;
    width: 7px;
    height: 7px;
    margin-right: 5px;
    border-radius: 2px;
    background: var(--c, transparent);
    vertical-align: 1px;
  }
  .amt {
    flex-shrink: 0;
    margin-left: auto;
    color: inherit;
  }
  .open-mark {
    flex-shrink: 0;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    border: 1.5px dashed var(--text-3);
  }
  /* In a narrow street column the position names the player; the full name is in the tooltip. */
  @container street (max-width: 210px) {
    .actor {
      display: none;
    }
    .what {
      flex: 1;
      min-width: 0;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }
  }
  .result {
    grid-column: 1 / -1;
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-3) var(--sp-6);
    padding: 0 var(--sp-3) var(--sp-3);
    border-top: 1px solid var(--line);
  }
  .result h4 {
    position: static;
    flex-basis: 100%;
    padding-inline: 0;
  }
  .who-won {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }
  .line {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-md);
  }
  .gain {
    color: var(--pos);
    font-weight: var(--fw-semibold);
  }
  .lost {
    color: var(--text-3);
  }
  .faint {
    color: var(--text-3);
    font-size: var(--fs-sm);
  }

  .besides {
    grid-template-columns: minmax(0, 1fr) var(--side);
    grid-template-rows: minmax(0, 1fr);
    gap: 20px;
  }
  .besides .main {
    align-self: start;
  }
  /* Beside the table the side column takes the full height; the log scrolls within it. */
  .besides .side {
    overflow: auto;
    /* Room for focus rings at the column's edges. */
    margin: -4px;
    padding: 4px;
  }
  /* The log scrolls on its own, so following the replay never pushes the verdict above it away. */
  .besides .log {
    display: block;
    flex: 1 1 0;
    min-height: 200px;
    overflow: auto;
  }
  .aside {
    flex-shrink: 0;
  }
  .besides .street-log {
    border-left: 0;
    border-top: 1px solid var(--line);
  }
  .besides h4 {
    position: static;
  }
  .besides .street-log:first-child {
    border-top: 0;
  }
  /* Narrow logs below the table keep two streets side by side. */
  @container log (max-width: 560px) {
    .log {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }
</style>
