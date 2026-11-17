<script lang="ts">
  import { PRESETS } from "./presets";
  import { CLASSES, classCombos, classLabel, formatRange, parseRange, rangeCombos } from "./range";

  /**
   * 13×13 range editor with weights. The text is what gets solved; painting the grid rewrites it.
   * `note` marks a range that was guessed rather than chosen.
   */
  let {
    title,
    position = $bindable(""),
    value = $bindable(""),
    note = "",
    lockPosition = false,
    onEdit,
  }: { title: string; position?: string; value: string; note?: string; lockPosition?: boolean; onEdit?: () => void } = $props();

  const BRUSHES = [1, 0.75, 0.5, 0.25, 0];
  let brush = $state(1);
  let painting = $state(false);
  let text = $state(value);
  let preset = $state("");

  // Outside changes (a template, a spot from the replayer) replace the text being edited.
  $effect(() => {
    text = value;
  });

  const parsed = $derived(parseRange(text));
  let lastValid = $state<number[]>(new Array(CLASSES).fill(0));
  $effect(() => {
    if (parsed) lastValid = parsed;
  });
  const weights = $derived(parsed ?? lastValid);
  const combos = $derived(rangeCombos(weights));

  function commit(next: string) {
    text = next;
    value = next;
    onEdit?.();
  }

  function paint(i: number) {
    if (Math.abs(weights[i] - brush) < 0.005) return;
    const w = [...weights];
    w[i] = brush;
    commit(formatRange(w));
  }

  function down(e: PointerEvent, i: number) {
    if (e.button !== 0) return;
    // Clicking a cell that already has the brush weight clears it, so one brush can toggle.
    if (Math.abs(weights[i] - brush) < 0.005 && brush > 0) {
      const w = [...weights];
      w[i] = 0;
      commit(formatRange(w));
    } else paint(i);
    painting = true;
  }

  function choosePreset(id: string) {
    const p = PRESETS.find((x) => x.id === id);
    if (p) commit(p.range);
    preset = "";
  }

  function fill(w: number): string {
    if (w <= 0) return "var(--surface-2)";
    const p = (w * 100).toFixed(0);
    return `linear-gradient(to top, var(--range) ${p}%, var(--surface-2) ${p}%)`;
  }
</script>

<svelte:window onpointerup={() => (painting = false)} />

<section class="panel editor">
  <header>
    <div class="who">
      <b>{title}</b>
      {#if lockPosition}
        <span class="dim">{position}</span>
      {:else}
        <input class="field pos" bind:value={position} aria-label="Position von {title}" maxlength="6" />
      {/if}
    </div>
    <span class="dim num">{combos.toLocaleString("de-DE", { maximumFractionDigits: 1 })} Kombos · {((combos / 1326) * 100).toLocaleString("de-DE", { maximumFractionDigits: 1 })} %</span>
  </header>
  {#if note}
    <p class="assume" title="Vorschlag aus Position und Preflop-Aktion – nicht aus der Hand gelesen.">
      <b>Annahme</b>
      {note}. Bitte prüfen und anpassen.
    </p>
  {/if}

  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="grid" onpointerleave={() => (painting = false)}>
    {#each weights as w, i (i)}
      <button
        type="button"
        class="cell"
        class:on={w >= 0.5}
        style="background:{fill(w)}"
        title="{classLabel(i)} · {classCombos(i)} Kombos · {Math.round(w * 100)} %"
        onpointerdown={(e) => down(e, i)}
        onpointerenter={() => painting && paint(i)}
      >
        <span>{classLabel(i)}</span>
      </button>
    {/each}
  </div>

  <div class="tools">
    <div class="seg" role="group" aria-label="Gewicht zum Malen">
      {#each BRUSHES as b (b)}
        <button class:on={brush === b} onclick={() => (brush = b)}>{b === 0 ? "Leeren" : `${b * 100} %`}</button>
      {/each}
    </div>
    <select class="field" bind:value={preset} onchange={() => choosePreset(preset)} aria-label="Vorlage für {title}">
      <option value="">Vorlage …</option>
      {#each PRESETS as p (p.id)}
        <option value={p.id}>{p.label}</option>
      {/each}
    </select>
    <button class="btn ghost" onclick={() => commit("")}>Leeren</button>
  </div>
  <textarea
    class="field text"
    class:bad={parsed === null}
    rows="3"
    spellcheck="false"
    bind:value={text}
    oninput={() => {
      if (parsed !== null) {
        value = text;
        onEdit?.();
      }
    }}
    aria-label="Range von {title} als Text"
  ></textarea>
  {#if parsed === null}<p class="err">Unbekannte Schreibweise – z. B. „22+, A2s+, KTo+, AhKd, QQ:0.5“.</p>{/if}
</section>

<style>
  .editor {
    --range: color-mix(in srgb, var(--amber) 80%, var(--surface-2));
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 14px;
    min-width: 0;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    flex-wrap: wrap;
  }
  .who {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .pos {
    width: 64px;
    height: 26px;
    font-size: 12px;
    text-align: center;
  }
  .assume {
    margin: 0;
    padding: 7px 10px;
    border-radius: var(--r-sm);
    border: 1px dashed color-mix(in srgb, var(--amber) 55%, transparent);
    color: var(--text-2);
    font-size: 12.5px;
  }
  .assume b {
    color: var(--amber);
    font-weight: 600;
    margin-right: 4px;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(13, minmax(0, 1fr));
    gap: 2px;
    touch-action: none;
    user-select: none;
  }
  .cell {
    aspect-ratio: 1;
    min-width: 0;
    padding: 0;
    border: 0;
    border-radius: 3px;
    color: var(--text-3);
    cursor: pointer;
    display: grid;
    place-items: center;
  }
  .cell span {
    font-family: var(--font-num);
    font-size: 10.5px;
    font-weight: 600;
    pointer-events: none;
  }
  .cell.on {
    color: #121418;
  }
  .cell:hover {
    outline: 1px solid var(--text);
    z-index: 1;
  }
  .tools {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    align-items: center;
  }
  .text {
    height: auto;
    padding: 7px 10px;
    font-family: var(--font-num);
    font-size: 12.5px;
    line-height: 1.45;
    resize: vertical;
  }
  .text.bad {
    border-color: var(--neg);
  }
  .err {
    margin: -4px 0 0;
    color: var(--neg);
    font-size: 12px;
  }
  @container (max-width: 520px) {
    .cell span {
      font-size: 8px;
    }
  }
</style>
