<script lang="ts">
  import type { StreetSizes } from "./types";

  /**
   * Bet and raise sizes per street as a few named choices; any other percentage stays visible
   * as its own chip and can be typed under "Eigene Prozentwerte".
   */
  let {
    sizes = $bindable(),
    allin = $bindable(),
    firstStreet,
    max,
    onEdit,
  }: {
    sizes: [StreetSizes, StreetSizes, StreetSizes];
    allin: number;
    firstStreet: number;
    max: number;
    onEdit?: () => void;
  } = $props();

  const STREETS = ["Flop", "Turn", "River"];
  const BETS = [
    { pct: 33, label: "Klein" },
    { pct: 66, label: "Mittel" },
    { pct: 100, label: "Groß" },
    { pct: 150, label: "Overbet" },
  ];
  const RAISES = [
    { pct: 50, label: "Klein" },
    { pct: 100, label: "Groß" },
  ];
  /** All-in on offer once the stack is at most this share of the pot. */
  const ALLIN_ON = 150;

  function toggle(street: number, kind: "bet" | "raise", pct: number) {
    const next = sizes.map((s) => ({ bet: [...s.bet], raise: [...s.raise] })) as typeof sizes;
    const list = next[street][kind];
    const at = list.indexOf(pct);
    if (at >= 0) list.splice(at, 1);
    else if (list.length < max) list.push(pct);
    else return;
    list.sort((a, b) => a - b);
    sizes = next;
    onEdit?.();
  }

  /** Sizes typed by hand that none of the named choices covers. */
  const extra = (list: number[], named: { pct: number }[]) => list.filter((p) => !named.some((n) => n.pct === p));

  let text = $state(toText(sizes));
  $effect(() => {
    text = toText(sizes);
  });

  function toText(list: StreetSizes[]) {
    return list.map((s) => ({ bet: s.bet.join(", "), raise: s.raise.join(", ") }));
  }

  function parse(t: string): number[] | null {
    const nums = t
      .split(/[,;\s]+/)
      .map((x) => x.replace("%", "").trim())
      .filter(Boolean)
      .map(Number);
    return nums.every((n) => Number.isFinite(n) && n > 0) ? nums : null;
  }

  function setText() {
    sizes = text.map((s, i) => ({
      bet: parse(s.bet) ?? sizes[i].bet,
      raise: parse(s.raise) ?? sizes[i].raise,
    })) as typeof sizes;
    onEdit?.();
  }
</script>

<div class="sizes">
  {#each STREETS as street, i (street)}
    <div class="row" class:off={i < firstStreet}>
      <span class="street">{street}</span>
      <div class="group" role="group" aria-label="Bets am {street}">
        <span class="kind">Bet</span>
        {#each BETS as b (b.pct)}
          {@const on = sizes[i].bet.includes(b.pct)}
          <button class="opt" class:on aria-pressed={on} disabled={i < firstStreet} onclick={() => toggle(i, "bet", b.pct)}>
            {b.label} <span class="num">{b.pct} %</span>
          </button>
        {/each}
        {#each extra(sizes[i].bet, BETS) as p (p)}
          <button class="opt on" aria-pressed="true" disabled={i < firstStreet} onclick={() => toggle(i, "bet", p)} title="Eigene Größe entfernen">
            <span class="num">{p} %</span> ×
          </button>
        {/each}
      </div>
      <div class="group" role="group" aria-label="Raises am {street}">
        <span class="kind">Raise</span>
        {#each RAISES as r (r.pct)}
          {@const on = sizes[i].raise.includes(r.pct)}
          <button class="opt" class:on aria-pressed={on} disabled={i < firstStreet} onclick={() => toggle(i, "raise", r.pct)}>
            {r.label} <span class="num">{r.pct} %</span>
          </button>
        {/each}
        {#each extra(sizes[i].raise, RAISES) as p (p)}
          <button class="opt on" aria-pressed="true" disabled={i < firstStreet} onclick={() => toggle(i, "raise", p)} title="Eigene Größe entfernen">
            <span class="num">{p} %</span> ×
          </button>
        {/each}
      </div>
    </div>
  {/each}
  <label class="check allin">
    <input
      type="checkbox"
      checked={allin > 0}
      onchange={(e) => {
        allin = e.currentTarget.checked ? ALLIN_ON : 0;
        onEdit?.();
      }}
    />
    <span>All-in anbieten, sobald der Stack höchstens {ALLIN_ON} % des Pots ist</span>
  </label>
  <p class="faint note">
    Bets in % des Pots, Raises in % des Pots nach dem Call. Höchstens {max} je Straße; jede Größe mehr vervielfacht Speicher und Zeit.
  </p>
  <details>
    <summary>Eigene Prozentwerte</summary>
    <table>
      <thead><tr><th></th><th>Bet in % Pot</th><th>Raise in % Pot</th></tr></thead>
      <tbody>
        {#each STREETS as street, i (street)}
          <tr class:off={i < firstStreet}>
            <td>{street}</td>
            <td>
              <input
                class="field num"
                class:bad={parse(text[i].bet) === null && text[i].bet.trim() !== ""}
                bind:value={text[i].bet}
                onchange={setText}
                disabled={i < firstStreet}
                aria-label="Bet-Größen {street}"
              />
            </td>
            <td>
              <input class="field num" bind:value={text[i].raise} onchange={setText} disabled={i < firstStreet} aria-label="Raise-Größen {street}" />
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
    <label class="inline">
      <span>All-in anbieten bis</span>
      <input class="field num narrow" type="number" min="0" step="10" bind:value={allin} oninput={onEdit} />
      <span>% des Pots</span>
    </label>
  </details>
</div>

<style>
  .sizes {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .row {
    display: grid;
    grid-template-columns: 44px minmax(0, 1fr) auto;
    align-items: center;
    gap: 6px 14px;
  }
  .row.off {
    opacity: 0.4;
  }
  .street {
    font-size: 12.5px;
    color: var(--text-2);
  }
  .group {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px;
  }
  .kind {
    width: 38px;
    font-size: 11px;
    color: var(--text-3);
  }
  .opt {
    height: 26px;
    padding: 0 8px;
    border: 1px solid var(--line-2);
    border-radius: 99px;
    background: none;
    color: var(--text-2);
    font-size: 12px;
    cursor: pointer;
    white-space: nowrap;
  }
  .opt .num {
    color: var(--text-3);
  }
  .opt:hover:not(:disabled) {
    color: var(--text);
    border-color: var(--text-3);
  }
  .opt.on {
    background: var(--surface-3);
    border-color: var(--text-2);
    color: var(--text);
  }
  .opt.on .num {
    color: var(--text-2);
  }
  .opt:disabled {
    cursor: default;
  }
  .allin {
    margin-top: 2px;
    font-size: 12.5px;
    color: var(--text-2);
  }
  .note {
    margin: 0;
    font-size: 11.5px;
  }
  details summary {
    cursor: pointer;
    font-size: 12.5px;
    color: var(--text-2);
  }
  details table {
    width: 100%;
    margin-top: 8px;
    border-collapse: collapse;
    font-size: 12.5px;
  }
  th {
    color: var(--text-3);
    font-weight: 500;
    font-size: 11px;
    text-align: left;
    padding: 0 0 4px 6px;
  }
  td {
    padding: 2px 0 2px 6px;
  }
  td:first-child {
    padding-left: 0;
    color: var(--text-2);
    width: 46px;
  }
  td .field {
    width: 100%;
    height: 28px;
  }
  .field.bad {
    border-color: var(--neg);
  }
  tr.off {
    opacity: 0.4;
  }
  .inline {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 8px;
    font-size: 12px;
    color: var(--text-2);
  }
  .narrow {
    width: 70px;
  }
  @container (max-width: 640px) {
    .row {
      grid-template-columns: 44px minmax(0, 1fr);
    }
    .row .group:last-child {
      grid-column: 2;
    }
  }
</style>
