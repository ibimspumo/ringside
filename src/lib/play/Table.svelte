<script lang="ts">
  import PlayingCard from "../ui/PlayingCard.svelte";
  import OvalFelt from "../table/OvalFelt.svelte";
  import { fit, minHeight, placeOf } from "../table/seats";
  import MiniCard from "../ui/MiniCard.svelte";
  import ProgressRing from "../ui/ProgressRing.svelte";
  import { madeOf, num } from "./labels";
  import type { Choice, HandView } from "./types";

  /**
   * The table at one step of the hand (`at` indexes `view.snapshots`): you at the bottom,
   * the others clockwise in the order they act. Per seat the last action of the street as a
   * badge and the chips in front on the line to the middle; chips slide in from the seat and
   * into the pot when the street ends. `ms` is the length of one step (0: no motion).
   * `solving` is set while the hand waits for a live solve: its progress 0–1, or null while
   * it is unknown; a ring on the board shows it.
   */
  let { view, at, final, ms, solving }: { view: HandView; at: number; final: boolean; ms: number; solving?: number | null } = $props();

  const snap = $derived(view.snapshots[Math.min(at, view.snapshots.length - 1)]);
  const n = $derived(view.players);
  const button = $derived(n === 2 ? 0 : n - 3);

  let w = $state(900);
  let vh = $state(900);
  const small = $derived(w < 860);
  const SEAT = $derived(small ? { w: 84, h: 66 } : { w: 104, h: 74 });
  const ME = $derived(small ? { w: 116, h: 96 } : { w: 136, h: 108 });
  const SIZES = $derived({
    seat: SEAT,
    me: ME,
    badge: small ? { w: 56, h: 20 } : { w: 72, h: 22 },
    // Narrow tables stack the chip above the amount: lines to the middle stay short.
    bet: small ? { w: 56, h: 36 } : { w: 92, h: 26 },
    dealer: small ? 20 : 22,
    gap: small ? 4 : 5,
    onEdge: true,
  });
  // Board above, pot below, symmetric round the centre.
  const MIDDLE = $derived(
    small
      ? [
          { box: { w: 5 * 36 + 24, h: 50 }, dy: -16 },
          { box: { w: 104, h: 26 }, dy: 28 },
        ]
      : [
          { box: { w: 5 * 48 + 24, h: 67 }, dy: -18 },
          { box: { w: 116, h: 28 }, dy: 37.5 },
        ],
  );
  const heightFor = (vw: number) => Math.round(Math.max(minHeight(SIZES, MIDDLE), Math.min(vw * 0.6, vh * (vh < 800 ? 0.36 : 0.46), 540)));
  const fitted = $derived(fit(n, w, heightFor, SIZES, MIDDLE, placeOf(button, view.hero, n)));
  const geo = $derived(fitted.geo);
  const scale = $derived(fitted.scale);
  const h = $derived(geo.h);
  const at_ = (seat: number) => geo.places[placeOf(seat, view.hero, n)];

  type Kind = Choice["kind"];
  /** Each seat's last action on the street shown; folds stay for the rest of the hand. */
  const badges = $derived.by(() => {
    const out: ({ text: string; kind: Kind } | null)[] = new Array(n).fill(null);
    for (const e of view.events) {
      if (e.snapshot > at) break;
      if (e.type === "board") {
        for (let s = 0; s < n; s++) if (out[s]?.kind !== "fold") out[s] = null;
      } else {
        out[e.seat] = { text: badgeText(e.choice), kind: e.choice.kind };
      }
    }
    return out;
  });

  /** The chips in front show the amount, so the badge names the action alone. */
  const BADGE: Record<Choice["kind"], string> = { fold: "Fold", check: "Check", call: "Call", limp: "Limp", bet: "Bet", raise: "Raise", all_in: "All-in" };
  const badgeText = (c: Choice) => BADGE[c.kind];

  /** The seat whose action this step shows. */
  const actor = $derived.by(() => {
    const e = view.events.find((x) => x.snapshot === at);
    return e?.type === "action" ? e.seat : null;
  });

  const outcome = $derived(final ? view.outcome : null);
  /** At the showdown each player's hand in words, on their plate instead of the stack. */
  const made = (s: number) => (outcome?.reason === "showdown" && outcome.made[s] && !snap.folded[s] ? (madeOf(view, s) ?? outcome.made[s]) : null);
  const winners = $derived(outcome?.winners ?? []);
  /** Cards turned up at the end: only players still in the hand show theirs. */
  const shown = (s: number) => (outcome?.reason === "showdown" && !snap.folded[s] ? outcome.shown[s] : null);
  /** Hands of folded opponents the hand reveals for learning, apart from their seats. */
  const revealed = $derived(
    outcome ? outcome.shown.map((c, s) => ({ c, s })).filter((x) => x.c && (snap.folded[x.s] || outcome.reason !== "showdown")) : [],
  );
  /** At the end the whole pot slides to a single winner. */
  const payout = $derived(outcome && winners.length === 1 ? `${num(snap.pot)} BB` : "");
  /** The board slot the live solve's ring sits in: the next card's. */
  const ringSlot = $derived(solving === undefined ? -1 : Math.min(snap.board.length, 4));

  /** Slides an element from `from` to its place, or from its place to `to` when it leaves. */
  function slide(_: Element, { dx, dy, out = false }: { dx: number; dy: number; out?: boolean }) {
    return {
      duration: ms ? Math.min(420, ms * 0.8) : 0,
      // Cubic ease-out (fast start, soft landing), close to --ease-enter; never linear.
      css: (t: number) => {
        const k = (1 - t) ** 3;
        return `transform: translate(-50%, -50%) translate(${dx * k}px, ${dy * k}px); opacity: ${Math.min(1, t * (out ? 2 : 2.5))}`;
      },
    };
  }

  const seats = $derived(Array.from({ length: n }, (_, s) => s));
  const KIND_NAME: Record<Kind, string> = { fold: "k-fold", check: "k-check", call: "k-call", limp: "k-call", bet: "k-bet", raise: "k-bet", all_in: "k-allin" };
</script>

<svelte:window bind:innerHeight={vh} />

<div class="wrap" bind:clientWidth={w}>
  <div class="frame" style="height:{h * scale}px">
  <div class="stage" style="width:{geo.w}px; height:{h}px; transform:scale({scale}); --ms:{ms}ms; --bw:{SIZES.badge.w}px; --bh:{SIZES.badge.h}px; --th:{SIZES.bet.h}px; --fs:{small ? 'var(--fs-sm)' : 'var(--fs-md)'}" aria-label="Pokertisch">
    <OvalFelt felt={geo.felt} />

    <div class="board" data-part="middle" style="left:{geo.center.x}px; top:{geo.center.y + MIDDLE[0].dy}px" aria-label="Board">
      {#each [0, 1, 2, 3, 4] as i (i)}
        {#if i === ringSlot && !snap.board[i]}
          <span class="slot ring" class:sm={small} data-part="solving-ring">
            <ProgressRing value={solving ?? null} size={small ? 30 : 38} stroke={3} tone="info" label="Spot wird gerechnet">
              {#if solving !== null && solving !== undefined}<span class="num">{Math.round(solving * 100)}</span>{/if}
            </ProgressRing>
          </span>
        {:else if snap.board[i]}
          <span class="deal" style:--d="{(i < 3 ? i : 0) * 60}ms"><PlayingCard card={snap.board[i]} size={small ? 36 : 48} /></span>
        {:else}
          <span class="slot" class:sm={small}></span>
        {/if}
      {/each}
    </div>
    <div class="pot num" data-part="middle" class:gone={!!payout} style="left:{geo.center.x}px; top:{geo.center.y + MIDDLE[1].dy}px" aria-live="polite">
      Pot <b>{num(snap.pot)} BB</b>
    </div>

    {#each seats as s (s)}
      {@const p = at_(s)}
      {#if snap.bets[s] > 0 && !payout}
        {#key `${s}-${snap.street}`}
          <div
            class="bet num"
            class:stack={small}
            data-part="bet"
            data-seat={s}
            style="left:{p.bet.x}px; top:{p.bet.y}px"
            in:slide|global={{ dx: p.seat.x - p.bet.x, dy: p.seat.y - p.bet.y }}
            out:slide|global={{ dx: geo.center.x - p.bet.x, dy: geo.center.y - p.bet.y, out: true }}
          >
            <span class="coin" aria-hidden="true"></span>{num(snap.bets[s])} BB
          </div>
        {/key}
      {/if}
      {#if s === button}
        <span class="dealer" data-part="dealer" style="left:{p.dealer.x}px; top:{p.dealer.y}px" title="Dealer-Button">D</span>
      {/if}
      {#if badges[s]}
        {@const b = badges[s]!}
        {#key `${b.text}-${s}`}
          <span class="badge {KIND_NAME[b.kind]}" data-part="badge" data-seat={s} style="left:{p.badge.x}px; top:{p.badge.y}px" in:slide|global={{ dx: 0, dy: 0 }}>{b.text}</span>
        {/key}
      {/if}
    {/each}

    {#if payout}
      {@const p = at_(winners[0])}
      <div class="bet win num" data-part="bet" style="left:{p.bet.x}px; top:{p.bet.y}px" in:slide|global={{ dx: geo.center.x - p.bet.x, dy: geo.center.y - p.bet.y }}>
        <span class="coin" aria-hidden="true"></span>{payout}
      </div>
    {/if}

    {#each seats as s (s)}
      {@const p = at_(s)}
      {@const me = s === view.hero}
      {@const out = snap.folded[s]}
      <div
        class="seat"
        class:me
        class:out
        class:acting={snap.to_act === s && !final}
        class:just={actor === s}
        class:won={winners.includes(s)}
        data-part="seat"
        data-seat={s}
        style="left:{p.seat.x}px; top:{p.seat.y}px; width:{p.box.w}px; height:{p.box.h}px"
      >
        <div class="hole" class:big={me} class:open={!me && !!shown(s)}>
          {#if me}
            {#key view.seed}
              <span class="deal" style:--d="0ms"><PlayingCard card={view.hero_cards[0]} size={small ? 46 : 54} /></span>
              <span class="deal" style:--d="60ms"><PlayingCard card={view.hero_cards[1]} size={small ? 46 : 54} /></span>
            {/key}
          {:else if !out}
            {@const c = shown(s)}
            <span class:flip={!!c}><PlayingCard card={c?.[0]} hidden={!c} size={small ? 26 : 30} /></span>
            <span class:flip={!!c}><PlayingCard card={c?.[1]} hidden={!c} size={small ? 26 : 30} /></span>
          {/if}
        </div>
        <div class="plate">
          <b>{me ? `Du · ${view.positions[s]}` : view.positions[s]}</b>
          {#if made(s)}
            <span class="made" title="Hand beim Showdown">{made(s)}</span>
          {:else if winners.includes(s)}
            <span class="made">gewinnt</span>
          {:else}
            <span class="num">{num(snap.stacks[s])} BB</span>
          {/if}
        </div>
      </div>
    {/each}
  </div>
  </div>

  {#if revealed.length}
    <p class="reveal">
      {#each revealed as r (r.s)}
        <span>Das hatte {view.positions[r.s]}: {#each r.c ?? [] as c (c)}<MiniCard card={c} />{/each}</span>
      {/each}
    </p>
  {/if}
</div>

<style>
  .wrap {
    width: 100%;
    min-width: 0;
  }
  .frame {
    /* Motion of the table: things that appear enter on the theme's curve, never linear. */
    --ease-move: var(--ease-enter);
    position: relative;
    width: 100%;
  }
  .stage {
    position: absolute;
    left: 0;
    top: 0;
    transform-origin: 0 0;
  }
  .board {
    position: absolute;
    transform: translate(-50%, -50%);
    display: flex;
    gap: 6px;
  }
  .slot {
    width: 48px;
    height: 67px;
    border-radius: 6px;
    border: 1px dashed var(--line-2);
  }
  .slot.sm {
    width: 36px;
    height: 50px;
  }
  .slot.ring {
    display: grid;
    place-items: center;
    border-style: solid;
    border-color: color-mix(in srgb, var(--info) 45%, transparent);
  }
  .deal {
    display: inline-flex;
    animation: deal calc(var(--ms) * 0.7) var(--ease-move) both;
    animation-delay: var(--d, 0ms);
  }
  @keyframes deal {
    from {
      opacity: 0;
      transform: translateY(-10px) scale(0.96);
    }
  }
  .pot {
    position: absolute;
    transform: translate(-50%, -50%);
    height: 28px;
    display: inline-flex;
    align-items: center;
    gap: 5px;
    white-space: nowrap;
    font-size: var(--fs-md);
    color: var(--text-2);
    background: color-mix(in srgb, var(--bg) 60%, transparent);
    padding: 3px 12px;
    border-radius: 999px;
  }
  .pot.gone {
    visibility: hidden;
  }
  .pot b {
    color: var(--text);
    font-size: var(--fs-lg);
    font-weight: 600;
  }
  .seat {
    position: absolute;
    transform: translate(-50%, -50%);
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: flex-end;
    transition: opacity calc(var(--ms) * 0.6) var(--ease-move);
    z-index: 2;
  }
  .seat.out {
    opacity: 0.75;
  }
  .hole {
    display: flex;
    gap: 3px;
    margin-bottom: -12px;
  }
  /* Cards turned up stand almost clear of the plate, so both ranks and suits read. */
  .hole.open {
    margin-bottom: -3px;
  }
  .hole.big {
    gap: 5px;
    margin-bottom: -8px;
  }
  .flip {
    display: inline-flex;
    animation: flip calc(var(--ms) + var(--dur-fast)) var(--ease-move) both;
  }
  @keyframes flip {
    from {
      transform: rotateY(90deg);
    }
  }
  .plate {
    position: relative;
    z-index: 1;
    display: grid;
    justify-items: center;
    width: 100%;
    padding: 5px 8px;
    border-radius: 9px;
    background: var(--plate);
    box-shadow:
      inset 0 0 0 1px var(--plate-edge),
      var(--shadow-2);
    font-size: var(--fs-base);
    line-height: 1.3;
    transition: box-shadow calc(var(--ms) * 0.4) var(--ease-move);
  }
  .plate b {
    font-weight: 600;
    letter-spacing: 0.02em;
  }
  .plate span {
    color: var(--text-2);
    font-size: var(--fs-md);
  }
  .plate .made {
    color: var(--text);
    font-weight: 600;
    white-space: nowrap;
  }
  .won .plate .made {
    color: var(--pos);
  }
  .me .plate {
    background: var(--surface-3);
  }
  .acting .plate {
    box-shadow:
      inset 0 0 0 2px var(--focus),
      0 0 0 4px color-mix(in srgb, var(--focus) 22%, transparent),
      var(--shadow-2);
  }
  .just .plate {
    animation: pulse calc(var(--ms) * 1.2) var(--ease-move);
  }
  @keyframes pulse {
    from {
      box-shadow:
        inset 0 0 0 2px var(--text-2),
        var(--shadow-2);
    }
  }
  .won .plate {
    box-shadow:
      inset 0 0 0 2px var(--pos),
      var(--shadow-2);
  }
  .badge {
    --k: var(--text-3);
    overflow: hidden;
    text-overflow: ellipsis;
    position: absolute;
    z-index: 3;
    transform: translate(-50%, -50%);
    max-width: var(--bw);
    height: var(--bh);
    padding: 0 9px;
    display: inline-flex;
    align-items: center;
    border-radius: 999px;
    font-size: var(--fs);
    font-weight: 600;
    line-height: 1;
    white-space: nowrap;
    color: var(--text);
    background: color-mix(in srgb, var(--k) 26%, var(--bg));
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--k) 70%, transparent);
  }
  .badge.k-fold {
    --k: var(--act-fold);
  }
  .badge.k-check {
    --k: var(--act-check);
  }
  .badge.k-call {
    --k: var(--act-call);
  }
  .badge.k-bet {
    --k: var(--act-bet-1);
  }
  .badge.k-allin {
    --k: var(--act-allin);
  }
  .bet {
    position: absolute;
    z-index: 2;
    transform: translate(-50%, -50%);
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: var(--th);
    padding: 0 9px 0 4px;
    font-size: var(--fs);
    font-weight: 600;
    white-space: nowrap;
    border-radius: 999px;
    background: color-mix(in srgb, var(--bg) 62%, transparent);
    color: var(--text);
  }
  .bet.stack {
    flex-direction: column;
    justify-content: center;
    gap: 1px;
    height: var(--th);
    min-width: 0;
    padding: 0 6px;
    border-radius: 8px;
    font-size: var(--fs-xs);
  }
  .bet.stack .coin {
    width: 13px;
    height: 13px;
  }
  .bet.win {
    color: var(--pos);
  }
  .coin {
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: radial-gradient(circle, var(--amber) 0 36%, color-mix(in srgb, var(--amber) 70%, var(--bg)) 38% 62%, var(--amber) 64%);
    box-shadow: 0 0 0 1px color-mix(in srgb, var(--bg) 50%, transparent);
  }
  .dealer {
    position: absolute;
    z-index: 3;
    transform: translate(-50%, -50%);
    width: 22px;
    height: 22px;
    border-radius: 50%;
    background: var(--text);
    color: var(--bg);
    font-size: var(--fs-xs);
    font-weight: 700;
    display: grid;
    place-items: center;
    box-shadow: var(--shadow-1);
  }
  .reveal {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 6px 18px;
    margin: 4px 0 0;
    font-size: 13px;
    color: var(--text-2);
  }
  @media (prefers-reduced-motion: reduce) {
    .deal,
    .flip,
    .just .plate {
      animation: none;
    }
    .seat {
      transition: none;
    }
  }
</style>
