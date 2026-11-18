<script lang="ts" module>
  export type GridMode = "you" | "model" | "diff";
</script>

<script lang="ts">
  import RangeGrid, { type RangeCell } from "../ui/RangeGrid.svelte";
  import { FEW, MOVE, TIGHT, WIDE, className, combos, pct } from "./labels";
  import { MOVES, type ModelRange, type RangeCounts } from "./types";

  /**
   * 13×13 grid of one situation on the shared RangeGrid. "you": your real actions per hand,
   * faded where there are few hands; "model": the model's strategy; "diff": where you play more
   * (orange) or less (blue) than the model. The model's playing range can be drawn as an outline.
   */
  let {
    mode,
    counts,
    model,
    outline = false,
    onCell,
  }: {
    mode: GridMode;
    counts: RangeCounts;
    model: ModelRange | null;
    outline?: boolean;
    onCell?: (i: number) => void;
  } = $props();

  const IDS = Array.from({ length: 169 }, (_, i) => i);
  const played = (c: number[]) => c[1] + c[2] + c[3];

  type Cell = { n: number; you: number | null; model: number | null; inModel: boolean };
  const cells = $derived(
    IDS.map((i): Cell => {
      const c = counts.cells[i];
      const n = c ? c[0] + c[1] + c[2] + c[3] : 0;
      const m = model?.cells[i] ?? null;
      return { n, you: n ? played(c) / n : null, model: m ? played(m) : model ? 0 : null, inModel: !!m };
    }),
  );

  /** The playing actions as shares from the left; fold is the empty rest. `strength` below 1 fades the colours. */
  function mix(shares: number[], strength = 1) {
    return [1, 2, 3]
      .filter((k) => shares[k] >= 0.005)
      .map((k) => ({
        color: strength < 1 ? `color-mix(in srgb, ${MOVE[MOVES[k]].color} ${Math.round(strength * 100)}%, var(--surface))` : MOVE[MOVES[k]].color,
        share: shares[k],
      }));
  }

  /** Bar in the hover card: the same mix as the cell. */
  function bar(shares: number[]): string {
    let x = 0;
    const stops = mix(shares).map((m) => `${m.color} ${(x * 100).toFixed(1)}% ${((x += m.share) * 100).toFixed(1)}%`);
    return stops.length ? `linear-gradient(to right, ${stops.join(", ")}, var(--surface-2) ${(x * 100).toFixed(1)}% 100%)` : "var(--surface-2)";
  }

  function corner(i: number): string | undefined {
    const cell = cells[i];
    if (mode === "you") return cell.n ? String(cell.n) : undefined;
    if (mode === "model") {
      const m = cell.model;
      return m !== null && cell.inModel && m > 0.005 && m < 0.995 ? String(Math.round(m * 100)) : undefined;
    }
    if (cell.you === null || cell.model === null) return undefined;
    const gap = Math.round((cell.you - cell.model) * 100);
    return Math.abs(gap) >= 10 ? (gap > 0 ? `+${gap}` : `−${-gap}`) : undefined;
  }

  const grid = $derived(
    IDS.map((i): RangeCell => {
      const cell = cells[i];
      const note = label(i);
      if (mode === "you") {
        if (!cell.n) return null;
        // Few hands: the colour stays recognisable but clearly weaker than a settled cell.
        const strength = cell.n >= FEW ? 1 : 0.3 + (0.45 * cell.n) / FEW;
        return { mix: mix(counts.cells[i].map((x) => x / cell.n), strength), value: corner(i), note };
      }
      if (mode === "model") {
        const m = model?.cells[i];
        return m ? { mix: mix(m), value: corner(i), note } : null;
      }
      if (cell.you === null || cell.model === null) return null;
      const gap = cell.you - cell.model;
      const strength = Math.min(1, Math.abs(gap)) * (cell.n >= FEW ? 1 : 0.55);
      const color = Math.abs(gap) < 0.1 ? "transparent" : `color-mix(in srgb, ${gap > 0 ? WIDE : TIGHT} ${Math.round(25 + 75 * strength)}%, var(--surface-2))`;
      return { color, value: corner(i), note };
    }),
  );

  // The model's playing range (at least half the time) as one outline along cell edges.
  const outlinePath = $derived.by(() => {
    if (!outline || !model) return "";
    const plays = (r: number, c: number) => r >= 0 && r < 13 && c >= 0 && c < 13 && (cells[r * 13 + c].model ?? 0) >= 0.5;
    let d = "";
    for (let r = 0; r < 13; r++) {
      for (let c = 0; c < 13; c++) {
        if (!plays(r, c)) continue;
        if (!plays(r - 1, c)) d += `M${c} ${r}h1`;
        if (!plays(r + 1, c)) d += `M${c} ${r + 1}h1`;
        if (!plays(r, c - 1)) d += `M${c} ${r}v1`;
        if (!plays(r, c + 1)) d += `M${c + 1} ${r}v1`;
      }
    }
    return d;
  });

  function verdict(c: Cell): string | null {
    if (c.you === null || c.model === null) return null;
    if (!c.inModel) return "Das Modell kommt mit dieser Hand nicht an diese Stelle – es spielt sie vorher anders.";
    const gap = c.you - c.model;
    if (Math.abs(gap) < 0.15) return "Passt ungefähr zum Modell.";
    if (gap > 0) return c.model < 0.05 ? "Das Modell foldet sie hier – du spielst sie." : "Du spielst sie öfter als das Modell.";
    return c.you < 0.05 ? "Das Modell spielt sie – du foldest sie." : "Du spielst sie seltener als das Modell.";
  }

  function shares(c: number[]): string {
    return MOVES.map((m, k) => [m, c[k]] as const)
      .filter(([, x]) => x > 0.005)
      .map(([m, x]) => `${MOVE[m].label} ${pct(x * 100)}`)
      .join(" · ");
  }

  function label(i: number): string {
    const c = cells[i];
    const you = c.n ? `du ${pct((c.you ?? 0) * 100)} gespielt in ${c.n} Händen` : "keine Hände";
    const m = c.model === null ? "" : `, Modell ${pct(c.model * 100)}`;
    return `${className(i)}: ${you}${m}`;
  }
</script>

<RangeGrid
  cells={grid}
  label="Range als 13 mal 13 Raster"
  onSelect={onCell && ((i) => cells[i].n > 0 && onCell?.(i))}
  clickHint="Klick zeigt deine Hände damit"
>
  {#snippet tip(i)}
    {@const c = cells[i]}
    {@const m = model?.cells[i] ?? null}
    {@const why = verdict(c)}
    <div class="range-tip">
      <header><b>{className(i)}</b><span>{combos(i)} Kombos</span></header>
      <div class="row">
        <span class="who">Du</span>
        {#if c.n}
          <i class="bar" style:background={bar(counts.cells[i].map((x) => x / c.n))}></i>
          <span class="what">{shares(counts.cells[i].map((x) => x / c.n))}</span>
          <span class="n num">{c.n} {c.n === 1 ? "Hand" : "Hände"}</span>
        {:else}
          <span class="what faint">Noch keine Hand in dieser Auswahl</span>
        {/if}
      </div>
      {#if model}
        <div class="row">
          <span class="who">Modell</span>
          {#if m}
            <i class="bar" style:background={bar(m)}></i>
            <span class="what">{shares(m)}</span>
          {:else}
            <span class="what faint">nicht in seiner Range</span>
          {/if}
        </div>
      {/if}
      {#if why}<p class="why">{why}</p>{/if}
      {#if c.n > 0 && c.n < FEW}<p class="few-note">Erst {c.n} {c.n === 1 ? "Hand" : "Hände"} – das kann noch Zufall sein.</p>{/if}
    </div>
  {/snippet}
  {#snippet overlay()}
    {#if outlinePath}<path class="outline" d={outlinePath} />{/if}
  {/snippet}
</RangeGrid>

<style>
  .outline {
    fill: none;
    stroke: var(--text);
    stroke-width: 2;
    stroke-linecap: square;
    vector-effect: non-scaling-stroke;
    filter: drop-shadow(0 0 1.5px rgba(0, 0, 0, 0.9));
  }
  .range-tip header {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    margin-bottom: 8px;
  }
  .range-tip header b {
    font-family: var(--font-num);
    font-size: 16px;
  }
  .range-tip header span {
    color: var(--text-3);
    font-size: 11.5px;
  }
  .row {
    display: grid;
    grid-template-columns: 46px 1fr auto;
    grid-template-areas: "who bar n" "who what what";
    column-gap: 8px;
    row-gap: 3px;
    align-items: center;
    margin-top: 6px;
  }
  .who {
    grid-area: who;
    align-self: start;
    color: var(--text-2);
    font-weight: 600;
  }
  .bar {
    grid-area: bar;
    height: 8px;
    border-radius: 2px;
  }
  .what {
    grid-area: what;
    color: var(--text);
  }
  .row:has(.faint) .what {
    grid-row: 1 / span 2;
  }
  .n {
    grid-area: n;
    color: var(--text-2);
    font-size: 11.5px;
  }
  .why {
    margin: 9px 0 0;
    color: var(--text);
  }
  .few-note {
    margin: 4px 0 0;
    color: var(--amber);
    font-size: 11.5px;
  }
</style>
