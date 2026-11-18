<script lang="ts">
  /**
   * Picks one value from a fixed, ascending list: a thick track with a grip, the value in a bubble
   * on the grip, quick steps as chips below. Arrow keys move by `nudge` value units, Page Up/Down
   * jump to the next quick step, Home/End to the ends.
   */
  let {
    id,
    label,
    values,
    value,
    onchange,
    unit = "",
    format = (v: number) => v.toLocaleString("de-DE", { maximumFractionDigits: 2 }),
    steps = [],
    log = false,
    nudge = 1,
    inset = [11, 11],
  }: {
    id?: string;
    /** Shown in the bubble and read by screen readers, e.g. "Dein Stack". */
    label: string;
    values: number[];
    /** May lie between the listed values; the grip then sits where it would. */
    value: number;
    onchange: (v: number) => void;
    unit?: string;
    format?: (v: number) => string;
    /** Quick steps; those outside `values` are left out. */
    steps?: number[];
    /** Spaces the values on a log scale, so small values get room. */
    log?: boolean;
    nudge?: number;
    /** Track margins in px, to line the grip up with a chart above. */
    inset?: [number, number];
  } = $props();

  const first = $derived(values[0] ?? 0);
  const last = $derived(values[values.length - 1] ?? 1);
  const t = (v: number) => (log ? Math.log(Math.max(v, 1e-9)) : v);
  const frac = (v: number) => (t(last) > t(first) ? Math.min(1, Math.max(0, (t(v) - t(first)) / (t(last) - t(first)))) : 0);
  const f = $derived(frac(value));
  const quick = $derived(steps.filter((s) => values.includes(s)));
  const text = (v: number) => (unit ? `${format(v)} ${unit}` : format(v));

  function nearest(at: number): number {
    return values.reduce((a, b) => (Math.abs(frac(b) - at) < Math.abs(frac(a) - at) ? b : a), first);
  }

  function pick(v: number) {
    if (v !== value) onchange(v);
  }

  function key(e: KeyboardEvent) {
    const eps = 1e-9;
    const up = () => values.find((v) => v >= value + nudge - eps) ?? last;
    const down = () => values.findLast((v) => v <= value - nudge + eps) ?? first;
    const next: Record<string, () => number> = {
      ArrowRight: up,
      ArrowUp: up,
      ArrowLeft: down,
      ArrowDown: down,
      PageUp: () => quick.find((s) => s > value + eps) ?? last,
      PageDown: () => quick.findLast((s) => s < value - eps) ?? first,
      Home: () => first,
      End: () => last,
    };
    if (!next[e.key]) return;
    e.preventDefault();
    pick(next[e.key]());
  }
</script>

<div class="ui-slider" style="--f:{f};--inset-l:{inset[0]}px;--inset-r:{inset[1]}px">
  <div class="rail">
    <div class="track"><div class="fill"></div></div>
    <div class="grip" aria-hidden="true"></div>
    <div class="bubble" aria-hidden="true"><span>{label}</span> <b class="num">{text(value)}</b></div>
    <input
      {id}
      type="range"
      min="0"
      max="1000"
      step="1"
      value={Math.round(f * 1000)}
      data-values={values.join(",")}
      aria-label={label}
      aria-valuetext={text(value)}
      oninput={(e) => pick(nearest(Number(e.currentTarget.value) / 1000))}
      onkeydown={key}
    />
    <div class="ends num" aria-hidden="true"><span>{format(first)}</span><span>{format(last)}</span></div>
  </div>
  {#if quick.length}
    <div class="steps" role="group" aria-label="Schnellwahl {label}">
      <span class="steps-lbl">Schnellwahl{unit ? ` · ${unit}` : ""}</span>
      {#each quick as s (s)}
        <button type="button" class="num" aria-pressed={s === value} aria-label={text(s)} onclick={() => pick(s)}>{format(s)}</button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .ui-slider {
    --grip: 22px;
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    min-width: 0;
  }
  .rail {
    position: relative;
    margin: 0 var(--inset-r) 0 var(--inset-l);
    padding-top: 34px;
    height: calc(34px + var(--grip) + 18px);
  }
  .track {
    position: absolute;
    left: 0;
    right: 0;
    top: calc(34px + var(--grip) / 2 - 4px);
    height: 8px;
    border-radius: var(--r-pill);
    background: var(--surface-3);
    box-shadow: inset 0 0 0 1px var(--line-2);
    overflow: hidden;
  }
  .fill {
    height: 100%;
    width: calc(var(--f) * 100%);
    border-radius: inherit;
    background: color-mix(in srgb, var(--primary) 80%, transparent);
    transition: width var(--dur-fast) var(--ease-standard);
  }
  .grip {
    position: absolute;
    top: 34px;
    left: calc(var(--f) * 100%);
    width: var(--grip);
    height: var(--grip);
    margin-left: calc(var(--grip) / -2);
    border-radius: 50%;
    background: var(--primary);
    border: 3px solid var(--surface);
    box-shadow:
      0 0 0 1px var(--line-2),
      var(--shadow-1);
    pointer-events: none;
    transition:
      left var(--dur-fast) var(--ease-standard),
      transform var(--dur-fast) var(--ease-standard),
      box-shadow var(--dur-fast) var(--ease-standard);
  }
  .rail:hover .grip {
    transform: scale(1.12);
  }
  .rail:has(input:focus-visible) .grip {
    box-shadow:
      0 0 0 2px var(--surface),
      0 0 0 4px var(--focus);
  }
  /* Left edge on the grip at the start, right edge at the end: never leaves the rail. */
  .bubble {
    position: absolute;
    top: 0;
    left: calc(var(--f) * 100%);
    transform: translateX(calc(var(--f) * -100%));
    padding: 3px var(--sp-2);
    border-radius: var(--r-sm);
    background: var(--surface-3);
    box-shadow: inset 0 0 0 1px var(--line-2);
    font-size: var(--fs-sm);
    line-height: 20px;
    color: var(--text-2);
    white-space: nowrap;
    pointer-events: none;
    transition:
      left var(--dur-fast) var(--ease-standard),
      transform var(--dur-fast) var(--ease-standard);
  }
  .bubble b {
    color: var(--text);
    font-weight: var(--fw-semibold);
  }
  /* The native input lies invisibly over the track, one grip wider, so its thumb maps 1:1 onto the grip. */
  input {
    position: absolute;
    top: 34px;
    left: calc(var(--grip) / -2);
    width: calc(100% + var(--grip));
    height: var(--grip);
    margin: 0;
    opacity: 0;
    cursor: pointer;
    -webkit-appearance: none;
    appearance: none;
  }
  input::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: var(--grip);
    height: var(--grip);
  }
  input::-moz-range-thumb {
    width: var(--grip);
    height: var(--grip);
    border: 0;
  }
  .ends {
    position: absolute;
    left: 0;
    right: 0;
    top: calc(34px + var(--grip) + 2px);
    display: flex;
    justify-content: space-between;
    font-size: var(--fs-xs);
    color: var(--text-3);
    pointer-events: none;
  }
  .steps {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    margin: 0 var(--inset-r) 0 var(--inset-l);
  }
  .steps-lbl {
    margin-right: var(--sp-1);
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  button {
    min-width: 38px;
    height: 28px;
    padding: 0 var(--sp-2);
    border: 1px solid var(--line-2);
    border-radius: var(--r-pill);
    background: transparent;
    color: var(--text-2);
    font-size: var(--fs-md);
    cursor: pointer;
    transition:
      background var(--dur-fast) var(--ease-standard),
      color var(--dur-fast) var(--ease-standard),
      border-color var(--dur-fast) var(--ease-standard);
  }
  button:hover {
    border-color: var(--text-3);
    color: var(--text);
  }
  button[aria-pressed="true"] {
    background: var(--primary);
    border-color: var(--primary);
    color: var(--primary-text);
    font-weight: var(--fw-semibold);
  }
  button:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: 1px;
  }
</style>
