<script lang="ts" module>
  import type { TableOpens } from "./preflop-types";

  /** Your seat at a table of `m` players with `behind` players after you; heads-up the button is the small blind. */
  export function seatAt(m: number, behind: number): number | null {
    const s = m === 2 && behind === 2 ? 0 : m - 1 - behind;
    return s >= 0 && s < m - 1 ? s : null;
  }

  export type Bar = { players: number; seat: number; name: string; pct: number };

  /** One bar per table size where the seat exists, the fullest table first. */
  export function barsFor(rows: TableOpens[], seatOf: (m: number) => number | null): Bar[] {
    return [...rows]
      .sort((a, b) => b.players - a.players)
      .flatMap((r) => {
        const s = seatOf(r.players);
        return s === null ? [] : [{ players: r.players, seat: s, name: r.positions[s], pct: r.pct[s] }];
      });
  }
</script>

<script lang="ts">
  import { preview } from "../learn/preview";
  import ColumnBars from "../ui/ColumnBars.svelte";
  import { pct } from "./words";

  /**
   * The same seat at other table sizes: how many hands it plays when everyone before folded.
   * Two rows: your position (the same number of players behind you), and whoever acts first.
   * A bar switches the table to that size.
   */
  let {
    rows,
    players,
    seat,
    pushfold,
    onPick,
  }: { rows: TableOpens[]; players: number; seat: number; pushfold: boolean; onPick: (players: number, seat: number) => void } = $props();

  const size = (n: number) => (n === 2 ? "heads-up" : n <= 5 ? `${n} Spielern` : `${n}-max`);
  const short = (n: number) => (n === 2 ? "HU" : String(n));
  const verb = $derived(pushfold ? "gehst du all-in mit" : "spielst du");

  const behind = $derived(players - 1 - seat);
  const mine = $derived(barsFor(rows, (m) => seatAt(m, behind)));
  const first = $derived(barsFor(rows, () => 0));
  const myName = $derived(rows.find((r) => r.players === players)?.positions[seat] ?? "");

  const list = (bars: Bar[]) => bars.map((b) => `${size(b.players)} ${pct(b.pct, 0)}\u00a0%`).join(" · ");

  /** Why your seat's range does or does not move across table sizes. */
  const mineWhy = $derived.by(() => {
    if (mine.length < 2) return "";
    const hu = mine.find((b) => b.players === 2);
    const rest = mine.filter((b) => b.players !== 2);
    const flat = rest.length > 1 && Math.max(...rest.map((b) => b.pct)) - Math.min(...rest.map((b) => b.pct)) < 8;
    const tail = hu
      ? ` Heads-up ${myName === "BTN" ? "bist du als Button selbst Small Blind, nur noch einer kommt nach dir" : "ist der Small Blind zugleich der Button und hat nach dem Flop Position"} – dann ${verb} ${pct(hu.pct, 0)}\u00a0%.`
      : "";
    if (behind === 1 && !hu) return "Hinter dem Small Blind sitzt nur noch der Big Blind.";
    const who = behind === 1 ? "nur der Big Blind" : behind === 2 ? "nur Small und Big Blind" : `${behind} Spieler`;
    return flat
      ? `Hinter dir sitzen an jedem dieser Tische ${who} – deshalb ändert sich deine Range kaum.${tail}`
      : `Hinter dir sitzen überall ${who}; die Antes der übrigen Spieler machen den Pot verschieden groß.${tail}`;
  });

  // Browser demo, for screenshots: `focus=sizes` scrolls the comparison into view.
  let panel = $state<HTMLElement>();
  $effect(() => {
    if (panel && preview("focus") === "sizes") panel.scrollIntoView({ block: "center" });
  });

  const showMine = $derived(mine.length > 1 && behind < players - 1);
  const span = (bars: Bar[], who: string) => {
    const full = bars[0];
    const few = bars[bars.length - 1];
    return `${who} ${verb} an ${size(full.players)} ${pct(full.pct, 0)}\u00a0%, ${few.players === 2 ? "heads-up" : `an ${size(few.players)}`} ${pct(few.pct, 0)}\u00a0%.`;
  };
  const firstWhy = $derived(
    first.length < 2
      ? ""
      : `Am vollen Tisch kommen nach dir noch ${first[0].players - 1} Spieler, heads-up nur einer. Je weniger Gegner nach dir handeln, desto seltener hat einer eine starke Hand – deshalb wird die Range breiter.`,
  );
</script>

{#snippet row(title: string, bars: Bar[])}
  <div class="row">
    <span class="row-title">{title}</span>
    <ColumnBars
      label="{title}: {list(bars)}"
      tone={pushfold ? "var(--act-allin)" : "var(--act-raise)"}
      selected={bars.find((b) => b.players === players && b.seat === seat)?.players ?? null}
      bars={bars.map((b) => ({
        key: b.players,
        value: b.pct,
        valueText: pct(b.pct, 0),
        label: short(b.players),
        title: `${b.name} an ${size(b.players)}: ${pct(b.pct, 0)} % der Hände`,
      }))}
      onpick={(k) => {
        const b = bars.find((x) => x.players === k);
        if (b) onPick(b.players, b.seat);
      }}
    />
  </div>
{/snippet}

{#if first.length > 1}
  <section class="panel size-compare" aria-label="Andere Tischgröße" bind:this={panel}>
    <div class="panel-head"><h2 class="panel-title">Andere Tischgröße</h2></div>
    {#if showMine}
      <p class="lead"><b>{span(mine, `${myName === "BTN" ? "Am Button" : `Als ${myName}`}`)}</b></p>
      {@render row(`Dein Platz: ${myName}`, mine)}
      <p class="why">{mineWhy}</p>
    {/if}
    <p class="lead"><b>{span(first, "Als Erster am Zug")}</b></p>
    {@render row("Als Erster am Zug", first)}
    <p class="why">{firstWhy}</p>
    <p class="foot faint">Prozent der Hände, {pushfold ? "mit denen du all-in gehst" : "die du spielst (Raise, All-in oder Limp)"}, wenn alle vor dir gefoldet haben. Unten: Spieler am Tisch, HU = heads-up. Klick wechselt den Tisch.</p>
  </section>
{/if}

<style>
  .size-compare {
    display: grid;
    gap: 10px;
    padding-bottom: 14px;
  }
  .lead,
  .why,
  .foot {
    margin: 0 18px;
    font-size: var(--fs-md);
    line-height: 1.5;
    color: var(--text-2);
  }
  .lead b {
    color: var(--text);
    font-weight: 600;
  }
  .foot {
    font-size: var(--fs-sm);
  }
  .row {
    display: grid;
    gap: 6px;
    margin: 0 18px;
  }
  .row-title {
    font-size: var(--fs-sm);
    color: var(--text-3);
  }
</style>
