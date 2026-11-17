<script lang="ts">
  import { int, mRatio, num } from "../math";

  let { mode = "bb" }: { mode?: "bb" | "m" } = $props();

  // An example structure with a classic ante per player.
  const LEVELS: [number, number, number][] = [
    [50, 100, 0],
    [100, 200, 25],
    [150, 300, 40],
    [200, 400, 50],
    [300, 600, 75],
    [400, 800, 100],
    [500, 1000, 125],
    [600, 1200, 150],
    [800, 1600, 200],
    [1000, 2000, 250],
  ];
  const PLAYERS = 8;

  let stack = $state(20000);
  let level = $state(4);

  const [sb, bb, ante] = $derived(LEVELS[level]);
  const bbs = $derived(stack / bb);
  const m = $derived(mRatio(stack, sb, bb, ante * PLAYERS));
  const value = $derived(mode === "bb" ? bbs : m);
  const MAX = $derived(mode === "bb" ? 80 : 40);

  type Zone = { from: number; to: number; label: string; name: string; color: string };
  // Rules of thumb, not exact borders.
  const ZONES: Record<"bb" | "m", Zone[]> = {
    bb: [
      { from: 0, to: 12, label: "Push/Fold", name: "Push/Fold-Bereich", color: "var(--surface-3)" },
      { from: 12, to: 25, label: "Kurz", name: "Kurzer Stack", color: "color-mix(in srgb, var(--text) 14%, var(--surface-2))" },
      { from: 25, to: 40, label: "Mittel", name: "Mittlerer Stack", color: "color-mix(in srgb, var(--text) 24%, var(--surface-2))" },
      { from: 40, to: 80, label: "Tief", name: "Tiefer Stack", color: "color-mix(in srgb, var(--text) 36%, var(--surface-2))" },
    ],
    m: [
      { from: 0, to: 1, label: "Tot", name: "Tote Zone", color: "var(--text-3)" },
      { from: 1, to: 6, label: "Rot", name: "Rote Zone", color: "var(--neg)" },
      { from: 6, to: 10, label: "Orange", name: "Orange Zone", color: "color-mix(in srgb, var(--neg) 50%, var(--amber))" },
      { from: 10, to: 20, label: "Gelb", name: "Gelbe Zone", color: "var(--amber)" },
      { from: 20, to: 40, label: "Grün", name: "Grüne Zone", color: "var(--pos)" },
    ],
  };
  const zones = $derived(ZONES[mode]);
  const zone = $derived(zones.find((z) => value >= z.from && value < z.to) ?? zones[zones.length - 1]);
  const at = (v: number) => `${(Math.min(v, MAX) / MAX) * 100}%`;
</script>

<div class="ruler">
  <div class="read">
    <div>
      <b class="num">{mode === "bb" ? `${num(bbs, bbs < 10 ? 1 : 0)} bb` : `M ${num(m)}`}</b>
      <span class="zone-name">{zone.name}</span>
    </div>
    <div class="facts">
      <span>Blinds <b class="num">{int(sb)}/{int(bb)}</b></span>
      <span>Ante <b class="num">{ante ? int(ante) : "–"}</b>{ante ? ` × ${PLAYERS}` : ""}</span>
      {#if mode === "m"}<span>Big Blinds <b class="num">{num(bbs, 0)}</b></span>{:else}<span>M <b class="num">{num(m)}</b></span>{/if}
    </div>
  </div>

  <div class="track">
    {#each zones as z (z.label)}
      <span class="z" style="left:{at(z.from)}; width:calc({at(z.to)} - {at(z.from)}); --c:{z.color}" class:cur={z === zone}>
        {#if (Math.min(z.to, MAX) - z.from) / MAX > 0.07}<em>{z.label}</em>{/if}
      </span>
    {/each}
    <span class="pin" style="left:{at(value)}"></span>
  </div>
  <div class="ticks num">
    {#each mode === "bb" ? [0, 12, 25, 40, 80] : [0, 6, 10, 20, 40] as t (t)}
      <span style="left:{at(t)}">{t}{t === MAX ? "+" : ""}</span>
    {/each}
  </div>

  <div class="sliders">
    <label for="sr-stack">Stack</label>
    <input id="sr-stack" type="range" min="2000" max="60000" step="500" bind:value={stack} />
    <b class="num">{int(stack)}</b>
    <label for="sr-level">Level</label>
    <input id="sr-level" type="range" min="0" max={LEVELS.length - 1} bind:value={level} />
    <b class="num">{level + 1}</b>
  </div>
  <p class="faint note">Beispiel-Struktur, {PLAYERS} Spieler. Zonen sind eine Faustregel{mode === "m" ? " nach Harrington; die Grenze Rot/Orange wird auch bei 5 gezogen" : ""}.</p>
</div>

<style>
  .ruler {
    display: grid;
    gap: 12px;
  }
  .read {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 16px;
    flex-wrap: wrap;
  }
  .read > div:first-child {
    display: grid;
  }
  .read b {
    font-size: 44px;
    font-weight: 600;
    line-height: 1.05;
  }
  .zone-name {
    color: var(--text-2);
    font-size: 13px;
  }
  .facts {
    display: grid;
    gap: 2px;
    text-align: right;
    color: var(--text-2);
    font-size: 13px;
  }
  .facts b {
    color: var(--text);
    font-size: 15px;
    font-weight: 600;
  }
  .track {
    position: relative;
    height: 40px;
    margin-top: 6px;
  }
  .z {
    position: absolute;
    top: 0;
    bottom: 0;
    background: color-mix(in srgb, var(--c) 45%, var(--bg));
    border-right: 2px solid var(--surface);
    display: grid;
    place-items: center;
    overflow: hidden;
    transition: background 0.15s;
  }
  .z.cur {
    background: color-mix(in srgb, var(--c) 85%, var(--bg));
  }
  .z:first-child {
    border-radius: 6px 0 0 6px;
  }
  .z:last-of-type {
    border-radius: 0 6px 6px 0;
    border-right: 0;
  }
  .z em {
    font-style: normal;
    font-size: 12px;
    color: var(--text);
    white-space: nowrap;
    text-shadow: 0 1px 2px rgba(0, 0, 0, 0.5);
  }
  .pin {
    position: absolute;
    top: -7px;
    bottom: -7px;
    width: 4px;
    margin-left: -2px;
    border-radius: 2px;
    background: var(--text);
    box-shadow: 0 0 0 2px var(--bg);
    transition: left 0.12s;
  }
  .ticks {
    position: relative;
    height: 14px;
    font-size: 11.5px;
    color: var(--text-3);
  }
  .ticks span {
    position: absolute;
    transform: translateX(-50%);
  }
  .ticks span:first-child {
    transform: none;
  }
  .ticks span:last-child {
    transform: translateX(-100%);
  }
  .sliders {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) 64px;
    align-items: center;
    gap: 8px 12px;
    margin-top: 6px;
    font-size: 13px;
    color: var(--text-2);
  }
  .sliders b {
    color: var(--text);
    font-size: 15px;
    text-align: right;
  }
  .note {
    margin: 0;
    font-size: 12px;
  }
</style>
