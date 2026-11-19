<script lang="ts">
  import { onMount } from "svelte";
  import { preview } from "../learn/preview";
  import { positionsFor } from "../solver/scenario";
  import Badge from "../ui/Badge.svelte";
  import Button from "../ui/Button.svelte";
  import SegmentedControl from "../ui/SegmentedControl.svelte";
  import Tag from "../ui/Tag.svelte";
  import { ANTE_LABEL, MODE_LABEL, TABLE_SIZES, configLine, tableName } from "./labels";
  import type { PlayAnte, PlayConfig, PlayMode } from "./types";

  /** Four ready-made tables and one of the player's own; the panel below tunes the choice. */
  let { config, busy = false, onChange, onStart }: { config: PlayConfig; busy?: boolean; onChange: (c: PlayConfig) => void; onStart: (c: PlayConfig) => void } = $props();

  const DEEP = [25, 30, 40, 50, 75, 100];
  const SHORT = [6, 8, 10, 12, 14, 16, 18, 20];
  type Tile = { id: string; title: string; figure: string; unit: string; text: string; seats: number; streets: number; config: PlayConfig };
  const TILES: Tile[] = [
    {
      id: "full",
      title: "Ganze Hände",
      figure: "40",
      unit: "Big Blinds · 6 Spieler",
      text: "Vom Austeilen bis zur letzten Karte. Jede deiner Entscheidungen wird sofort bewertet.",
      seats: 6,
      streets: 4,
      config: { mode: "full", players: 6, ante: "big_blind", stack_bb: 40, mixed_stacks: false, hero_seat: null },
    },
    {
      id: "heads-up",
      title: "Heads-up",
      figure: "1",
      unit: "Gegner · 40 BB",
      text: "Nur du und ein Gegner. Wer den Button hat, zahlt den Small Blind und spielt fast jede Hand.",
      seats: 2,
      streets: 4,
      config: { mode: "full", players: 2, ante: "big_blind", stack_bb: 40, mixed_stacks: false, hero_seat: null },
    },
    {
      id: "short",
      title: "Wenige Chips",
      figure: "6–20",
      unit: "Big Blinds",
      text: "Kurzer Stack: alles setzen (Push) oder aussteigen (Fold) – aus jeder Position, mit wechselnden Stacks.",
      seats: 6,
      streets: 1,
      config: { mode: "push_fold", players: 6, ante: "big_blind", stack_bb: 10, mixed_stacks: true, hero_seat: null },
    },
    {
      id: "post",
      title: "Ab dem Flop",
      figure: "3",
      unit: "Setzrunden",
      text: "Die Hand beginnt am Flop eines vorbereiteten Spots, du gegen einen Gegner: Flop, Turn, River.",
      seats: 2,
      streets: 3,
      config: { mode: "postflop", players: 6, ante: "big_blind", stack_bb: 40, mixed_stacks: false, hero_seat: null },
    },
  ];

  // Browser demo, for screenshots: `custom=1` opens the own choice.
  let custom = $state(preview("custom") === "1");
  let panel = $state<HTMLElement>();

  function set(patch: Partial<PlayConfig>) {
    const next = { ...config, ...patch };
    if (next.mode === "push_fold" && !SHORT.includes(next.stack_bb)) next.stack_bb = 10;
    if (next.mode !== "push_fold" && !DEEP.includes(next.stack_bb)) next.stack_bb = 40;
    if (next.hero_seat !== null && next.hero_seat >= next.players) next.hero_seat = null;
    onChange(next);
  }

  function openCustom() {
    custom = !custom;
    if (!custom) return;
    queueMicrotask(() => {
      panel?.scrollIntoView({ block: "nearest", behavior: "smooth" });
      panel?.querySelector<HTMLElement>("[role=radio][aria-checked=true]")?.focus({ preventScroll: true });
    });
  }

  onMount(() => {
    if (custom) panel?.scrollIntoView({ block: "nearest" });
  });

  /** Seats round a small oval: you at the bottom, the others clockwise, as at the table. */
  function dots(n: number) {
    return Array.from({ length: n }, (_, i) => {
      const a = Math.PI / 2 + (i / n) * Math.PI * 2;
      return { x: 50 + Math.cos(a) * 40, y: 30 + Math.sin(a) * 21, me: i === 0 };
    });
  }

  // Heads-up the button posts the small blind.
  const positions = $derived(config.players === 2 ? ["SB", "BB"] : positionsFor(config.players));
  const FULL: Record<string, string> = {
    UTG: "Under the Gun: als Erster dran, braucht die stärksten Hände",
    "UTG+1": "UTG+1: früh dran, noch viele Spieler nach dir",
    "UTG+2": "UTG+2: früh dran, noch viele Spieler nach dir",
    LJ: "Lowjack: mittlere Position",
    HJ: "Hijack: zwei Plätze vor dem Button",
    CO: "Cutoff: direkt vor dem Button, schon recht spät",
    BTN: "Button: nach dem Flop immer zuletzt dran, die beste Position",
    SB: "Small Blind: zahlt einen halben Big Blind, nach dem Flop zuerst dran",
    BB: "Big Blind: zahlt einen Big Blind, vor dem Flop zuletzt dran",
  };
  const fullName = (p: string) => (config.players === 2 && p === "SB" ? "Small Blind und Button: vor dem Flop zuerst, danach zuletzt dran" : FULL[p]);
  /** Seats round the oval in the order they act, the first top left. */
  const spot = (i: number, n: number) => {
    const a = Math.PI * 1.25 + (i / n) * Math.PI * 2;
    return `left:${50 + Math.cos(a) * 42}%; top:${50 + Math.sin(a) * 36}%`;
  };
  const stacks = $derived(config.mode === "push_fold" ? SHORT : DEEP);
  const MODES: PlayMode[] = ["full", "push_fold", "postflop"];
  const ANTES: PlayAnte[] = ["big_blind", "classic", "none"];
  const stackValue = $derived(config.mixed_stacks ? "mixed" : String(config.stack_bb));
</script>

<div class="start">
  <div class="tiles">
    {#each TILES as t (t.id)}
      <button class="tile" onclick={() => onStart(t.config)} disabled={busy}>
        <svg class="mini" viewBox="0 0 100 60" aria-hidden="true">
          <ellipse cx="50" cy="30" rx="40" ry="21" />
          {#each dots(t.seats) as d, i (i)}<circle cx={d.x} cy={d.y} r={d.me ? 5 : 3.6} class:me={d.me} />{/each}
          {#each Array.from({ length: t.streets }) as _, i (i)}<rect x={50 - t.streets * 4 + i * 8 + 1} y="26" width="6" height="8" rx="1" />{/each}
        </svg>
        <b>{t.title}</b>
        {#if t.id === "full"}<span class="pick"><Badge tone="good">Empfohlen zum Start</Badge></span>{/if}
        <span class="figure num">{t.figure}<small>{t.unit}</small></span>
        <span class="text">{t.text}</span>
        <span class="go" class:main={t.id === "full"}>Losspielen <svg viewBox="0 0 24 24" width="14" height="14" aria-hidden="true"><path d="M5 12h14M13 6l6 6-6 6" /></svg></span>
      </button>
    {/each}
    <button class="tile own" class:open={custom} onclick={openCustom} aria-expanded={custom} aria-controls="play-custom">
      <svg class="mini" viewBox="0 0 100 60" aria-hidden="true">
        <ellipse cx="50" cy="30" rx="40" ry="21" />
        <path class="slider" d="M28 30h44" /><circle class="knob" cx="58" cy="30" r="5" />
      </svg>
      <b>Eigene Auswahl</b>
      <span class="figure num">2–9<small>Spieler</small></span>
      <span class="text">Tischgröße, Ante, Stack und dein Platz – so, wie du sie üben willst.</span>
      <span class="go">{custom ? "Zuklappen" : "Einstellen"}</span>
    </button>
  </div>

  {#if custom}
    <section class="custom" id="play-custom" bind:this={panel} aria-label="Eigene Auswahl">
      <div class="grid">
        <span class="k">Modus</span>
        <SegmentedControl size="sm" label="Modus" value={config.mode} options={MODES.map((m) => ({ value: m, label: MODE_LABEL[m] }))} onchange={(m: PlayMode) => set({ mode: m, mixed_stacks: m === "push_fold" ? config.mixed_stacks : false })} />
        <span class="k">Spieler</span>
        <SegmentedControl size="sm" label="Spieler am Tisch" value={config.players} options={TABLE_SIZES.map((n) => ({ value: n as number, label: tableName(n) }))} onchange={(n: number) => set({ players: n })} />
        <span class="k">Ante</span>
        <SegmentedControl size="sm" label="Ante" value={config.ante} options={ANTES.map((a) => ({ value: a, label: ANTE_LABEL[a] }))} onchange={(a: PlayAnte) => set({ ante: a })} />
        <span class="k">Stack</span>
        <div class="line">
          <SegmentedControl
            size="sm"
            label="Stack in Big Blinds"
            value={stackValue}
            options={[...stacks.map((s) => ({ value: String(s), label: String(s) })), { value: "mixed", label: "gemischt" }]}
            onchange={(v: string) => set(v === "mixed" ? { mixed_stacks: true } : { stack_bb: Number(v), mixed_stacks: false })}
          />
          <span class="hint">{config.mixed_stacks ? "jeder Spieler anders tief" : "BB je Spieler, alle gleich tief"}</span>
        </div>
        <span class="k">Dein Platz</span>
        <div class="seats">
          <div class="oval" role="group" aria-label="Dein Platz am Tisch">
            {#each positions as p, i (p)}
              <span class="seat" style={spot(i, positions.length)} title={fullName(p)}>
                <Tag selected={config.hero_seat === i} onclick={() => set({ hero_seat: i })}>{p}</Tag>
              </span>
            {/each}
            <span class="mid"><Tag selected={config.hero_seat === null} onclick={() => set({ hero_seat: null })}>Zufall</Tag></span>
          </div>
          <p class="hint">{config.hero_seat === null ? "Jede Hand auf einem anderen Platz – so lernst du alle kennen." : fullName(positions[config.hero_seat])}</p>
        </div>
      </div>
      <div class="end">
        <span class="faint">{configLine(config)}</span>
        <Button variant="primary" onclick={() => onStart(config)} disabled={busy}>Mit dieser Auswahl spielen</Button>
      </div>
    </section>
  {/if}
</div>

<style>
  .start {
    display: grid;
    gap: 16px;
    max-width: 1360px;
    margin: 0 auto;
  }
  .tiles {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
    gap: 12px;
  }
  .tile {
    position: relative;
    display: grid;
    grid-template-rows: auto auto auto 1fr auto;
    align-content: start;
    gap: 6px;
    min-height: 260px;
    padding: 18px 20px;
    text-align: left;
    color: var(--text);
    border-radius: var(--r-xl);
    border: 1px solid var(--line-2);
    background:
      radial-gradient(120% 90% at 100% 0%, color-mix(in srgb, var(--felt) 85%, transparent) 0%, transparent 62%),
      var(--surface);
    cursor: pointer;
    overflow: hidden;
    transition:
      border-color var(--dur) var(--ease-standard),
      transform var(--dur) var(--ease-standard),
      background-color var(--dur) var(--ease-standard);
  }
  .tile:hover:not(:disabled) {
    border-color: color-mix(in srgb, var(--text) 28%, transparent);
    transform: translateY(-2px);
  }
  .tile:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: 2px;
  }
  .tile:disabled {
    cursor: progress;
    opacity: 0.7;
  }
  .tile.own {
    background: var(--surface);
    border-style: dashed;
  }
  .tile.own.open {
    border-style: solid;
    border-color: var(--focus);
  }
  .mini {
    width: 92px;
    height: 56px;
    margin: -4px 0 4px -6px;
  }
  .mini ellipse {
    fill: var(--felt);
    stroke: var(--felt-edge);
    stroke-width: 3;
  }
  .mini circle {
    fill: var(--text-3);
  }
  .mini circle.me {
    fill: var(--text);
  }
  .mini rect {
    fill: color-mix(in srgb, var(--text) 80%, transparent);
  }
  .mini .slider {
    stroke: var(--text-3);
    stroke-width: 3;
    stroke-linecap: round;
  }
  .mini .knob {
    fill: var(--text);
  }
  .figure {
    display: flex;
    align-items: baseline;
    gap: 8px;
    white-space: nowrap;
    font-size: var(--fs-3xl);
    line-height: 1;
    font-weight: 600;
    letter-spacing: -0.02em;
  }
  .figure small {
    font-size: var(--fs-md);
    font-weight: 500;
    letter-spacing: 0;
    color: var(--text-2);
  }
  .tile b {
    margin-top: 2px;
    font-size: var(--fs-2xl);
  }
  .text {
    color: var(--text-2);
    font-size: var(--fs-md);
    line-height: 1.45;
  }
  .pick {
    position: absolute;
    top: 18px;
    right: 18px;
  }
  .go.main {
    justify-self: start;
    margin-top: 12px;
    padding: 7px 14px;
    border-radius: var(--r-sm);
    background: var(--primary);
    color: var(--primary-text);
  }
  .go {
    padding-top: 12px;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-md);
    font-weight: 600;
    color: var(--text);
  }
  .go svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    transition: transform var(--dur) var(--ease-standard);
  }
  .tile:hover:not(:disabled) .go svg {
    transform: translateX(3px);
  }
  .custom {
    display: grid;
    gap: 14px;
    padding: 18px 20px;
    border-radius: var(--r-xl);
    background: var(--surface);
    border: 1px solid var(--line-2);
    animation: open var(--dur-slow) var(--ease-enter) both;
  }
  @keyframes open {
    from {
      opacity: 0;
      transform: translateY(-6px);
    }
  }
  .grid {
    display: grid;
    grid-template-columns: max-content minmax(0, 1fr);
    justify-items: start;
    align-items: center;
    gap: 12px 18px;
  }
  .line {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 14px;
  }
  .seats {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px 20px;
  }
  .oval {
    position: relative;
    width: 300px;
    height: 132px;
    border-radius: 50%;
    background: var(--felt);
    box-shadow: inset 0 0 0 4px var(--felt-edge);
    margin: 14px 26px;
  }
  .oval .seat,
  .oval .mid {
    position: absolute;
    transform: translate(-50%, -50%);
  }
  .oval .mid {
    left: 50%;
    top: 50%;
  }
  .seats .hint {
    max-width: 34ch;
    margin: 0;
  }
  .k {
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .hint {
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .end {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding-top: 14px;
    border-top: 1px solid var(--line);
    font-size: var(--fs-md);
  }
  @media (prefers-reduced-motion: reduce) {
    .tile,
    .go svg {
      transition: none;
    }
    .tile:hover:not(:disabled) {
      transform: none;
    }
    .custom {
      animation: none;
    }
  }
</style>
