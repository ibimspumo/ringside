<script lang="ts">
  import { positionsFor } from "../solver/scenario";
  import { ANTE_LABEL, MODE_LABEL, TABLE_SIZES, configLine, tableName } from "./labels";
  import type { PlayAnte, PlayConfig, PlayMode } from "./types";

  /** Four ready-made tables and one of the player's own; the panel below tunes the choice. */
  let { config, onChange, onStart }: { config: PlayConfig; onChange: (c: PlayConfig) => void; onStart: (c: PlayConfig) => void } = $props();

  const DEEP = [25, 30, 40, 50, 75, 100];
  const SHORT = [6, 8, 10, 12, 14, 16, 18, 20];
  const TILES: { id: string; title: string; figure: string; unit: string; text: string; config: PlayConfig }[] = [
    {
      id: "full",
      title: "Ganze Hände",
      figure: "40",
      unit: "BB · 6-max",
      text: "Von Preflop bis River. Jede deiner Entscheidungen wird sofort bewertet.",
      config: { mode: "full", players: 6, ante: "big_blind", stack_bb: 40, mixed_stacks: false, hero_seat: null },
    },
    {
      id: "heads-up",
      title: "Heads-up",
      figure: "1",
      unit: "Gegner · 40 BB",
      text: "Nur du und ein Gegner. Wer den Button hat, zahlt den Small Blind und spielt fast jede Hand.",
      config: { mode: "full", players: 2, ante: "big_blind", stack_bb: 40, mixed_stacks: false, hero_seat: null },
    },
    {
      id: "short",
      title: "Kurze Stacks",
      figure: "≤ 20",
      unit: "BB · Push/Fold",
      text: "Shove oder Fold aus jeder Position, mit wechselnden Stacks von 6 bis 20 BB.",
      config: { mode: "push_fold", players: 6, ante: "big_blind", stack_bb: 10, mixed_stacks: true, hero_seat: null },
    },
    {
      id: "post",
      title: "Nur Postflop",
      figure: "3",
      unit: "Straßen · Heads-up",
      text: "Die Hand beginnt am Flop eines vorbereiteten Spots. Flop, Turn und River.",
      config: { mode: "postflop", players: 6, ante: "big_blind", stack_bb: 40, mixed_stacks: false, hero_seat: null },
    },
  ];

  let custom = $state(false);
  let panel = $state<HTMLElement>();

  function set(patch: Partial<PlayConfig>) {
    const next = { ...config, ...patch };
    if (next.mode === "push_fold" && !SHORT.includes(next.stack_bb)) next.stack_bb = 10;
    if (next.mode !== "push_fold" && !DEEP.includes(next.stack_bb)) next.stack_bb = 40;
    if (next.hero_seat !== null && next.hero_seat >= next.players) next.hero_seat = null;
    onChange(next);
  }

  function openCustom() {
    custom = true;
    queueMicrotask(() => panel?.querySelector<HTMLElement>("button")?.focus());
  }

  // Heads-up the button posts the small blind.
  const positions = $derived(config.players === 2 ? ["SB · Button", "BB"] : positionsFor(config.players));
  const stacks = $derived(config.mode === "push_fold" ? SHORT : DEEP);
  const MODES: PlayMode[] = ["full", "push_fold", "postflop"];
  const ANTES: PlayAnte[] = ["big_blind", "classic", "none"];
</script>

<div class="start">
  <header>
    <h2>Hände spielen</h2>
    <p class="dim">Zufällige Hände, endlos – die Gegner spielen nach Ringsides Lösungen, du entscheidest. Nach jedem Zug siehst du, was die Lösung tun würde und warum.</p>
  </header>

  <div class="tiles">
    {#each TILES as t (t.id)}
      <button class="tile" onclick={() => onStart(t.config)}>
        <span class="figure num">{t.figure}<small>{t.unit}</small></span>
        <b>{t.title}</b>
        <span class="text">{t.text}</span>
        <span class="go">Losspielen <svg viewBox="0 0 24 24" width="14" height="14" aria-hidden="true"><path d="M5 12h14M13 6l6 6-6 6" /></svg></span>
      </button>
    {/each}
    <button class="tile own" onclick={openCustom} aria-expanded={custom}>
      <span class="figure num">…<small>deine Wahl</small></span>
      <b>Eigene Auswahl</b>
      <span class="text">Tischgröße, Ante, Stack und dein Platz – so wie du sie üben willst.</span>
      <span class="go">Einstellen</span>
    </button>
  </div>

  <section class="panel" class:open={custom} bind:this={panel} aria-label="Eigene Auswahl">
    <div class="row">
      <span class="k">Modus</span>
      <div class="seg" role="group" aria-label="Modus">
        {#each MODES as m (m)}
          <button class:on={config.mode === m} onclick={() => set({ mode: m, mixed_stacks: m === "push_fold" ? config.mixed_stacks : false })}>{MODE_LABEL[m]}</button>
        {/each}
      </div>
    </div>
    <div class="row">
      <span class="k">Spieler</span>
      <div class="seg wrap" role="group" aria-label="Spieler am Tisch">
        {#each TABLE_SIZES as n (n)}
          <button class:on={config.players === n} onclick={() => set({ players: n })}>{tableName(n)}</button>
        {/each}
      </div>
      <span class="k">Ante</span>
      <div class="seg" role="group" aria-label="Ante">
        {#each ANTES as a (a)}
          <button class:on={config.ante === a} onclick={() => set({ ante: a })}>{ANTE_LABEL[a]}</button>
        {/each}
      </div>
    </div>
    <div class="row">
      <span class="k">Stack</span>
      <div class="seg wrap" role="group" aria-label="Stack in Big Blinds">
        {#each stacks as s (s)}
          <button class:on={!config.mixed_stacks && config.stack_bb === s} onclick={() => set({ stack_bb: s, mixed_stacks: false })}>{s}</button>
        {/each}
        <button class:on={config.mixed_stacks} onclick={() => set({ mixed_stacks: true })}>gemischt</button>
      </div>
      <span class="faint hint">BB je Spieler, alle gleich tief</span>
    </div>
    <div class="row">
      <span class="k">Dein Platz</span>
      <div class="seg wrap" role="group" aria-label="Dein Platz">
        <button class:on={config.hero_seat === null} onclick={() => set({ hero_seat: null })}>Zufall</button>
        {#each positions as p, i (p)}
          <button class:on={config.hero_seat === i} onclick={() => set({ hero_seat: i })}>{p}</button>
        {/each}
      </div>
    </div>
    <div class="row end">
      <span class="faint">{configLine(config)}</span>
      <button class="btn primary" onclick={() => onStart(config)}>Mit dieser Auswahl spielen</button>
    </div>
  </section>
</div>

<style>
  .start {
    display: grid;
    gap: 22px;
    max-width: 1280px;
    margin: 0 auto;
    padding: 8px 0 24px;
  }
  h2 {
    margin: 0 0 4px;
    font-size: 26px;
    letter-spacing: -0.01em;
  }
  header p {
    margin: 0;
    max-width: 70ch;
  }
  .tiles {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(210px, 1fr));
    gap: 12px;
  }
  .tile {
    position: relative;
    display: grid;
    align-content: start;
    gap: 8px;
    min-height: 220px;
    padding: 20px 20px 18px;
    text-align: left;
    border-radius: 14px;
    border: 1px solid var(--line-2);
    background:
      radial-gradient(120% 90% at 100% 0%, rgba(38, 55, 47, 0.75) 0%, transparent 60%),
      var(--surface);
    cursor: pointer;
    overflow: hidden;
    transition:
      border-color 0.15s,
      transform 0.15s;
  }
  .tile:hover {
    border-color: rgba(236, 235, 231, 0.28);
    transform: translateY(-2px);
  }
  .tile.own {
    background: var(--surface);
    border-style: dashed;
  }
  .figure {
    font-size: 58px;
    line-height: 0.95;
    font-weight: 600;
    letter-spacing: -0.03em;
    display: flex;
    align-items: baseline;
    gap: 8px;
  }
  .figure small {
    font-size: 13px;
    font-weight: 500;
    letter-spacing: 0;
    color: var(--text-2);
  }
  .tile b {
    font-size: 17px;
    margin-top: 6px;
  }
  .text {
    color: var(--text-2);
    font-size: 13px;
  }
  .go {
    margin-top: auto;
    padding-top: 10px;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12.5px;
    color: var(--text);
  }
  .go svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
  }
  .panel {
    display: none;
    gap: 12px;
    padding: 16px 18px;
    border-radius: var(--r);
    background: var(--surface);
    border: 1px solid var(--line);
  }
  .panel.open {
    display: grid;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px 14px;
  }
  .row.end {
    justify-content: space-between;
    padding-top: 6px;
    border-top: 1px solid var(--line);
  }
  .k {
    min-width: 72px;
    font-size: 12px;
    color: var(--text-3);
  }
  .hint {
    font-size: 12px;
  }
  .seg {
    display: inline-flex;
    gap: 2px;
    padding: 2px;
    border-radius: var(--r-sm);
    background: var(--surface-2);
  }
  .seg.wrap {
    flex-wrap: wrap;
  }
  .seg button {
    height: 28px;
    padding: 0 11px;
    border: 0;
    border-radius: 5px;
    background: none;
    color: var(--text-2);
    font-size: 12.5px;
    cursor: pointer;
  }
  .seg button.on {
    background: var(--surface-3);
    color: var(--text);
  }
  @media (prefers-reduced-motion: reduce) {
    .tile {
      transition: none;
    }
    .tile:hover {
      transform: none;
    }
  }
</style>
