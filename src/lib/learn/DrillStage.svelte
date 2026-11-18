<script lang="ts" module>
  import type { ExercisePrompt } from "../types";

  export type DrillPrompt = Extract<ExercisePrompt, { type: "bounty" | "icm" | "open_fold" | "river_bet" | "stack_depth" | "hud_read" }>;
  const TYPES = new Set(["bounty", "icm", "open_fold", "river_bet", "stack_depth", "hud_read"]);
  export const isDrill = (p: ExercisePrompt): p is DrillPrompt => TYPES.has(p.type);
</script>

<script lang="ts">
  import { positionsFor } from "../solver/scenario";
  import Badge from "../ui/Badge.svelte";
  import { int } from "../ui/format";
  import PokerTable, { type TableSeat } from "../ui/PokerTable.svelte";
  import Stat from "../ui/Stat.svelte";
  import { fmtBB, fmtPct } from "./catalog";

  /** The picture of the exercises for bounty, ICM, opening, river, stack depth and HUD reads. */
  let { prompt }: { prompt: DrillPrompt } = $props();

  const dollars = (x: number) =>
    `${x.toLocaleString("de-DE", { minimumFractionDigits: x % 1 ? 2 : 0, maximumFractionDigits: 2 })} $`;
  const blindOf = (i: number, n: number) => (i === n - 1 ? 1 : i === n - 2 ? 0.5 : 0);

  /** Seats in action order: folded before the hero, the shover all-in, the rest still to act. */
  function seats(positions: string[], stacks: number[], hero: number, hand: string[], shover: number | null): TableSeat[] {
    const n = positions.length;
    return positions.map((position, i) => {
      const blind = blindOf(i, n) || undefined;
      const s: TableSeat = { position, stack: stacks[i], bet: blind, dealer: position === "BTN" };
      if (i === shover) return { ...s, bet: stacks[i], action: { kind: "allin" }, cards: "hidden" };
      if (i === hero) return { ...s, cards: hand, toAct: true };
      if (i < hero) return { ...s, folded: true };
      return s;
    });
  }

  const SHOVER: Record<string, string> = { UTG: "UTG", "der Hijack": "HJ", "der Cutoff": "CO", "der Button": "BTN" };

  /** The other player first, the hero at the bottom. */
  function riverSeats(p: Extract<DrillPrompt, { type: "river_bet" }>): TableSeat[] {
    return [
      { position: p.hero_ip ? p.oop : p.ip, stack: p.stack_bb, cards: "hidden", action: p.hero_ip ? { kind: "check" } : undefined },
      { position: p.hero_ip ? p.ip : p.oop, stack: p.stack_bb, cards: p.hand, toAct: true },
    ];
  }

  /** Everyone folds to the big blind but the shover. */
  function anteSeats(p: Extract<DrillPrompt, { type: "stack_depth" }>): TableSeat[] {
    const pos = positionsFor(p.players);
    const shover = pos.indexOf(SHOVER[p.shover ?? ""] ?? "BTN");
    return pos.map((position, i) => ({
      position,
      bet: i === shover ? (p.shove_bb ?? 0) : blindOf(i, p.players) || undefined,
      action: i === shover ? { kind: "allin" } : undefined,
      folded: i !== shover && i !== p.players - 1,
      toAct: i === p.players - 1,
      dealer: position === "BTN",
    }));
  }
</script>

{#if prompt.type === "bounty"}
  {@const p = prompt}
  {@const covers = p.hero_stack_bb >= p.shove_bb}
  {@const call = Math.min(p.shove_bb, p.hero_stack_bb)}
  <div class="duel">
    <div class="who them">
      <span class="role">Gegner · all-in</span>
      <b class="num">{fmtBB(p.shove_bb)}</b>
      {#if p.villain_bounty !== null}<Badge tone="warn">Kopfgeld {dollars(p.villain_bounty)}</Badge>{/if}
      {#if p.bounty_bb !== null}<Badge tone="warn">Kopfgeld {fmtBB(p.bounty_bb)}</Badge>{/if}
    </div>
    <div class="vs-mark" aria-hidden="true">gegen</div>
    <div class="who me">
      <span class="role">Du</span>
      <b class="num">{fmtBB(p.hero_stack_bb)}</b>
      {#if p.hero_bounty !== null}<Badge>Dein Kopfgeld {dollars(p.hero_bounty)}</Badge>{/if}
    </div>
  </div>
  <div class="money" role="img" aria-label="Im Pot {fmtBB(p.dead_bb + p.shove_bb)}, dein Call {fmtBB(call)}">
    <span class="part pot" style:flex={p.dead_bb + p.shove_bb}><b class="num">{fmtBB(p.dead_bb + p.shove_bb)}</b><small>im Pot</small></span>
    {#if p.bounty_bb !== null}
      <span class="part bounty" style:flex={p.bounty_bb}><b class="num">{fmtBB(p.bounty_bb)}</b><small>Kopfgeld</small></span>
    {/if}
    <span class="part call" style:flex={call}><b class="num">{fmtBB(call)}</b><small>dein Call</small></span>
  </div>
  <div class="facts-row">
    {#if p.rate}<Stat size="sm" label="Kurs" value="1 $ = {fmtBB(p.rate.bb_per_dollar)}" />{/if}
    {#if p.equity_pct !== null}<Stat size="sm" label="Deine Equity" value={fmtPct(p.equity_pct)} />{/if}
    {#if p.hero_bounty !== null}
      <Stat size="sm" label="Wer deckt wen?" value={covers ? "Du deckst ihn" : "Er deckt dich"} />
    {/if}
  </div>
{:else if prompt.type === "icm"}
  {@const p = prompt}
  {@const total = p.stacks_bb.reduce((a, b) => a + b, 0)}
  <div class="prizes" aria-label="Preise">
    {#each p.payouts as pay, i (i)}
      <span class="prize"><span class="place">{i + 1}. Platz</span><b class="num">{dollars(pay)}</b></span>
    {/each}
    {#if p.positions.length > p.payouts.length}<span class="prize none"><span class="place">{p.payouts.length + 1}. Platz</span><b>0 $</b></span>{/if}
  </div>
  {#if p.ask !== null}
    <ol class="stacks" aria-label="Stacks">
      {#each p.positions as pos, i (pos)}
        <li class:asked={i === p.ask}>
          <span class="seat-name">{pos}</span>
          <span class="bar"><i style:width="{(p.stacks_bb[i] / Math.max(...p.stacks_bb)) * 100}%"></i></span>
          <span class="num">{fmtBB(p.stacks_bb[i])}</span>
          <span class="share num">{fmtPct((p.stacks_bb[i] / total) * 100)} der Chips</span>
        </li>
      {/each}
    </ol>
  {:else if p.hero !== null}
    <div class="felt-box"><PokerTable seats={seats(p.positions, p.stacks_bb, p.hero, p.hand, p.shover)} hero={p.hero} label="Tisch an der Bubble" /></div>
  {/if}
{:else if prompt.type === "open_fold"}
  {@const p = prompt}
  <div class="felt-box"><PokerTable seats={seats(p.positions, p.positions.map(() => p.stack_bb), p.hero, p.hand, null)} hero={p.hero} label="Tisch, alle vor dir haben gefoldet" /></div>
  <p class="caption">
    Öffnen heißt hier: Raise auf {fmtBB(p.open_to_bb)}{#if p.ante_bb > 0} · Ante {fmtBB(p.ante_bb)}{/if}{#if p.bb_ante_bb > 0} · Big-Blind-Ante {fmtBB(p.bb_ante_bb)}{/if}
  </p>
{:else if prompt.type === "river_bet"}
  {@const p = prompt}
  <div class="felt-box"><PokerTable seats={riverSeats(p)} hero={1} board={p.board} pot={p.pot_bb} label="River" /></div>
  <p class="caption">Bet heißt hier: {fmtBB(p.bet_bb)}, also 75 % des Pots.</p>
{:else if prompt.type === "stack_depth"}
  {@const p = prompt}
  {#if p.ask === "ante_odds" && p.shove_bb !== null}
    {@const antes = p.bb_ante ? p.ante : p.ante * p.players}
    <div class="felt-box"><PokerTable seats={anteSeats(p)} hero={p.players - 1} pot={antes} label="Tisch mit Antes" /></div>
    <p class="caption">In der Mitte liegen schon {fmtBB(antes)} Antes{p.bb_ante ? ", die du als Big Blind gezahlt hast" : ""}.</p>
  {:else}
    <div class="facts-row">
      <Stat label="Dein Stack" value={int(p.stack)} unit=" Chips" />
      <Stat label="Blinds" value="{int(p.small_blind)}/{int(p.big_blind)}" />
      {#if p.ask === "m"}
        <Stat label={p.bb_ante ? "Big-Blind-Ante" : "Ante pro Spieler"} value={int(p.ante)} />
        <Stat label="Spieler" value={String(p.players)} />
      {/if}
    </div>
  {/if}
{:else if prompt.type === "hud_read"}
  {@const p = prompt}
  <div class="hud" aria-label="HUD des Gegners">
    <Stat size="lg" label="VPIP" value={fmtPct(p.vpip)} help="Wie oft er vor dem Flop freiwillig Geld in den Pot steckt." />
    <Stat size="lg" label="PFR" value={fmtPct(p.pfr)} help="Wie oft er vor dem Flop erhöht." />
    <Stat size="lg" label="3-Bet" value={fmtPct(p.three_bet)} help="Wie oft er eine Erhöhung noch einmal erhöht." />
    <Stat size="lg" label="Hände" value={int(p.hands)} help="Aus so vielen Händen stammen die Werte." />
  </div>
{/if}

<style>
  .felt-box {
    max-width: 600px;
    margin: 0 auto;
  }
  .duel {
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    align-items: center;
    gap: 12px;
    padding: 16px;
    border-radius: var(--r);
    background: var(--surface-2);
  }
  .who {
    display: grid;
    justify-items: center;
    gap: 6px;
  }
  .who b {
    font-size: var(--fs-2xl);
    font-weight: var(--fw-semibold);
  }
  .role,
  .vs-mark {
    color: var(--text-2);
    font-size: var(--fs-sm);
  }
  .money {
    display: flex;
    margin-top: 12px;
    border-radius: var(--r-sm);
    overflow: hidden;
    min-height: 52px;
  }
  .part {
    display: grid;
    align-content: center;
    gap: 2px;
    padding: 6px 10px;
    min-width: 72px;
  }
  .part.pot {
    background: var(--surface-3);
  }
  .part.bounty {
    background: color-mix(in srgb, var(--amber) 26%, var(--surface-2));
  }
  .part.call {
    background: color-mix(in srgb, var(--act-call) 30%, var(--surface-2));
  }
  .part b {
    font-size: var(--fs-lg);
  }
  .part small {
    color: var(--text-2);
    font-size: var(--fs-sm);
  }
  .facts-row {
    display: flex;
    flex-wrap: wrap;
    gap: 12px 32px;
    margin-top: 14px;
  }
  .prizes {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-bottom: 12px;
  }
  .prize {
    display: grid;
    gap: 2px;
    padding: 8px 14px;
    border-radius: var(--r-sm);
    background: var(--surface-2);
  }
  .prize b {
    font-size: var(--fs-lg);
  }
  .prize.none {
    opacity: 0.6;
  }
  .place {
    color: var(--text-2);
    font-size: var(--fs-sm);
  }
  .stacks {
    display: grid;
    gap: 6px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .stacks li {
    display: grid;
    grid-template-columns: 52px 1fr 64px 108px;
    align-items: center;
    gap: 10px;
    padding: 6px 10px;
    border-radius: var(--r-sm);
  }
  .stacks li.asked {
    background: var(--surface-2);
    box-shadow: inset 0 0 0 1px var(--line-2);
    font-weight: var(--fw-semibold);
  }
  .seat-name {
    font-weight: var(--fw-semibold);
  }
  .bar {
    height: 10px;
    border-radius: var(--r-pill);
    background: var(--surface-3);
    overflow: hidden;
  }
  .bar i {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: var(--text-2);
  }
  .asked .bar i {
    background: var(--text);
  }
  .share {
    color: var(--text-2);
    font-size: var(--fs-sm);
    text-align: right;
  }
  .caption {
    margin: 8px 0 0;
    color: var(--text-2);
    font-size: var(--fs-md);
    text-align: center;
  }
  .hud {
    display: flex;
    flex-wrap: wrap;
    gap: 16px 40px;
    padding: 18px 20px;
    border-radius: var(--r);
    background: var(--surface-2);
  }
  @container (max-width: 520px) {
    .stacks li {
      grid-template-columns: 44px 1fr 56px;
    }
    .share {
      display: none;
    }
  }
</style>
